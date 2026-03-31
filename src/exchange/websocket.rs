use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info, warn};

use crate::config::AppConfig;
use crate::exchange::models::{WsSubscribeCommand, WsSubscribeParams};
use crate::types::{Action, ExchangeEvent, MarketTicker, OrderStatus, Side};

const MAX_RECONNECT_DELAY_SECS: u64 = 5;

/// Commands sent to the WS task to dynamically change subscriptions.
#[derive(Debug, Clone)]
pub enum WsCommand {
    SubscribeMarkets(Vec<String>),
    UnsubscribeAll,
}

pub async fn run_websocket(
    config: AppConfig,
    event_tx: mpsc::Sender<ExchangeEvent>,
    mut cmd_rx: mpsc::Receiver<WsCommand>,
) {
    let mut reconnect_delay = Duration::from_secs(1);
    let mut subscribed_markets: Vec<String> = Vec::new();

    loop {
        info!("Connecting to WebSocket...");
        match connect_and_stream(
            &config,
            &event_tx,
            &mut cmd_rx,
            &mut subscribed_markets,
        )
        .await
        {
            Ok(()) => {
                info!("WebSocket session ended cleanly");
                reconnect_delay = Duration::from_secs(1);
            }
            Err(e) => {
                error!(error = %e, "WebSocket connection/stream error — reconnecting");
                let _ = event_tx.send(ExchangeEvent::Disconnected).await;
            }
        }

        info!(delay_secs = reconnect_delay.as_secs(), "Reconnecting...");
        sleep(reconnect_delay).await;
        reconnect_delay =
            Duration::from_secs((reconnect_delay.as_secs() * 2).min(MAX_RECONNECT_DELAY_SECS));
    }
}

