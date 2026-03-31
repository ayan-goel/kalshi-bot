use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::ws::WsEvent;
use crate::api::{AppState, BotCommand};
use crate::bot_state::BotState;

// ── Health ──

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ── Status ──

#[derive(Serialize)]
struct StatusResponse {
    state: String,
    environment: String,
    uptime_secs: Option<i64>,
    connectivity: String,
    active_markets: usize,
    open_orders: usize,
    trading_enabled: bool,
    error_message: Option<String>,
}

pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let bot = state.bot_state.read().await;
    let engine = state.state_engine.read().await;
    let config = state.config.read().await;

    let uptime = bot
        .started_at()
        .map(|t| (chrono::Utc::now() - t).num_seconds());

    Json(StatusResponse {
        state: bot.state().to_string(),
        environment: config.environment.clone(),
        uptime_secs: uptime,
        connectivity: format!("{:?}", engine.connectivity()),
        active_markets: engine.active_market_count(),
        open_orders: engine.open_order_count(),
        trading_enabled: config.trading_enabled(),
        error_message: bot.error_message().map(|s| s.to_string()),
    })
}

// ── Bot control ──

pub async fn bot_start(State(state): State<AppState>) -> impl IntoResponse {
    let bot = state.bot_state.read().await;
    if bot.state() != BotState::Stopped {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": format!("Cannot start: bot is {}", bot.state()) })),
        );
    }
    drop(bot);

    if state.bot_cmd_tx.send(BotCommand::Start).await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to send start command" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "starting" })),
    )
}

pub async fn bot_stop(State(state): State<AppState>) -> impl IntoResponse {
    let bot = state.bot_state.read().await;
    let current = bot.state();
    if current != BotState::Running && current != BotState::Error {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": format!("Cannot stop: bot is {}", current) })),
        );
    }
    drop(bot);

    if state.bot_cmd_tx.send(BotCommand::Stop).await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to send stop command" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "stopping" })),
    )
}

pub async fn bot_kill(State(state): State<AppState>) -> impl IntoResponse {
    if state.bot_cmd_tx.send(BotCommand::Kill).await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to send kill command" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "killing" })),
    )
}

// ── Environment ──

pub async fn get_environment(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    Json(serde_json::json!({
        "environment": config.environment,
        "is_demo": config.is_demo(),
    }))
}

#[derive(Deserialize)]
pub struct SetEnvironmentRequest {
    environment: String,
    #[serde(default)]
    confirm: Option<String>,
}

pub async fn set_environment(
    State(state): State<AppState>,
    Json(req): Json<SetEnvironmentRequest>,
) -> impl IntoResponse {
    if req.environment != "demo" && req.environment != "production" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "environment must be 'demo' or 'production'" })),
        );
    }

    if req.environment == "production" {
        match &req.confirm {
            Some(c) if c == "CONFIRM" => {}
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Switching to production requires confirm: 'CONFIRM'"
                    })),
                );
            }
        }
    }

    if state
        .bot_cmd_tx
        .send(BotCommand::SwitchEnvironment {
            environment: req.environment.clone(),
        })
        .await
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to send switch command" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "switching", "environment": req.environment })),
    )
}

// ── Config ──

pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    Json(serde_json::json!({
        "strategy": config.strategy,
        "risk": config.risk,
        "trading": config.trading,
    }))
}

