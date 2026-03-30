mod api;
mod bot_state;
mod config;
mod db;
mod exchange;
mod execution;
mod fair_value;
mod orderbook;
mod risk;
mod state;
mod strategy;
mod types;

use std::sync::Arc;

use anyhow::Result;
use rust_decimal::Decimal;
use std::path::Path;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing_subscriber::{fmt, EnvFilter};

use api::ws::WsEvent;
use bot_state::{BotState, BotStateMachine};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = config::AppConfig::load(Path::new("config/default.yaml"))?;

    if config.logging.json {
        fmt()
            .json()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    } else {
        fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    tracing::info!(
        environment = %config.environment,
        trading_enabled = config.trading_enabled(),
        "Starting Kalshi bot service"
    );

    let db_pool = db::init_pool(&config).await?;
    tracing::info!("Database connected and migrations applied");

    // Load config overrides from DB
    let config = load_config_overrides(config, &db_pool).await;

    let api_port = config.api_port;
    let state_engine = Arc::new(RwLock::new(state::StateEngine::new(db_pool.clone())));
    let bot_state_machine = Arc::new(RwLock::new(BotStateMachine::new(db_pool.clone())));
    let shared_config = Arc::new(RwLock::new(config));

    let (event_broadcast_tx, _) = broadcast::channel::<WsEvent>(1024);
    let event_broadcast = Arc::new(event_broadcast_tx);

    let (bot_cmd_tx, bot_cmd_rx) = mpsc::channel::<api::BotCommand>(32);

    let app_state = api::AppState {
        state_engine: state_engine.clone(),
        bot_state: bot_state_machine.clone(),
        config: shared_config.clone(),
        db_pool: db_pool.clone(),
        event_tx: event_broadcast.clone(),
        bot_cmd_tx,
    };

    // Spawn Axum HTTP server
    let router = api::create_router(app_state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}")).await?;
    tracing::info!(port = api_port, "Axum API server starting");

    let api_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!(error = %e, "Axum server error");
        }
    });

    // Spawn PnL snapshot task
    let pnl_state = state_engine.clone();
    let pnl_pool = db_pool.clone();
    let pnl_broadcast = event_broadcast.clone();
    let pnl_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let engine = pnl_state.read().await;
            let bal = engine.balance();
            let realized = engine.daily_pnl();
            let unrealized = engine
                .positions()
                .values()
                .map(|p| p.unrealized_pnl)
                .sum::<Decimal>();

            let _ = db::insert_pnl_snapshot(
                &pnl_pool,
                realized,
                unrealized,
                bal.available,
                bal.portfolio_value,
                engine.open_order_count() as i32,
                engine.active_market_count() as i32,
            )
            .await;

            let _ = pnl_broadcast.send(WsEvent::PnlTick {
                realized_pnl: realized.to_string(),
                unrealized_pnl: unrealized.to_string(),
                balance: bal.available.to_string(),
                portfolio_value: bal.portfolio_value.to_string(),
            });
        }
    });

    // Main bot control loop
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