async fn connect_and_stream(
    config: &AppConfig,
    event_tx: &mpsc::Sender<ExchangeEvent>,
    cmd_rx: &mut mpsc::Receiver<WsCommand>,
    subscribed_markets: &mut Vec<String>,
) -> Result<()> {
    let auth = crate::exchange::auth::KalshiAuth::from_config(config)?;

    let ws_url = &config.exchange.ws_url;

    let ws_path = url::Url::parse(ws_url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| "/trade-api/ws/v2".to_string());
    let headers = auth.sign_request("GET", &ws_path);

    let request = tungstenite::http::Request::builder()
        .uri(ws_url)
        .header("KALSHI-ACCESS-KEY", &headers.api_key)
        .header("KALSHI-ACCESS-TIMESTAMP", &headers.timestamp)
        .header("KALSHI-ACCESS-SIGNATURE", &headers.signature)
        .header(
            "Host",
            url::Url::parse(ws_url)
                .map(|u| u.host_str().unwrap_or("").to_string())
                .unwrap_or_default(),
        )
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .context("Failed to build WS request")?;

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("WebSocket connection failed")?;

    info!("WebSocket connected");
    let _ = event_tx.send(ExchangeEvent::Connected).await;

    let (mut write, mut read) = ws_stream.split();

    let mut cmd_id: u64 = 1;

    // Always subscribe to user data channels (fills + order updates)
    let user_data_sub = WsSubscribeCommand {
        id: cmd_id,
        cmd: "subscribe".to_string(),
        params: WsSubscribeParams {
            channels: vec!["fill".to_string(), "user_orders".to_string()],
            market_tickers: None,
        },
    };
    cmd_id += 1;

    let msg = serde_json::to_string(&user_data_sub)?;
    debug!(cmd = %msg, "Sending user data subscribe");
    write
        .send(tungstenite::Message::Text(msg))
        .await
        .context("Failed to send user subscribe")?;

    // Re-subscribe to previously-active markets on reconnect
    if !subscribed_markets.is_empty() {
        let market_data_sub = WsSubscribeCommand {
            id: cmd_id,
            cmd: "subscribe".to_string(),
            params: WsSubscribeParams {
                channels: vec!["orderbook_delta".to_string(), "trade".to_string()],
                market_tickers: Some(subscribed_markets.clone()),
            },
        };
        cmd_id += 1;

        let msg = serde_json::to_string(&market_data_sub)?;
        debug!(cmd = %msg, "Re-subscribing to {} markets after reconnect", subscribed_markets.len());
        write
            .send(tungstenite::Message::Text(msg))
            .await
            .context("Failed to send market data re-subscribe")?;
    }

    let mut seq_tracker: HashMap<u64, u64> = HashMap::new();

    let (ping_tx, mut ping_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            if ping_tx.send(()).await.is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(text))) => {
                        if let Err(e) = handle_ws_message(&text, event_tx, &mut seq_tracker).await {
                            warn!(error = %e, "Failed to process WS message");
                        }
                    }
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        write.send(tungstenite::Message::Pong(data)).await?;
                    }
                    Some(Ok(tungstenite::Message::Close(_))) => {
                        info!("WebSocket closed by server");
                        let _ = event_tx.send(ExchangeEvent::Disconnected).await;
                        return Err(anyhow::anyhow!("WebSocket closed by server"));
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "WebSocket read error");
                        let _ = event_tx.send(ExchangeEvent::Disconnected).await;
                        return Err(anyhow::anyhow!("WebSocket read error: {e}"));
                    }
                    None => {
                        info!("WebSocket stream ended");
                        let _ = event_tx.send(ExchangeEvent::Disconnected).await;
                        return Err(anyhow::anyhow!("WebSocket stream ended unexpectedly"));
                    }
                    _ => {}
                }
            }
            _ = ping_rx.recv() => {
                write.send(tungstenite::Message::Ping(vec![])).await?;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WsCommand::SubscribeMarkets(tickers)) => {
                        if tickers.is_empty() {
                            continue;
                        }
                        info!(count = tickers.len(), "Subscribing to market data channels");
                        let sub = WsSubscribeCommand {
                            id: cmd_id,
                            cmd: "subscribe".to_string(),
                            params: WsSubscribeParams {
                                channels: vec!["orderbook_delta".to_string(), "trade".to_string()],
                                market_tickers: Some(tickers.clone()),
                            },
                        };
                        cmd_id += 1;
                        let msg = serde_json::to_string(&sub)?;
                        write.send(tungstenite::Message::Text(msg)).await
                            .context("Failed to send market subscribe")?;
                        *subscribed_markets = tickers;
                    }
                    Some(WsCommand::UnsubscribeAll) => {
                        if subscribed_markets.is_empty() {
                            continue;
                        }
                        info!(count = subscribed_markets.len(), "Unsubscribing from market data channels");
                        let unsub = WsSubscribeCommand {
                            id: cmd_id,
                            cmd: "unsubscribe".to_string(),
                            params: WsSubscribeParams {
                                channels: vec!["orderbook_delta".to_string(), "trade".to_string()],
                                market_tickers: Some(subscribed_markets.clone()),
                            },
                        };
                        cmd_id += 1;
                        let msg = serde_json::to_string(&unsub)?;
                        if let Err(e) = write.send(tungstenite::Message::Text(msg)).await {
                            warn!(error = %e, "Failed to send market unsubscribe");
                        }
                        subscribed_markets.clear();
                    }
                    None => {
                        info!("WS command channel closed");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

use crate::exchange::models::WsMessage;

async fn handle_ws_message(
    text: &str,
    event_tx: &mpsc::Sender<ExchangeEvent>,
    seq_tracker: &mut HashMap<u64, u64>,
) -> Result<()> {
    let ws_msg: WsMessage = serde_json::from_str(text).context("Failed to parse WS message")?;

    if let (Some(sid), Some(seq)) = (ws_msg.sid, ws_msg.seq) {
        let expected = seq_tracker.entry(sid).or_insert(0);
        if *expected > 0 && seq != *expected + 1 && ws_msg.msg_type != "orderbook_snapshot" {
            warn!(
                sid = sid,
                expected = *expected + 1,
                got = seq,
                "Sequence gap detected — requesting book resync"
            );
            if matches!(ws_msg.msg_type.as_str(), "orderbook_delta") {
                if let Some(ticker) = ws_msg
                    .msg
                    .as_ref()
                    .and_then(|m| m.get("market_ticker"))
                    .and_then(|v| v.as_str())
                {
                    let _ = event_tx
                        .send(ExchangeEvent::BookResyncNeeded {
                            market_ticker: MarketTicker::from(ticker),
                        })
                        .await;
                }
            }
        }
        *expected = seq;
    }

    match ws_msg.msg_type.as_str() {
        "orderbook_snapshot" => {
            if let Some(msg) = ws_msg.msg {
                let ticker = msg
                    .get("market_ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                let yes_bids = parse_price_levels(msg.get("yes_dollars_fp"));
                let no_bids = parse_price_levels(msg.get("no_dollars_fp"));

                event_tx
                    .send(ExchangeEvent::BookSnapshot {
                        market_ticker: MarketTicker::from(ticker),
                        yes_bids,
                        no_bids,
                        seq: ws_msg.seq.unwrap_or(0),
                    })
                    .await?;
            }
        }
        "orderbook_delta" => {
            if let Some(msg) = ws_msg.msg {
                let ticker = msg
                    .get("market_ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let side = match msg.get("side").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("yes") => Side::Yes,
                    _ => Side::No,
                };
                let price = msg
                    .get("price_dollars")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();
                let delta = msg
                    .get("delta_fp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();

                event_tx
                    .send(ExchangeEvent::BookDelta {
                        market_ticker: MarketTicker::from(ticker),
                        side,
                        price,
                        delta,
                        seq: ws_msg.seq.unwrap_or(0),
                    })
                    .await?;
            }
        }
        "trade" => {
            if let Some(msg) = ws_msg.msg {
                let ticker = msg
                    .get("market_ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let price = msg
                    .get("yes_price_dollars")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .or_else(|| {
                        msg.get("yes_price").and_then(|v| {
                            if let Some(s) = v.as_str() {
                                s.parse::<Decimal>().ok()
                            } else {
                                v.as_i64().map(|n| Decimal::new(n, 2))
                            }
                        })
                    })
                    .unwrap_or_default();
                let count = msg
                    .get("count_fp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or(Decimal::ONE);
                let taker_side = match msg.get("taker_side").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("yes") => Side::Yes,
                    _ => Side::No,
                };

                event_tx
                    .send(ExchangeEvent::Trade {
                        market_ticker: MarketTicker::from(ticker),
                        price,
                        count,
                        taker_side,
                        ts: chrono::Utc::now(),
                    })
                    .await?;
            }
        }
        "fill" => {
            if let Some(msg) = ws_msg.msg {
                let trade_id = msg
                    .get("trade_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let order_id = msg
                    .get("order_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let ticker = msg
                    .get("market_ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let side = match msg.get("side").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("yes") => Side::Yes,
                    _ => Side::No,
                };
                let action = match msg.get("action").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("buy") => Action::Buy,
                    _ => Action::Sell,
                };
                let price = msg
                    .get("yes_price_dollars")
                    .or_else(|| msg.get("no_price_dollars"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .or_else(|| {
                        msg.get("yes_price")
                            .or_else(|| msg.get("no_price"))
                            .and_then(|v| v.as_i64())
                            .map(|n| Decimal::new(n, 2))
                    })
                    .unwrap_or_default();
                let count = msg
                    .get("count_fp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();
                let fee = msg
                    .get("fee_cost")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();
                let is_taker = msg
                    .get("is_taker")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                event_tx
                    .send(ExchangeEvent::Fill {
                        trade_id,
                        order_id,
                        market_ticker: MarketTicker::from(ticker),
                        side,
                        action,
                        price,
                        count,
                        fee,
                        is_taker,
                        ts: chrono::Utc::now(),
                    })
                    .await?;
            }
        }
        "user_order" | "user_orders" => {
            if let Some(msg) = ws_msg.msg {
                let order_id = msg
                    .get("order_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let ticker = msg
                    .get("market_ticker")
                    .or_else(|| msg.get("ticker"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let status_str = msg
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("canceled");
                let status = match status_str {
                    "resting" => OrderStatus::Resting,
                    "executed" => OrderStatus::Executed,
                    _ => OrderStatus::Canceled,
                };
                let side = match msg.get("side").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("yes") => Side::Yes,
                    _ => Side::No,
                };
                let action = match msg.get("action").and_then(|v| v.as_str()) {
                    Some(s) if s.eq_ignore_ascii_case("buy") => Action::Buy,
                    _ => Action::Sell,
                };
                let price = msg
                    .get("yes_price_dollars")
                    .or_else(|| msg.get("no_price_dollars"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .or_else(|| {
                        msg.get("yes_price")
                            .or_else(|| msg.get("no_price"))
                            .and_then(|v| v.as_i64())
                            .map(|n| Decimal::new(n, 2))
                    })
                    .unwrap_or_default();
                let remaining = msg
                    .get("remaining_count_fp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();
                let filled = msg
                    .get("fill_count_fp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_default();

                event_tx
                    .send(ExchangeEvent::OrderUpdate {
                        order_id,
                        market_ticker: MarketTicker::from(ticker),
                        status,
                        side,
                        action,
                        price,
                        remaining_count: remaining,
                        fill_count: filled,
                    })
                    .await?;
            }
        }
        "subscribed" | "ok" | "error" => {
            debug!(msg_type = %ws_msg.msg_type, msg = ?ws_msg.msg, "WS control message");
        }
        other => {
            debug!(msg_type = %other, "Unhandled WS message type");
        }
    }

    Ok(())
}

fn parse_price_levels(val: Option<&serde_json::Value>) -> Vec<crate::types::PriceLevel> {
    let arr = match val {
        Some(serde_json::Value::Array(a)) => a,
        _ => return vec![],
    };

    arr.iter()
        .filter_map(|entry| {
            let pair = entry.as_array()?;
            if pair.len() < 2 {
                return None;
            }
            let price = pair[0].as_str()?.parse::<Decimal>().ok()?;
            let quantity = pair[1].as_str()?.parse::<Decimal>().ok()?;
            Some(crate::types::PriceLevel { price, quantity })
        })
        .collect()
}