pub async fn put_config_strategy(
    State(state): State<AppState>,
    Json(strategy): Json<crate::config::StrategyConfig>,
) -> impl IntoResponse {
    let value = match serde_json::to_value(&strategy) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    if let Err(e) = crate::db::set_config(&state.db_pool, "strategy", &value).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    state.config.write().await.strategy = strategy;
    let _ = state.event_tx.send(WsEvent::ConfigChange {
        section: "strategy".to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated" })),
    )
}

pub async fn put_config_risk(
    State(state): State<AppState>,
    Json(risk): Json<crate::config::RiskConfig>,
) -> impl IntoResponse {
    let value = match serde_json::to_value(&risk) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    if let Err(e) = crate::db::set_config(&state.db_pool, "risk", &value).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    state.config.write().await.risk = risk;
    let _ = state.event_tx.send(WsEvent::ConfigChange {
        section: "risk".to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated" })),
    )
}

pub async fn put_config_trading(
    State(state): State<AppState>,
    Json(trading): Json<crate::config::TradingConfig>,
) -> impl IntoResponse {
    let value = match serde_json::to_value(&trading) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    if let Err(e) = crate::db::set_config(&state.db_pool, "trading", &value).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    state.config.write().await.trading = trading;
    let _ = state.event_tx.send(WsEvent::ConfigChange {
        section: "trading".to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated" })),
    )
}

// ── Data endpoints ──

#[derive(Serialize)]
struct MarketSummary {
    ticker: String,
    mid: Option<String>,
    spread: Option<String>,
    best_bid: Option<String>,
    best_ask: Option<String>,
    position: Option<PositionSummary>,
    score: Option<f64>,
    event_ticker: Option<String>,
    category: Option<String>,
    hours_to_expiry: Option<f64>,
    volume_24h: Option<f64>,
}

#[derive(Serialize)]
struct PositionSummary {
    yes_contracts: String,
    no_contracts: String,
    net_inventory: String,
    realized_pnl: String,
}

pub async fn get_markets(State(state): State<AppState>) -> impl IntoResponse {
    let engine = state.state_engine.read().await;
    let markets: Vec<MarketSummary> = engine
        .books()
        .iter()
        .map(|(ticker, book)| {
            let pos = engine.get_position(ticker).map(|p| PositionSummary {
                yes_contracts: p.yes_contracts.to_string(),
                no_contracts: p.no_contracts.to_string(),
                net_inventory: p.net_inventory().to_string(),
                realized_pnl: p.realized_pnl.to_string(),
            });
            let meta = engine.get_market_meta(ticker);
            MarketSummary {
                ticker: ticker.0.clone(),
                mid: book.mid().map(|d| d.to_string()),
                spread: book.spread().map(|d| d.to_string()),
                best_bid: book.best_yes_bid().map(|d| d.price.to_string()),
                best_ask: book.implied_yes_ask().map(|d| d.to_string()),
                position: pos,
                score: meta.map(|m| m.score),
                event_ticker: meta.and_then(|m| m.event_ticker.clone()),
                category: meta.and_then(|m| m.category.clone()),
                hours_to_expiry: meta.map(|m| m.hours_to_expiry()),
                volume_24h: meta.map(|m| m.volume_24h),
            }
        })
        .collect();

    Json(markets)
}

pub async fn get_market_detail(
    State(state): State<AppState>,
    Path(ticker): Path<String>,
) -> impl IntoResponse {
    let engine = state.state_engine.read().await;
    let mt = crate::types::MarketTicker::from(ticker.as_str());

    let book = match engine.get_book(&mt) {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Market not found" })),
            );
        }
    };

    let pos = engine.get_position(&mt);
    let orders = engine.orders_for_market(&mt);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ticker": ticker,
            "mid": book.mid().map(|d| d.to_string()),
            "spread": book.spread().map(|d| d.to_string()),
            "best_bid": book.best_yes_bid().map(|d| d.price.to_string()),
            "best_ask": book.implied_yes_ask().map(|d| d.to_string()),
            "microprice": book.microprice().map(|d| d.to_string()),
            "position": pos.map(|p| serde_json::json!({
                "yes_contracts": p.yes_contracts.to_string(),
                "no_contracts": p.no_contracts.to_string(),
                "net_inventory": p.net_inventory().to_string(),
                "realized_pnl": p.realized_pnl.to_string(),
            })),
            "open_orders": orders.iter().map(|o| serde_json::json!({
                "order_id": o.order_id,
                "side": o.side.to_string(),
                "action": o.action.to_string(),
                "price": o.price.to_string(),
                "remaining_count": o.remaining_count.to_string(),
            })).collect::<Vec<_>>(),
        })),
    )
}

