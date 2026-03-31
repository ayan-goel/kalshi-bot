mod api;
mod bot_state;
mod config;
mod cross_market;
mod db;
mod event_detector;
mod exchange;
mod execution;
mod fair_value;
mod log_buffer;
mod market_scanner;
mod orderbook;
mod risk;
mod state;
mod strategy;
mod types;

use std::sync::Arc;

use anyhow::{Context, Result};
use std::path::Path;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing_subscriber::Layer;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use api::ws::WsEvent;
use bot_state::{BotState, BotStateMachine};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = config::AppConfig::load(Path::new("config/default.yaml"))?;
    let log_buffer = log_buffer::LogBuffer::from_env(10_000);

    let log_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::try_new(&config.logging.level).unwrap_or_else(|_| EnvFilter::new("info"))
    });

    if config.logging.json {
        tracing_subscriber::registry()
            .with(fmt::layer().json().with_filter(log_filter))
            .with(log_buffer::LogBufferLayer::new(log_buffer.clone()))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(fmt::layer().with_filter(log_filter))
            .with(log_buffer::LogBufferLayer::new(log_buffer.clone()))
            .init();
    }

    tracing::info!(
        environment = %config.environment,
        trading_enabled = config.trading_enabled(),
        "Starting Kalshi bot service"
    );

    let db_pool = db::init_pool(&config).await?;
    tracing::info!("Database connected and migrations applied");

    let config = load_config_overrides(config, &db_pool).await;

    let api_port = config.api_port;
    let state_engine = Arc::new(RwLock::new(state::StateEngine::new(db_pool.clone())));
    let bot_state_machine = Arc::new(RwLock::new(BotStateMachine::new(db_pool.clone())));
    let shared_config = Arc::new(RwLock::new(config.clone()));

    let (event_broadcast_tx, _) = broadcast::channel::<WsEvent>(1024);
    let event_broadcast = Arc::new(event_broadcast_tx);

    let (bot_cmd_tx, bot_cmd_rx) = mpsc::channel::<api::BotCommand>(32);

    let api_secret = std::env::var("BOT_API_SECRET").ok();
    if api_secret.is_some() {
        tracing::info!(
            "API secret is set — all endpoints (except /api/health) require Bearer token"
        );
    } else {
        tracing::warn!("BOT_API_SECRET is not set — API is unauthenticated!");
    }

    let app_state = api::AppState {
        state_engine: state_engine.clone(),
        bot_state: bot_state_machine.clone(),
        config: shared_config.clone(),
        db_pool: db_pool.clone(),
        event_tx: event_broadcast.clone(),
        bot_cmd_tx,
        log_buffer: log_buffer.clone(),
        api_secret,
    };

    let router = api::create_router(app_state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}")).await?;
    tracing::info!(port = api_port, "Axum API server starting");

    let api_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!(error = %e, "Axum server error");
        }
    });

    // ── Process-level exchange connection ──
    // Create REST client + fetch initial balance/positions so the dashboard
    // always has data even before the bot is started.
    let rest_client = exchange::rest::KalshiRestClient::new(&config)
        .context("Failed to create REST client")?;

    match rest_client.get_balance().await {
        Ok(bal) => {
            tracing::info!(available = %bal.available, portfolio_value = %bal.portfolio_value, "Initial balance loaded");
            state_engine.write().await.set_balance(bal);
        }
        Err(e) => tracing::warn!(error = %e, "Failed to fetch initial balance"),
    }

    match rest_client.get_positions().await {
        Ok(positions) => {
            tracing::info!(count = positions.len(), "Initial positions loaded");
            let mut engine = state_engine.write().await;
            for pos in positions {
                engine.upsert_position(pos);
            }
        }
        Err(e) => tracing::warn!(error = %e, "Failed to fetch initial positions"),
    }

    // ── Process-level WebSocket ──
    // The exchange event channel feeds data into the state engine regardless
    // of whether the trading loop is running.
    let (exchange_event_tx, mut exchange_event_rx) =
        mpsc::channel::<types::ExchangeEvent>(4096);

    let ws_config = config.clone();
    let _ws_handle = tokio::spawn({
        let event_tx = exchange_event_tx.clone();
        async move {
            exchange::websocket::run_websocket(ws_config, vec![], event_tx, true).await;
        }
    });

    // ── Process-level data sync ──
    // Forwards exchange events into the state engine and broadcasts to dashboard WS.
    // Also periodically refreshes balance/positions from REST.
    let data_sync_state = state_engine.clone();
    let data_sync_broadcast = event_broadcast.clone();
    let data_sync_rest = rest_client.clone();
    let _data_sync_handle = tokio::spawn(async move {
        let mut sync_tick = tokio::time::interval(tokio::time::Duration::from_secs(120));
        sync_tick.tick().await;

        loop {
            tokio::select! {
                event = exchange_event_rx.recv() => {
                    match event {
                        Some(ev) => {
                            broadcast_exchange_event(&ev, &data_sync_broadcast);
                            let mut engine = data_sync_state.write().await;
                            engine.process_event(ev).await;
                        }
                        None => {
                            tracing::error!("Exchange event channel closed — WS task exited");
                            break;
                        }
                    }
                }
                _ = sync_tick.tick() => {
                    if let Ok(bal) = data_sync_rest.get_balance().await {
                        let mut engine = data_sync_state.write().await;
                        tracing::debug!(available = %bal.available, "Balance refreshed");
                        engine.set_balance(bal);
                    }
                    if let Ok(positions) = data_sync_rest.get_positions().await {
                        let mut engine = data_sync_state.write().await;
                        for pos in positions {
                            engine.upsert_position(pos);
                        }
                    }
                }
            }
        }
    });

    // ── PnL snapshot loop ──
    let pnl_state = state_engine.clone();
    let pnl_pool = db_pool.clone();
    let pnl_broadcast = event_broadcast.clone();
    let pnl_bot_state = bot_state_machine.clone();
    let pnl_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let bot = pnl_bot_state.read().await;
            let is_running = bot.state() == BotState::Running;
            drop(bot);
            if !is_running {
                continue;
            }

            let mut engine = pnl_state.write().await;
            engine.roll_daily_context(chrono::Utc::now());

            let available = engine.balance().available;
            let portfolio_value = engine.balance().portfolio_value;
            let equity = engine.current_equity();

            let session_realized = engine.session_realized_pnl();
            let session_unrealized = engine.session_unrealized_pnl();
            let session_total = engine.session_total_pnl();

            let daily_realized = engine.daily_realized_pnl();
            let daily_unrealized = engine.daily_unrealized_pnl();
            let daily_total = engine.daily_total_pnl();

            let session_started_at = engine.session_started_at();
            let open_orders = engine.open_order_count() as i32;
            let active_markets = engine.active_market_count() as i32;
            drop(engine);

            let _ = db::insert_pnl_snapshot(
                &pnl_pool,
                session_realized,
                session_unrealized,
                available,
                portfolio_value,
                equity,
                session_total,
                daily_realized,
                daily_unrealized,
                daily_total,
                session_started_at,
                open_orders,
                active_markets,
            )
            .await;

            let _ = pnl_broadcast.send(WsEvent::PnlTick {
                realized_pnl: session_realized.to_string(),
                unrealized_pnl: session_unrealized.to_string(),
                balance: available.to_string(),
                portfolio_value: portfolio_value.to_string(),
                equity: equity.to_string(),
                session_pnl: session_total.to_string(),
                daily_pnl: daily_total.to_string(),
            });
        }
    });

    run_bot_control_loop(
        bot_cmd_rx,
        state_engine,
        bot_state_machine,
        shared_config,
        db_pool,
        event_broadcast,
    )
    .await;

    pnl_handle.abort();
    api_handle.abort();
    tracing::info!("Service shutdown complete");
    Ok(())
}

