use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketTicker(pub String);

impl fmt::Display for MarketTicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MarketTicker {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MarketTicker {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Yes,
    No,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Yes => write!(f, "yes"),
            Side::No => write!(f, "no"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Buy,
    Sell,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Buy => write!(f, "buy"),
            Action::Sell => write!(f, "sell"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Resting,
    Canceled,
    Executed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    FillOrKill,
    GoodTillCanceled,
    ImmediateOrCancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_ticker: MarketTicker,
    pub yes_contracts: Decimal,
    pub no_contracts: Decimal,
    pub avg_yes_price: Option<Decimal>,
    pub avg_no_price: Option<Decimal>,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
}

impl Position {
    pub fn net_inventory(&self) -> Decimal {
        self.yes_contracts - self.no_contracts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub available: Decimal,
    pub portfolio_value: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityState {
    Connected,
    Disconnected,
    Reconnecting,
}

/// Internal event types emitted by the WebSocket client
#[derive(Debug, Clone)]
pub enum ExchangeEvent {
    BookSnapshot {
        market_ticker: MarketTicker,
        yes_bids: Vec<PriceLevel>,
        no_bids: Vec<PriceLevel>,
        seq: u64,
    },
    BookDelta {
        market_ticker: MarketTicker,
        side: Side,
        price: Decimal,
        delta: Decimal,
        seq: u64,
    },
    Trade {
        market_ticker: MarketTicker,
        price: Decimal,
        count: Decimal,
        taker_side: Side,
        ts: DateTime<Utc>,
    },
    Fill {
        trade_id: String,
        order_id: String,
        market_ticker: MarketTicker,
        side: Side,
        action: Action,
        price: Decimal,
        count: Decimal,
        fee: Decimal,
        is_taker: bool,
        ts: DateTime<Utc>,
    },
    OrderUpdate {
        order_id: String,
        market_ticker: MarketTicker,
        status: OrderStatus,
        side: Side,
        action: Action,
        price: Decimal,
        remaining_count: Decimal,
        fill_count: Decimal,
    },
    Connected,
    Disconnected,
    /// Emitted when a WS sequence gap is detected for a market's orderbook.
    /// The trading loop handles this by fetching a REST snapshot and re-applying it.
    BookResyncNeeded {
        market_ticker: MarketTicker,
    },
}

/// Desired action emitted by the strategy engine
#[derive(Debug, Clone)]
pub enum DesiredAction {
    CreateOrder {
        market_ticker: MarketTicker,
        side: Side,
        action: Action,
        price: Decimal,
        quantity: Decimal,
        client_order_id: String,
    },
    CancelOrder {
        order_id: String,
        market_ticker: MarketTicker,
    },
}

/// Output of the strategy: what quotes we want live (multi-level)
#[derive(Debug, Clone)]
pub struct TargetQuote {
    pub market_ticker: MarketTicker,
    pub yes_bids: Vec<PriceLevel>,
    pub yes_asks: Vec<PriceLevel>,
    pub reason: String,
}

/// Fair value estimate for a market
#[derive(Debug, Clone)]
pub struct FairValue {
    pub market_ticker: MarketTicker,
    pub price: Decimal,
    pub confidence: f64,
}

/// Risk decision
#[derive(Debug, Clone)]
pub enum RiskDecision {
    Approved,
    Rejected { reason: String },
    KillSwitch { reason: String },
}