pub async fn get_orders(State(state): State<AppState>) -> impl IntoResponse {
    let engine = state.state_engine.read().await;
    let orders: Vec<serde_json::Value> = engine
        .open_orders()
        .values()
        .map(|o| {
            serde_json::json!({
                "order_id": o.order_id,
                "market_ticker": o.market_ticker.0,
                "side": o.side.to_string(),
                "action": o.action.to_string(),
                "price": o.price.to_string(),
                "remaining_count": o.remaining_count.to_string(),
                "fill_count": o.fill_count.to_string(),
                "status": format!("{:?}", o.status).to_lowercase(),
            })
        })
        .collect();

    Json(orders)
}

pub async fn get_positions(State(state): State<AppState>) -> impl IntoResponse {
    let engine = state.state_engine.read().await;
    let positions: Vec<serde_json::Value> = engine
        .positions()
        .iter()
        .map(|(ticker, p)| {
            serde_json::json!({
                "market_ticker": ticker.0,
                "yes_contracts": p.yes_contracts.to_string(),
                "no_contracts": p.no_contracts.to_string(),
                "net_inventory": p.net_inventory().to_string(),
                "realized_pnl": p.realized_pnl.to_string(),
                "unrealized_pnl": p.unrealized_pnl.to_string(),
            })
        })
        .collect();

    Json(positions)
}

pub async fn get_balance(State(state): State<AppState>) -> impl IntoResponse {
    let engine = state.state_engine.read().await;
    let available = engine.balance().available;
    // portfolio_value is computed from live positions × mid-prices
    // (not from Kalshi's /portfolio/balance which doesn't include it)
    let portfolio_value = engine.compute_portfolio_value();
    let total_reserved = engine.total_reserved();
    Json(serde_json::json!({
        "available": available.to_string(),
        "portfolio_value": portfolio_value.to_string(),
        "total_reserved": total_reserved.to_string(),
    }))
}

#[derive(Deserialize)]
pub struct PnlQuery {
    #[serde(default)]
    pub window: Option<String>,
}

#[derive(Serialize)]
struct PnlBreakdownResponse {
    pnl: String,
    realized_pnl: String,
    unrealized_pnl: String,
}

#[derive(Serialize)]
struct PnlComponentsResponse {
    cash: String,
    position_value: String,
    equity: String,
}

#[derive(Serialize)]
struct PnlSnapshotResponse {
    ts: chrono::DateTime<chrono::Utc>,
    realized_pnl: String,
    unrealized_pnl: String,
    balance: String,
    portfolio_value: String,
    equity: String,
    session_pnl: String,
    session_realized_pnl: String,
    session_unrealized_pnl: String,
    daily_pnl: String,
    daily_realized_pnl: String,
    daily_unrealized_pnl: String,
    open_order_count: i32,
    active_market_count: i32,
}

#[derive(Serialize)]
struct PnlResponse {
    window: String,
    session_started_at: Option<chrono::DateTime<chrono::Utc>>,
    session: PnlBreakdownResponse,
    daily: PnlBreakdownResponse,
    components: PnlComponentsResponse,
    snapshots: Vec<PnlSnapshotResponse>,
}

fn pnl_window_cutoff(window: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now();
    match window {
        "30m" => Some(now - chrono::Duration::minutes(30)),
        "1h" => Some(now - chrono::Duration::hours(1)),
        "4h" => Some(now - chrono::Duration::hours(4)),
        "1d" => Some(now - chrono::Duration::days(1)),
        _ => None,
    }
}

