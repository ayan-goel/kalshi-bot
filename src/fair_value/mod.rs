use rust_decimal::Decimal;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::orderbook::OrderBook;
use crate::types::{FairValue, MarketTicker, Position};

/// Fair value engine V1: microstructure-driven estimates.
///
/// Formula:
///   raw_fair = microprice + a1 * order_imbalance + a2 * recent_trade_sign
///   fair = clamp(raw_fair + inventory_adj, 0.01, 0.99)
///
/// Where inventory_adj = -k1 * normalized_inventory - k3 * inventory^3
pub struct FairValueEngine {
    order_imbalance_alpha: Decimal,
    trade_sign_alpha: Decimal,
    inventory_penalty_k1: Decimal,
    inventory_penalty_k3: Decimal,
}

impl FairValueEngine {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            order_imbalance_alpha: config.order_imbalance_alpha,
            trade_sign_alpha: config.trade_sign_alpha,
            inventory_penalty_k1: config.inventory_penalty_k1,
            inventory_penalty_k3: config.inventory_penalty_k3,
        }
    }

    /// Compute fair value for a market. Returns None if the book is too thin.
    pub fn compute(&self, book: &OrderBook, position: Option<&Position>) -> Option<FairValue> {
        if book.is_empty() {
            return None;
        }

        let microprice = book.microprice().or_else(|| book.mid())?;
        let imbalance = book.order_imbalance().unwrap_or(Decimal::ZERO);

        let imbalance_adj = self.order_imbalance_alpha * imbalance;

        // Inventory adjustment
        let inventory = position
            .map(|p| p.net_inventory())
            .unwrap_or(Decimal::ZERO);

        let inv_adj = -self.inventory_penalty_k1 * inventory
            - self.inventory_penalty_k3 * inventory * inventory * inventory;

        let raw_fair = microprice + imbalance_adj + inv_adj;

        let min_price = Decimal::new(1, 2); // 0.01
        let max_price = Decimal::new(99, 2); // 0.99
        let fair = raw_fair.max(min_price).min(max_price);

        let spread = book.spread().unwrap_or(Decimal::ONE);
        let confidence = if spread > Decimal::ZERO {
            let conf = Decimal::ONE / (Decimal::ONE + spread * Decimal::from(10));
            // Convert to f64 for the confidence field
            conf.to_string().parse::<f64>().unwrap_or(0.5)
        } else {
            0.5
        };

        debug!(
            microprice = %microprice,
            imbalance = %imbalance,
            inventory = %inventory,
            fair = %fair,
            confidence = confidence,
            "Fair value computed"
        );

        Some(FairValue {
            market_ticker: MarketTicker::from(""),
            price: fair,
            confidence,
        })
    }
}