async fn load_config_overrides(mut config: config::AppConfig, pool: &sqlx::PgPool) -> config::AppConfig {
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
                        // Force to stopped regardless of current state
                        bsm.transition(BotState::Stopped, "kill_switch", None).await.ok();
                        drop(bsm);

                        let _ = event_broadcast.send(WsEvent::StateChange {
                            from,
                            to: "stopped".to_string(),
                            trigger: "kill_switch".to_string(),
                        });
                    }
                    Some(api::BotCommand::SwitchEnvironment { environment }) => {
                        // Stop trading first
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

                        // Update environment in config
                        let mut cfg = config.write().await;
                        cfg.environment = environment.clone();
                        drop(cfg);

                        // Clear state
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

    // Create REST client
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

    // Startup reconciliation
    match reconcile_startup(&rest_client, &state_engine, &cfg).await {
        Ok(()) => {
            tracing::info!("Startup reconciliation complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Startup reconciliation failed");
            let mut bsm = bot_state.write().await;
            let _ = bsm
                .transition(
                    BotState::Error,
                    "reconciliation_failed",
                    Some(serde_json::json!({ "message": e.to_string() })),
                )
                .await;
            return;
        }
    }

    // Transition to Running
    {
        let mut bsm = bot_state.write().await;
        if bsm.transition(BotState::Running, "reconciliation_complete", None).await.is_err() {
            return;
        }
    }

    let _ = event_broadcast.send(WsEvent::StateChange {
        from: "starting".to_string(),
        to: "running".to_string(),
        trigger: "reconciliation_complete".to_string(),
    });

    // Spawn WebSocket
    let (event_tx, mut event_rx) = mpsc::channel::<types::ExchangeEvent>(4096);
    let ws_config = cfg.clone();
    let target_markets = {
        let engine = state_engine.read().await;
        engine.books().keys().map(|k| k.0.clone()).collect::<Vec<_>>()
    };
    let ws_markets = target_markets.clone();
    let ws_handle = tokio::spawn(async move {
        exchange::websocket::run_websocket(ws_config, ws_markets, event_tx).await
    });

    let risk_engine = risk::RiskEngine::new(&cfg.risk);
    let strategy_engine = strategy::MarketMakerStrategy::new(&cfg.strategy);
    let mut execution_engine =
        execution::ExecutionEngine::new(rest_client.clone(), db_pool.clone(), &cfg.strategy);
    let fair_value_engine = fair_value::FairValueEngine::new(&cfg.strategy);

    let tick_interval = tokio::time::Duration::from_millis(cfg.strategy.tick_interval_ms);
    let mut tick = tokio::time::interval(tick_interval);

    tracing::info!("Trading loop started");

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
            event = event_rx.recv() => {
                match event {
                    Some(ev) => {
                        broadcast_exchange_event(&ev, &event_broadcast);
                        let mut engine = state_engine.write().await;
                        engine.process_event(ev).await;
                    }
                    None => {
                        tracing::error!("Event channel closed in trading loop");
                        break;
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

                let target_quotes = compute_target_quotes(
                    &engine,
                    &fair_value_engine,
                    &strategy_engine,
                    &risk_engine,
                    &target_markets,
                );

                drop(engine);

                let engine = state_engine.read().await;
                execution_engine
                    .reconcile(&engine, &target_quotes)
                    .await;
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
    let balance = rest_client.get_balance().await?;
    tracing::info!(
        available = %balance.available,
        portfolio_value = %balance.portfolio_value,
        "Account balance fetched"
    );

    let mut engine = state_engine.write().await;
    engine.set_balance(balance);

    let open_orders = rest_client.get_orders(Some("resting")).await?;
    tracing::info!(count = open_orders.len(), "Reconciled open orders");
    for order in &open_orders {
        engine.upsert_order(order.clone());
    }

    let positions = rest_client.get_positions().await?;
    tracing::info!(count = positions.len(), "Reconciled positions");
    for pos in positions {
        engine.upsert_position(pos);
    }

    // Set up target markets
    let target_markets = if config.trading.markets_allowlist.is_empty() {
        let markets = rest_client.get_markets(Some("open"), Some(30)).await?;
        markets.into_iter().map(|m| m.ticker).collect::<Vec<_>>()
    } else {
        config.trading.markets_allowlist.clone()
    };

    tracing::info!(count = target_markets.len(), "Target markets selected");
    for ticker in &target_markets {
        engine.ensure_book(types::MarketTicker::from(ticker.as_str()));
    }

    Ok(())
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
    markets: &[String],
) -> Vec<types::TargetQuote> {
    let mut quotes = Vec::new();
    for ticker_str in markets {
        let ticker = types::MarketTicker::from(ticker_str.as_str());
        let book = match state.get_book(&ticker) {
            Some(b) => b,
            None => continue,
        };

        let position = state.get_position(&ticker);
        let fv = match fv_engine.compute(book, position) {
            Some(fv) => fv,
            None => continue,
        };

        let target = strategy.generate_quotes(&ticker, &fv, book, position);
        if let Some(target) = target {
            let _ = risk.check_target_quote(&target, state);
            quotes.push(target);
        }
    }
    quotes
}