pub async fn get_pnl(
    State(state): State<AppState>,
    Query(q): Query<PnlQuery>,
) -> impl IntoResponse {
    let window = q.window.unwrap_or_else(|| "all".to_string()).to_lowercase();
    let since = pnl_window_cutoff(&window);

    let engine = state.state_engine.read().await;
    let cash = engine.balance().available;
    let position_value = engine.compute_portfolio_value();
    let equity = cash + position_value;

    let session_realized = engine.session_realized_pnl();
    let session_unrealized = engine.session_unrealized_pnl();
    let session_pnl = engine.session_total_pnl();

    let daily_realized = engine.daily_realized_pnl();
    let daily_unrealized = engine.daily_unrealized_pnl();
    let daily_pnl = engine.daily_total_pnl();

    let session_started_at = engine.session_started_at();
    drop(engine);

    let snapshots = crate::db::get_pnl_snapshots(&state.db_pool, 2000, since)
        .await
        .unwrap_or_default();

    let snapshots: Vec<PnlSnapshotResponse> = snapshots
        .into_iter()
        .map(|s| PnlSnapshotResponse {
            ts: s.ts,
            realized_pnl: s.realized_pnl.to_string(),
            unrealized_pnl: s.unrealized_pnl.to_string(),
            balance: s.balance.to_string(),
            portfolio_value: s.portfolio_value.to_string(),
            equity: s.equity.to_string(),
            session_pnl: s.session_pnl.to_string(),
            session_realized_pnl: s.realized_pnl.to_string(),
            session_unrealized_pnl: s.unrealized_pnl.to_string(),
            daily_pnl: s.daily_pnl.to_string(),
            daily_realized_pnl: s.daily_realized_pnl.to_string(),
            daily_unrealized_pnl: s.daily_unrealized_pnl.to_string(),
            open_order_count: s.open_order_count,
            active_market_count: s.active_market_count,
        })
        .collect();

    Json(PnlResponse {
        window,
        session_started_at,
        session: PnlBreakdownResponse {
            pnl: session_pnl.to_string(),
            realized_pnl: session_realized.to_string(),
            unrealized_pnl: session_unrealized.to_string(),
        },
        daily: PnlBreakdownResponse {
            pnl: daily_pnl.to_string(),
            realized_pnl: daily_realized.to_string(),
            unrealized_pnl: daily_unrealized.to_string(),
        },
        components: PnlComponentsResponse {
            cash: cash.to_string(),
            position_value: position_value.to_string(),
            equity: equity.to_string(),
        },
        snapshots,
    })
}

#[derive(Deserialize)]
pub struct LimitQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

fn default_offset() -> i64 {
    0
}

fn clamp_limit(limit: i64) -> i64 {
    limit.clamp(1, 1000)
}

pub async fn get_fills(
    State(state): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    let fills = crate::db::get_recent_fills(&state.db_pool, clamp_limit(q.limit))
        .await
        .unwrap_or_default();
    Json(fills)
}

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

#[derive(Serialize)]
pub struct PageResponse<T> {
    pub items: Vec<T>,
    pub limit: i64,
    pub offset: i64,
    pub next_offset: Option<i64>,
}

pub async fn get_risk_events(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let limit = clamp_limit(q.limit);
    let offset = q.offset.max(0);
    let events = crate::db::get_risk_events(&state.db_pool, limit, offset)
        .await
        .unwrap_or_default();
    let next_offset = if events.len() as i64 == limit {
        Some(offset + limit)
    } else {
        None
    };
    Json(PageResponse {
        items: events,
        limit,
        offset,
        next_offset,
    })
}

pub async fn get_strategy_decisions(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let limit = clamp_limit(q.limit);
    let offset = q.offset.max(0);
    let decisions = crate::db::get_strategy_decisions(&state.db_pool, limit, offset)
        .await
        .unwrap_or_default();
    let next_offset = if decisions.len() as i64 == limit {
        Some(offset + limit)
    } else {
        None
    };
    Json(PageResponse {
        items: decisions,
        limit,
        offset,
        next_offset,
    })
}

#[derive(Deserialize)]
pub struct RawLogsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub before_id: Option<u64>,
}

#[derive(Serialize)]
pub struct RawLogsResponse {
    pub items: Vec<crate::log_buffer::RawLogEntry>,
    pub limit: i64,
    pub next_before_id: Option<u64>,
}

pub async fn get_raw_logs(
    State(state): State<AppState>,
    Query(q): Query<RawLogsQuery>,
) -> impl IntoResponse {
    let limit = clamp_limit(q.limit) as usize;
    let items = match q.before_id {
        Some(before_id) => state.log_buffer.before(before_id, limit),
        None => state.log_buffer.latest(limit),
    };
    let next_before_id = items.last().map(|entry| entry.id);

    Json(RawLogsResponse {
        items,
        limit: limit as i64,
        next_before_id,
    })
}
