use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ── REST API models ──

#[derive(Debug, Clone, Deserialize)]
pub struct BalanceResponse {
    pub balance: i64,
    pub portfolio_value: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketResponse {
    pub ticker: String,
    pub event_ticker: Option<String>,
    pub series_ticker: Option<String>,
    pub title: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub expiration_time: Option<String>,
    #[serde(default)]
    pub price_level_structure: Option<String>,
    #[serde(default)]
    pub fractional_trading_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketsListResponse {
    pub markets: Vec<MarketResponse>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookResponse {
    pub orderbook: OrderbookData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookData {
    #[serde(default)]
    pub yes: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(default)]
    pub no: Option<Vec<Vec<serde_json::Value>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateOrderRequest {
    pub ticker: String,
    pub side: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_fp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes_price: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_price: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes_price_dollars: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_price_dollars: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateOrderResponse {
    pub order: OrderResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    #[serde(default)]
    pub client_order_id: Option<String>,
    pub ticker: String,
    pub side: String,
    pub action: String,
    pub status: String,
    #[serde(rename = "type")]
    pub order_type: Option<String>,
    #[serde(default)]
    pub yes_price: Option<i64>,
    #[serde(default)]
    pub no_price: Option<i64>,
    #[serde(default)]
    pub yes_price_dollars: Option<String>,
    #[serde(default)]
    pub no_price_dollars: Option<String>,
    #[serde(default)]
    pub remaining_count: Option<i64>,
    #[serde(default)]
    pub remaining_count_fp: Option<String>,
    #[serde(default)]
    pub fill_count: Option<i64>,
    #[serde(default)]
    pub fill_count_fp: Option<String>,
    #[serde(default)]
    pub initial_count_fp: Option<String>,
    #[serde(default)]
    pub created_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrdersListResponse {
    pub orders: Vec<OrderResponse>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FillResponse {
    pub trade_id: String,
    pub order_id: String,
    pub ticker: String,
    pub side: String,
    pub action: String,
    #[serde(default)]
    pub yes_price: Option<i64>,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub is_taker: Option<bool>,
    #[serde(default)]
    pub created_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FillsListResponse {
    pub fills: Vec<FillResponse>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionResponse {
    pub ticker: String,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub market_exposure: Option<i64>,
    #[serde(default)]
    pub realized_pnl: Option<i64>,
    #[serde(default)]
    pub total_traded: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionsListResponse {
    pub market_positions: Vec<PositionResponse>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchCancelRequest {
    pub order_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchCancelResponse {
    pub orders: Vec<OrderResponse>,
}

// ── WebSocket message models ──

#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub sid: Option<u64>,
    #[serde(default)]
    pub seq: Option<u64>,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub msg: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsSubscribeCommand {
    pub id: u64,
    pub cmd: String,
    pub params: WsSubscribeParams,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsSubscribeParams {
    pub channels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_tickers: Option<Vec<String>>,
}

// ── Conversion helpers ──

impl OrderResponse {
    pub fn price_dollars(&self) -> Option<Decimal> {
        if let Some(ref p) = self.yes_price_dollars {
            p.parse::<Decimal>().ok()
        } else if let Some(p) = self.yes_price {
            Some(Decimal::new(p, 2))
        } else {
            None
        }
    }

    pub fn remaining_qty(&self) -> Decimal {
        if let Some(ref fp) = self.remaining_count_fp {
            fp.parse::<Decimal>().unwrap_or_default()
        } else if let Some(c) = self.remaining_count {
            Decimal::from(c)
        } else {
            Decimal::ZERO
        }
    }

    pub fn fill_qty(&self) -> Decimal {
        if let Some(ref fp) = self.fill_count_fp {
            fp.parse::<Decimal>().unwrap_or_default()
        } else if let Some(c) = self.fill_count {
            Decimal::from(c)
        } else {
            Decimal::ZERO
        }
    }

    pub fn to_internal_status(&self) -> crate::types::OrderStatus {
        match self.status.as_str() {
            "resting" => crate::types::OrderStatus::Resting,
            "canceled" => crate::types::OrderStatus::Canceled,
            "executed" => crate::types::OrderStatus::Executed,
            _ => crate::types::OrderStatus::Canceled,
        }
    }

    pub fn to_internal_side(&self) -> crate::types::Side {
        match self.side.as_str() {
            "yes" => crate::types::Side::Yes,
            _ => crate::types::Side::No,
        }
    }

    pub fn to_internal_action(&self) -> crate::types::Action {
        match self.action.as_str() {
            "buy" => crate::types::Action::Buy,
            _ => crate::types::Action::Sell,
        }
    }
}