async fn load_config_overrides(
    mut config: config::AppConfig,
    pool: &sqlx::PgPool,
) -> config::AppConfig {
    if let Ok(Some(val)) = db::get_config(pool, "strategy").await {
        if let Ok(s) = serde_json::from_value::<config::StrategyConfig>(val) {
            tracing::info!("Loaded strategy config override from DB");
            config.strategy = s;
        }
    }
    if let Ok(Some(val)) = db::get_config(pool, "risk").await {
        if let Ok(r) = serde_json::from_value::<config::RiskConfig>(val) {
            tracing::info!("Loaded risk config override from DB");
            config.risk = r;
        }
    }
    if let Ok(Some(val)) = db::get_config(pool, "trading").await {
        if let Ok(t) = serde_json::from_value::<config::TradingConfig>(val) {
            tracing::info!("Loaded trading config override from DB");
            config.trading = t;
        }
    }
    config
}

async fn run_bot_control_loop(
    mut cmd_rx: mpsc::Receiver<api::BotCommand>,
    state_engine: api::SharedState,
    bot_state: api::SharedBotState,
    config: api::SharedConfig,
    db_pool: sqlx::PgPool,
    event_broadcast: api::EventBroadcast,
) {
    let mut trading_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut trading_shutdown_tx: Option<mpsc::Sender<()>> = None;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::warn!("CTRL+C received, shutting down");
                if let Some(tx) = trading_shutdown_tx.take() {
                    let _ = tx.send(()).await;
                }
                if let Some(h) = trading_handle.take() {
                    let _ = h.await;
                }
                break;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(api::BotCommand::Start) => {
                        let mut bsm = bot_state.write().await;
                        if bsm.transition(BotState::Starting, "api_start", None).await.is_err() {
                            continue;
                        }
                        drop(bsm);

                        let _ = event_broadcast.send(WsEvent::StateChange {
                            from: "stopped".to_string(),
                            to: "starting".to_string(),
                            trigger: "api_start".to_string(),
                        });

                        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
                        trading_shutdown_tx = Some(shutdown_tx);

                        let se = state_engine.clone();
                        let bs = bot_state.clone();
                        let cfg = config.clone();
                        let pool = db_pool.clone();
                        let broadcast = event_broadcast.clone();

                        trading_handle = Some(tokio::spawn(async move {
                            run_trading_loop(se, bs, cfg, pool, broadcast, shutdown_rx).await;
                        }));
                    }
                    Some(api::BotCommand::Stop) => {
                        if let Some(tx) = trading_shutdown_tx.take() {
                            let _ = tx.send(()).await;
                        }
                        if let Some(h) = trading_handle.take() {
                            let _ = h.await;
                        }

                        let mut bsm = bot_state.write().await;
                        let from = bsm.state().to_string();
                        let _ = bsm.transition(BotState::Stopped, "api_stop", None).await;
                        drop(bsm);

                        let _ = event_broadcast.send(WsEvent::StateChange {
                            from,
                            to: "stopped".to_string(),
                            trigger: "api_stop".to_string(),
                        });
                    }
                    Some(api::BotCommand::Kill) => {
                        tracing::warn!("Kill command received");
                        if let Some(tx) = trading_shutdown_tx.take() {
                            let _ = tx.send(()).await;
                        }
                        if let Some(h) = trading_handle.take() {
                            let _ = h.await;
                        }

                        let mut bsm = bot_state.write().await;
                        let from = bsm.state().to_string();
                        bsm.transition(BotState::Stopped, "kill_switch", None).await.ok();
                        drop(bsm);

                        let _ = event_broadcast.send(WsEvent::StateChange {
                            from,
                            to: "stopped".to_string(),
                            trigger: "kill_switch".to_string(),
                        });
                    }
                    Some(api::BotCommand::SwitchEnvironment { environment }) => {
                        if let Some(tx) = trading_shutdown_tx.take() {
                            let _ = tx.send(()).await;
                        }
                        if let Some(h) = trading_handle.take() {
                            let _ = h.await;
                        }

                        let mut bsm = bot_state.write().await;
                        let from = bsm.state().to_string();
                        let _ = bsm.transition(BotState::Stopped, "env_switch", None).await;
                        drop(bsm);

                        let mut cfg = config.write().await;
                        cfg.environment = environment.clone();
                        drop(cfg);

                        let mut engine = state_engine.write().await;
                        engine.clear_all();
                        drop(engine);

                        let _ = event_broadcast.send(WsEvent::StateChange {
                            from,
                            to: "stopped".to_string(),
                            trigger: format!("env_switch to {environment}"),
                        });

                        tracing::info!(environment = %environment, "Environment switched, bot stopped. Start again to use new environment.");
                    }
                    None => {
                        tracing::info!("Command channel closed");
                        break;
                    }
                }
            }
        }
    }
}

