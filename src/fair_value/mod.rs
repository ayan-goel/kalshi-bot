use chrono::Utc;
use rust_decimal::Decimal;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::orderbook::OrderBook;
use crate::state::MarketMeta;
use crate::types::{FairValue, MarketTicker, Position};

/// Fair value engine V2: microstructure + trade flow + time-to-expiry.
///
/// Formula:
///   raw_fair = microprice
///            + a1 * order_imbalance
///            + a2 * recent_trade_sign
///            + inventory_adj (-k1*inv - k3*inv^3)
///
///   fair = clamp(raw_fair, tick_min, tick_max)
///
/// Confidence combines spread, volume, staleness, and trade flow.
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
    pub fn compute(
        &self,
        ticker: &MarketTicker,
        book: &OrderBook,
        position: Option<&Position>,
        trade_sign: Decimal,
        meta: Option<&MarketMeta>,
    ) -> Option<FairValue> {
        if book.is_empty() {
            return None;
        }

        let microprice = book.microprice().or_else(|| book.mid())?;
        let imbalance = book.order_imbalance().unwrap_or(Decimal::ZERO);

        let imbalance_adj = self.order_imbalance_alpha * imbalance;
        let trade_sign_adj = self.trade_sign_alpha * trade_sign;

        let inventory = position.map(|p| p.net_inventory()).unwrap_or(Decimal::ZERO);

        let inv_adj = -self.inventory_penalty_k1 * inventory
            - self.inventory_penalty_k3 * inventory * inventory * inventory;

        let raw_fair = microprice + imbalance_adj + trade_sign_adj + inv_adj;

        let (min_price, max_price) = match meta {
            Some(m) => (m.tick_min, m.tick_max),
            None => (Decimal::new(1, 2), Decimal::new(99, 2)),
        };
        let fair = raw_fair.max(min_price).min(max_price);

        let confidence = self.compute_confidence(book, meta, imbalance);

        debug!(
            market = %ticker,
            microprice = %microprice,
            imbalance = %imbalance,
            trade_sign = %trade_sign,
            inventory = %inventory,
            fair = %fair,
            confidence = confidence,
            "Fair value computed"
        );

        Some(FairValue {
            market_ticker: ticker.clone(),
            price: fair,
            confidence,
        })
    }

    fn compute_confidence(
        &self,
        book: &OrderBook,
        meta: Option<&MarketMeta>,
        imbalance: Decimal,
    ) -> f64 {
        let spread = book.spread().unwrap_or(Decimal::ONE);

        // Base: spread-based (tighter = higher confidence)
        let spread_f = spread.to_string().parse::<f64>().unwrap_or(1.0);
        let base_conf = 1.0 / (1.0 + spread_f * 10.0);

        // Volume factor: higher volume = more reliable signal
        let volume_factor = match meta {
            Some(m) if m.volume_24h > 0.0 => ((1.0 + m.volume_24h).ln() / 8.0).min(1.0),
            _ => 0.5,
        };

        // Staleness factor: decays if book hasn't been updated recently
        let staleness_factor = {
            let age_secs = (Utc::now() - book.last_update).num_seconds() as f64;
            if age_secs < 5.0 {
                1.0
            } else {
                (1.0 / (1.0 + (age_secs - 5.0) / 30.0)).max(0.1)
            }
        };

        // Imbalance factor: extreme imbalance = less reliable for MM
        let imb_f = imbalance.to_string().parse::<f64>().unwrap_or(0.0).abs();
        let imbalance_factor = 1.0 - (imb_f * 0.3).min(0.5);

        let confidence = base_conf * volume_factor * staleness_factor * imbalance_factor;
        confidence.max(0.01).min(1.0)
    }
}
