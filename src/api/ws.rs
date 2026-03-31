use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;

use super::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    #[serde(rename = "state_change")]
    StateChange {
        from: String,
        to: String,
        trigger: String,
    },
    #[serde(rename = "pnl_tick")]
    PnlTick {
        realized_pnl: String,
        unrealized_pnl: String,
        balance: String,
        portfolio_value: String,
        equity: String,
        session_pnl: String,
        daily_pnl: String,
    },
    #[serde(rename = "fill")]
    Fill {
        fill_id: String,
        order_id: String,
        market_ticker: String,
        side: String,
        price: String,
        count: String,
    },
    #[serde(rename = "order_update")]
    OrderUpdate {
        order_id: String,
        market_ticker: String,
        status: String,
        side: String,
        price: String,
    },
    #[serde(rename = "risk_event")]
    RiskEvent { severity: String, message: String },
    #[serde(rename = "config_change")]
    ConfigChange { section: String },
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.event_tx.subscribe();

    // Bootstrap newly connected dashboards with immediate status + pnl data
    // so they reflect an already-running bot without waiting for polling.
    let bootstrap_state = {
        let bot = state.bot_state.read().await;
        WsEvent::StateChange {
            from: "".to_string(),
            to: bot.state().to_string(),
            trigger: "bootstrap".to_string(),
        }
    };
    if let Ok(json) = serde_json::to_string(&bootstrap_state) {
        if sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    let bootstrap_pnl = {
        let engine = state.state_engine.read().await;
        let balance = engine.balance().available;
        let portfolio_value = engine.compute_portfolio_value();
        let equity = balance + portfolio_value;
        WsEvent::PnlTick {
            realized_pnl: engine.session_realized_pnl().to_string(),
            unrealized_pnl: engine.session_unrealized_pnl().to_string(),
            balance: balance.to_string(),
            portfolio_value: portfolio_value.to_string(),
            equity: equity.to_string(),
            session_pnl: engine.session_total_pnl().to_string(),
            daily_pnl: engine.daily_total_pnl().to_string(),
        }
    };
    if let Ok(json) = serde_json::to_string(&bootstrap_pnl) {
        if sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(Message::Ping(data)) => {
                    // pong is handled automatically by axum
                    let _ = data;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