async fn run_trading_loop(
    state_engine: api::SharedState,
    bot_state: api::SharedBotState,
    config: api::SharedConfig,
    db_pool: sqlx::PgPool,
    event_broadcast: api::EventBroadcast,
    mut shutdown_rx: mpsc::Receiver<()>,
) {
    let cfg = config.read().await.clone();

    let rest_client = match create_rest_client(&cfg) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create REST client");
            let mut bsm = bot_state.write().await;
            let _ = bsm
                .transition(
                    BotState::Error,
                    "rest_client_init_failed",
                    Some(serde_json::json!({ "message": e.to_string() })),
                )
                .await;
            return;
        }
    };

    // Startup reconciliation — fetches balance, positions, selects markets.
    // Does NOT clear_all: balance/positions/connectivity persist from the
    // process-level data sync.
    match reconcile_startup(&rest_client, &state_engine, &cfg).await {
        Ok(()) => {
            tracing::info!("Startup reconciliation complete");
        }
        Err(e) => {
            let full_err = format!("{e:#}");
            tracing::error!(error = %full_err, "Startup reconciliation failed");
            let mut bsm = bot_state.write().await;
            let _ = bsm
                .transition(
                    BotState::Error,
                    "reconciliation_failed",
                    Some(serde_json::json!({ "message": full_err })),
                )
                .await;
            return;
        }
    }

    // Initialize PnL baselines
    let now = chrono::Utc::now();
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight")
        .and_utc();

    let daily_realized = match db::sum_fill_cashflow_since(&db_pool, day_start).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to restore daily realized PnL from DB; defaulting to 0");
            rust_decimal::Decimal::ZERO
        }
    };

    let fallback_equity = {
        let engine = state_engine.read().await;
        engine.current_equity()
    };

    let daily_start_equity = match db::get_first_equity_snapshot_since(&db_pool, day_start).await {
        Ok(Some(v)) => v,
        Ok(None) => fallback_equity,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to read day-start equity baseline from DB; using current equity");
            fallback_equity
        }
    };

    {
        let mut engine = state_engine.write().await;
        engine.initialize_pnl_context(now, daily_realized, daily_start_equity);
    }

    {
        let mut bsm = bot_state.write().await;
        if bsm
            .transition(BotState::Running, "reconciliation_complete", None)
            .await
            .is_err()
        {
            return;
        }
    }

    let _ = event_broadcast.send(WsEvent::StateChange {
        from: "starting".to_string(),
        to: "running".to_string(),
        trigger: "reconciliation_complete".to_string(),
    });

    // Subscribe to market-specific WS channels by re-sending the target
    // market tickers through the WS task. The existing process-level WS
    // was started with an empty ticker list; now we tell it which markets
    // to watch. We accomplish this by spawning a dedicated WS for trading
    // markets while the process-level WS handles user-level events.
    let target_markets: Vec<String> = {
        let engine = state_engine.read().await;
        engine.books().keys().map(|k| k.0.clone()).collect()
    };

    let (trading_event_tx, mut trading_event_rx) =
        mpsc::channel::<types::ExchangeEvent>(4096);

    let ws_config = cfg.clone();
    let ws_markets = target_markets.clone();
    let ws_handle = tokio::spawn({
        let tx = trading_event_tx;
        async move {
            exchange::websocket::run_websocket(ws_config, ws_markets, tx, false).await;
        }
    });

    let risk_engine = risk::RiskEngine::new(&cfg.risk);
    let strategy_engine = strategy::MarketMakerStrategy::new(&cfg.strategy);
    let mut execution_engine =
        execution::ExecutionEngine::new(rest_client.clone(), db_pool.clone(), &cfg.strategy);
    let fair_value_engine = fair_value::FairValueEngine::new(&cfg.strategy);
    let cross_market_checker = cross_market::CrossMarketChecker::new();
    let mut event_detector = event_detector::EventDetector::new(&cfg.strategy);

    let tick_interval = tokio::time::Duration::from_millis(cfg.strategy.tick_interval_ms);
    let mut tick = tokio::time::interval(tick_interval);

    let rescan_interval =
        tokio::time::Duration::from_secs(cfg.trading.market_rescan_interval_mins as u64 * 60);
    let mut rescan_tick = tokio::time::interval(rescan_interval);
    rescan_tick.tick().await;

    let mut order_sync_tick = tokio::time::interval(tokio::time::Duration::from_secs(120));
    order_sync_tick.tick().await;

    let max_markets = cfg.trading.max_markets_active;

    let mut skip_markets: std::collections::HashSet<types::MarketTicker> =
        std::collections::HashSet::new();
    let mut market_failures: std::collections::HashMap<types::MarketTicker, u32> =
        std::collections::HashMap::new();
    const BLACKLIST_THRESHOLD: u32 = 3;
    let mut disconnect_pause_logged = false;
    let mut disconnect_cancel_attempted = false;

    tracing::info!(
        markets = target_markets.len(),
        tick_ms = cfg.strategy.tick_interval_ms,
        rescan_mins = cfg.trading.market_rescan_interval_mins,
        "Trading loop started"
    );

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("Shutdown signal received in trading loop");
                let engine = state_engine.write().await;
                execution_engine.cancel_all(&engine).await;
                drop(engine);

                let mut bsm = bot_state.write().await;
                let _ = bsm.transition(BotState::Stopping, "shutdown", None).await;
                let _ = bsm.transition(BotState::Stopped, "orders_cancelled", None).await;
                break;
            }
            event = trading_event_rx.recv() => {
                match event {
                    Some(types::ExchangeEvent::BookResyncNeeded { market_ticker }) => {
                        tracing::info!(market = %market_ticker, "Performing REST book resync after sequence gap");
                        match rest_client.get_orderbook(&market_ticker.0).await {
                            Ok(ob_data) => {
                                let yes_bids = orderbook_data_to_price_levels(&ob_data.yes_dollars);
                                let no_bids = orderbook_data_to_price_levels(&ob_data.no_dollars);
                                let mut engine = state_engine.write().await;
                                engine.process_event(types::ExchangeEvent::BookSnapshot {
                                    market_ticker,
                                    yes_bids,
                                    no_bids,
                                    seq: 0,
                                }).await;
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, market = %market_ticker, "REST book resync failed");
                            }
                        }
                    }
                    Some(ev) => {
                        let is_disconnect = matches!(&ev, types::ExchangeEvent::Disconnected);
                        let is_connect = matches!(&ev, types::ExchangeEvent::Connected);

                        broadcast_exchange_event(&ev, &event_broadcast);
                        let mut engine = state_engine.write().await;
                        engine.process_event(ev).await;
                        drop(engine);

                        if is_connect {
                            disconnect_cancel_attempted = false;
                            disconnect_pause_logged = false;
                        }

                        if is_disconnect && !disconnect_cancel_attempted {
                            tracing::warn!("Exchange disconnected — issuing one-time best-effort cancel_all");
                            let engine = state_engine.read().await;
                            execution_engine.cancel_all(&engine).await;
                            disconnect_cancel_attempted = true;
                        }
                    }
                    None => {
                        tracing::error!("Trading event channel closed — WS task exited");
                        let mut bsm = bot_state.write().await;
                        let _ = bsm
                            .transition(
                                BotState::Error,
                                "event_channel_closed",
                                Some(serde_json::json!({
                                    "message": "WebSocket event channel closed unexpectedly"
                                })),
                            )
                            .await;
                        drop(bsm);
                        break;
                    }
                }
            }
            _ = order_sync_tick.tick() => {
                match rest_client.get_balance().await {
                    Ok(bal) => {
                        let mut engine = state_engine.write().await;
                        tracing::debug!(available = %bal.available, "Balance refreshed");
                        engine.set_balance(bal);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Balance refresh failed");
                    }
                }

                match rest_client.get_positions().await {
                    Ok(positions) => {
                        let mut engine = state_engine.write().await;
                        for pos in positions {
                            engine.upsert_position(pos);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Position sync failed");
                    }
                }

                match rest_client.get_orders(Some("resting")).await {
                    Ok(live_orders) => {
                        let live_ids: std::collections::HashSet<String> =
                            live_orders.iter().map(|o| o.order_id.clone()).collect();
                        let mut engine = state_engine.write().await;
                        let stale: Vec<String> = engine
                            .open_orders()
                            .keys()
                            .filter(|id| !live_ids.contains(*id))
                            .cloned()
                            .collect();
                        if !stale.is_empty() {
                            tracing::info!(
                                count = stale.len(),
                                "Pruning stale orders not present on exchange"
                            );
                            for id in &stale {
                                engine.remove_order(id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Periodic order sync failed");
                    }
                }
            }
            _ = rescan_tick.tick() => {
                tracing::info!("Periodic market rescan triggered");
                let cfg = config.read().await;
                if cfg.trading.markets_allowlist.is_empty() {
                    let scanner = market_scanner::MarketScanner::new(&cfg.trading);
                    match scanner.select_markets(
                        &rest_client,
                        max_markets as usize,
                        &cfg.trading.markets_allowlist,
                    ).await {
                        Ok((new_tickers, scored)) => {
                            let mut engine = state_engine.write().await;
                            for ticker in &new_tickers {
                                let mt = types::MarketTicker::from(ticker.as_str());
                                if engine.get_book(&mt).is_none() {
                                    engine.ensure_book(mt.clone());
                                    if let Some(sm) = scored.iter().find(|s| s.ticker == *ticker) {
                                        match rest_client.get_markets(None, Some(1)).await {
                                            Ok(markets) => {
                                                if let Some(m) = markets.into_iter().find(|m| m.ticker == *ticker) {
                                                    let meta = state::MarketMeta::from_market_response(&m, sm.score);
                                                    engine.set_market_meta(mt, meta);
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(error = %e, ticker = %ticker, "Failed to fetch market metadata during rescan");
                                            }
                                        }
                                    }
                                    tracing::info!(ticker = %ticker, "New market added from rescan");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Market rescan failed");
                        }
                    }
                }
            }
            _ = tick.tick() => {
                let cfg = config.read().await;
                if !cfg.trading_enabled() {
                    continue;
                }
                drop(cfg);

                let engine = state_engine.read().await;

                if let Some(reason) = risk_engine.kill_switch_check(&engine) {
                    tracing::error!(reason = %reason, "Kill switch triggered");
                    drop(engine);

                    let engine_w = state_engine.write().await;
                    execution_engine.cancel_all(&engine_w).await;
                    drop(engine_w);

                    let mut bsm = bot_state.write().await;
                    let _ = bsm.transition(
                        BotState::Error,
                        "kill_switch",
                        Some(serde_json::json!({ "message": reason.clone() })),
                    ).await;
                    drop(bsm);

                    let _ = event_broadcast.send(WsEvent::RiskEvent {
                        severity: "critical".to_string(),
                        message: reason,
                    });
                    break;
                }

                if engine.connectivity() == types::ConnectivityState::Disconnected {
                    let elapsed_secs = engine
                        .disconnected_for_secs(chrono::Utc::now())
                        .unwrap_or_default();
                    if !disconnect_pause_logged {
                        tracing::warn!(
                            elapsed_secs,
                            timeout_secs = risk_engine.disconnect_timeout_secs(),
                            "Exchange disconnected; pausing quote generation while reconnecting"
                        );
                        disconnect_pause_logged = true;
                    }
                    drop(engine);
                    continue;
                } else if disconnect_pause_logged {
                    tracing::info!("Exchange reconnected; resuming quote generation");
                    disconnect_pause_logged = false;
                }

                let all_tickers: Vec<types::MarketTicker> = engine.books().keys().cloned().collect();
                for ticker in &all_tickers {
                    if let Some(book) = engine.get_book(ticker) {
                        event_detector.update(ticker, book);
                    }
                }

                let target_quotes = compute_target_quotes(
                    &engine,
                    &fair_value_engine,
                    &strategy_engine,
                    &risk_engine,
                    &event_detector,
                    &cross_market_checker,
                    max_markets,
                    &skip_markets,
                );

                drop(engine);

                let engine = state_engine.read().await;
                let failed_markets = execution_engine
                    .reconcile(&engine, &target_quotes, &risk_engine)
                    .await;
                drop(engine);

                for market in failed_markets {
                    let count = market_failures.entry(market.clone()).or_insert(0);
                    *count += 1;
                    if *count >= BLACKLIST_THRESHOLD {
                        tracing::warn!(
                            market = %market,
                            failures = count,
                            "Blacklisting market after repeated invalid_order errors"
                        );
                        skip_markets.insert(market);
                    }
                }
            }
        }
    }

    ws_handle.abort();
    tracing::info!("Trading loop ended");
}

fn create_rest_client(config: &config::AppConfig) -> Result<exchange::rest::KalshiRestClient> {
    exchange::rest::KalshiRestClient::new(config)
}

async fn reconcile_startup(
    rest_client: &exchange::rest::KalshiRestClient,
    state_engine: &api::SharedState,
    config: &config::AppConfig,
) -> Result<()> {
    tracing::info!("Step 1/4: Fetching account balance...");
    let balance = rest_client
        .get_balance()
        .await
        .context("reconcile step 1: get_balance failed")?;
    tracing::info!(
        available = %balance.available,
        portfolio_value = %balance.portfolio_value,
        "Account balance fetched"
    );

    let mut engine = state_engine.write().await;
    engine.set_balance(balance);

    tracing::info!("Step 2/4: Fetching open orders...");
    let open_orders = rest_client
        .get_orders(Some("resting"))
        .await
        .context("reconcile step 2: get_orders failed")?;
    tracing::info!(count = open_orders.len(), "Reconciled open orders");
    for order in &open_orders {
        engine.upsert_order(order.clone());
    }

    tracing::info!("Step 3/4: Fetching positions...");
    let positions = rest_client
        .get_positions()
        .await
        .context("reconcile step 3: get_positions failed")?;
    tracing::info!(count = positions.len(), "Reconciled positions");
    for pos in positions {
        engine.upsert_position(pos);
    }

    tracing::info!("Step 4/4: Selecting target markets via scanner...");
    let max_markets = config.trading.max_markets_active.max(1) as usize;
    let scanner = market_scanner::MarketScanner::new(&config.trading);
    let (selected_tickers, all_scored) = scanner
        .select_markets(rest_client, max_markets, &config.trading.markets_allowlist)
        .await
        .context("reconcile step 4: market scanning failed")?;

    let all_market_data = rest_client
        .get_all_markets(Some("open"), None, None)
        .await
        .unwrap_or_default();

    let market_data_map: std::collections::HashMap<&str, &exchange::models::MarketResponse> =
        all_market_data
            .iter()
            .map(|m| (m.ticker.as_str(), m))
            .collect();

    for ticker in &selected_tickers {
        let mt = types::MarketTicker::from(ticker.as_str());
        engine.ensure_book(mt.clone());

        let score = all_scored
            .iter()
            .find(|s| s.ticker == *ticker)
            .map(|s| s.score)
            .unwrap_or(0.0);

        if let Some(m) = market_data_map.get(ticker.as_str()) {
            let meta = state::MarketMeta::from_market_response(m, score);
            engine.set_market_meta(mt, meta);
        }
    }

    tracing::info!(
        count = selected_tickers.len(),
        "Target markets selected and metadata loaded"
    );

    Ok(())
}

fn orderbook_data_to_price_levels(levels: &Option<Vec<Vec<String>>>) -> Vec<types::PriceLevel> {
    use rust_decimal::Decimal;
    let Some(v) = levels else { return vec![] };
    v.iter()
        .filter_map(|pair| {
            if pair.len() < 2 {
                return None;
            }
            let price = pair[0].parse::<Decimal>().ok()?;
            let quantity = pair[1].parse::<Decimal>().ok()?;
            Some(types::PriceLevel { price, quantity })
        })
        .collect()
}

fn broadcast_exchange_event(ev: &types::ExchangeEvent, tx: &api::EventBroadcast) {
    match ev {
        types::ExchangeEvent::Fill {
            trade_id,
            order_id,
            market_ticker,
            side,
            price,
            count,
            ..
        } => {
            let _ = tx.send(WsEvent::Fill {
                fill_id: trade_id.clone(),
                order_id: order_id.clone(),
                market_ticker: market_ticker.0.clone(),
                side: side.to_string(),
                price: price.to_string(),
                count: count.to_string(),
            });
        }
        types::ExchangeEvent::OrderUpdate {
            order_id,
            market_ticker,
            status,
            side,
            price,
            ..
        } => {
            let _ = tx.send(WsEvent::OrderUpdate {
                order_id: order_id.clone(),
                market_ticker: market_ticker.0.clone(),
                status: format!("{:?}", status).to_lowercase(),
                side: side.to_string(),
                price: price.to_string(),
            });
        }
        types::ExchangeEvent::Connected => {
            let _ = tx.send(WsEvent::StateChange {
                from: "".to_string(),
                to: "connected".to_string(),
                trigger: "ws_connected".to_string(),
            });
        }
        types::ExchangeEvent::Disconnected => {
            let _ = tx.send(WsEvent::StateChange {
                from: "connected".to_string(),
                to: "disconnected".to_string(),
                trigger: "ws_disconnected".to_string(),
            });
        }
        _ => {}
    }
}

fn compute_target_quotes(
    state: &state::StateEngine,
    fv_engine: &fair_value::FairValueEngine,
    strategy: &strategy::MarketMakerStrategy,
    risk: &risk::RiskEngine,
    event_detector: &event_detector::EventDetector,
    cross_market: &cross_market::CrossMarketChecker,
    max_markets: u32,
    skip_markets: &std::collections::HashSet<types::MarketTicker>,
) -> Vec<types::TargetQuote> {
    let balance = state.balance().clone();
    let all_tickers: Vec<types::MarketTicker> = state.books().keys().cloned().collect();

    let mut pre_adjust: Vec<(types::TargetQuote, rust_decimal::Decimal)> = Vec::new();

    for ticker in &all_tickers {
        if skip_markets.contains(ticker) {
            continue;
        }

        let book = match state.get_book(ticker) {
            Some(b) => b,
            None => continue,
        };

        let position = state.get_position(ticker);
        let meta = state.get_market_meta(ticker);

        let meta = match meta {
            Some(m) => m,
            None => {
                tracing::debug!(market = %ticker, "Skipping market: no metadata loaded");
                continue;
            }
        };

        let hours = meta.hours_to_expiry();
        if hours < 0.5 {
            tracing::debug!(
                market = %ticker,
                hours = hours,
                "Skipping market: too close to expiry or already closed"
            );
            continue;
        }

        let trade_sign = state.recent_trade_sign(ticker);

        let fv = match fv_engine.compute(ticker, book, position, trade_sign, Some(meta)) {
            Some(fv) => fv,
            None => continue,
        };

        let target = strategy.generate_quotes(
            ticker,
            &fv,
            book,
            position,
            Some(meta),
            &balance,
            max_markets,
        );

        if let Some(mut target) = target {
            let mult = event_detector.spread_multiplier(ticker);
            if mult > rust_decimal::Decimal::ONE {
                let level_count = target.yes_bids.len().min(target.yes_asks.len());
                for i in 0..level_count {
                    let mid = (target.yes_bids[i].price + target.yes_asks[i].price)
                        / rust_decimal::Decimal::TWO;
                    let half = (target.yes_asks[i].price - target.yes_bids[i].price)
                        / rust_decimal::Decimal::TWO;
                    let widened_half = half * mult;
                    target.yes_bids[i].price = mid - widened_half;
                    target.yes_asks[i].price = mid + widened_half;
                }
            }

            pre_adjust.push((target, fv.price));
        }
    }

    let raw_quotes: Vec<types::TargetQuote> = pre_adjust.iter().map(|(q, _)| q.clone()).collect();
    let adjusted_quotes = cross_market.adjust_quotes(raw_quotes, state);

    let fv_by_ticker: std::collections::HashMap<&types::MarketTicker, rust_decimal::Decimal> =
        pre_adjust
            .iter()
            .map(|(q, fv)| (&q.market_ticker, *fv))
            .collect();

    let mut quotes = Vec::new();
    for target in adjusted_quotes {
        let fair = fv_by_ticker.get(&target.market_ticker).copied();
        match risk.check_target_quote(&target, state, fair) {
            types::RiskDecision::Approved => {
                quotes.push(target);
            }
            types::RiskDecision::Rejected { reason } => {
                tracing::debug!(
                    market = %target.market_ticker,
                    reason = %reason,
                    "Target quote rejected by risk (post cross-market)"
                );
            }
            types::RiskDecision::KillSwitch { reason } => {
                tracing::error!(
                    market = %target.market_ticker,
                    reason = %reason,
                    "Kill switch from target quote risk check"
                );
            }
        }
    }

    quotes
}
