pub mod routes;
pub mod ws;

use std::sync::Arc;
use tokio::sync::RwLock;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use crate::bot_state::BotStateMachine;
use crate::config::AppConfig;
use crate::state::StateEngine;

pub type SharedState = Arc<RwLock<StateEngine>>;
pub type SharedBotState = Arc<RwLock<BotStateMachine>>;
pub type SharedConfig = Arc<RwLock<AppConfig>>;
pub type EventBroadcast = Arc<tokio::sync::broadcast::Sender<ws::WsEvent>>;

#[derive(Clone)]
pub struct AppState {
    pub state_engine: SharedState,
    pub bot_state: SharedBotState,
    pub config: SharedConfig,
    pub db_pool: sqlx::PgPool,
    pub event_tx: EventBroadcast,
    pub bot_cmd_tx: tokio::sync::mpsc::Sender<BotCommand>,
    pub log_buffer: crate::log_buffer::LogBuffer,
    pub api_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub enum BotCommand {
    Start,
    Stop,
    Kill,
    SwitchEnvironment { environment: String },
}

async fn auth_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let secret = req.extensions().get::<Option<String>>().cloned().flatten();

    if let Some(expected) = secret {
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);

        // Also check query param for WebSocket connections (browsers can't set WS headers)
        let query_token = req
            .uri()
            .query()
            .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("token=")))
            .unwrap_or("");

        if token != expected && query_token != expected {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(req).await)
}

pub fn create_router(app_state: AppState) -> axum::Router {
    use axum::routing::{get, post, put};
    use tower_http::cors::CorsLayer;

    let secret = app_state.api_secret.clone();

    // Health endpoint is outside auth — Railway needs it for health checks
    let health_route = axum::Router::new().route("/api/health", get(routes::health));

    let protected_routes = axum::Router::new()
        .route("/api/status", get(routes::status))
        .route("/api/bot/start", post(routes::bot_start))
        .route("/api/bot/stop", post(routes::bot_stop))
        .route("/api/bot/kill", post(routes::bot_kill))
        .route("/api/environment", get(routes::get_environment))
        .route("/api/environment", post(routes::set_environment))
        .route("/api/config", get(routes::get_config))
        .route("/api/config/strategy", put(routes::put_config_strategy))
        .route("/api/config/risk", put(routes::put_config_risk))
        .route("/api/config/trading", put(routes::put_config_trading))
        .route("/api/markets", get(routes::get_markets))
        .route("/api/markets/{ticker}", get(routes::get_market_detail))
        .route("/api/orders", get(routes::get_orders))
        .route("/api/positions", get(routes::get_positions))
        .route("/api/balance", get(routes::get_balance))
        .route("/api/pnl", get(routes::get_pnl))
        .route("/api/fills", get(routes::get_fills))
        .route("/api/risk-events", get(routes::get_risk_events))
        .route(
            "/api/strategy-decisions",
            get(routes::get_strategy_decisions),
        )
        .route("/api/raw-logs", get(routes::get_raw_logs))
        .route("/api/ws", get(ws::ws_handler))
        .layer(axum::middleware::from_fn(
            move |req: Request, next: Next| {
                let s = secret.clone();
                async move {
                    let mut req = req;
                    req.extensions_mut().insert(s);
                    auth_middleware(req, next).await
                }
            },
        ));

    health_route
        .merge(protected_routes)
        .layer(CorsLayer::permissive())
        .with_state(app_state)
}
