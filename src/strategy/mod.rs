use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::orderbook::OrderBook;
use crate::types::*;

/// Inventory-aware market-making strategy.
///
/// For each market:
/// 1. Compute fair value (done externally)
/// 2. Compute half-spread: s_base + s_inv + s_vol + s_fee
/// 3. bid = fair - s_total, ask = fair + s_total
/// 4. Inventory skew: shift both by -skew_coeff * inventory
/// 5. Clamp, filter, emit target quotes
pub struct MarketMakerStrategy {
    base_half_spread: Decimal,
    min_edge_after_fees: Decimal,
    default_order_size: Decimal,
    max_order_size: Decimal,
    inventory_skew_coeff: Decimal,
    volatility_widen_coeff: Decimal,
}

impl MarketMakerStrategy {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            base_half_spread: config.base_half_spread,
            min_edge_after_fees: config.min_edge_after_fees,
            default_order_size: Decimal::from(config.default_order_size),
            max_order_size: Decimal::from(config.max_order_size),
            inventory_skew_coeff: config.inventory_skew_coeff,
            volatility_widen_coeff: config.volatility_widen_coeff,
        }
    }

    pub fn generate_quotes(
        &self,
        ticker: &MarketTicker,
        fv: &FairValue,
        book: &OrderBook,
        position: Option<&Position>,
    ) -> Option<TargetQuote> {
        // Participation filter: skip empty/stale books
        if book.is_empty() {
            return None;
        }

        if book.is_stale(chrono::Duration::seconds(60)) {
            debug!(market = %ticker, "Skipping stale book");
            return None;
        }

        // Check spread vs min edge
        let spread = book.spread().unwrap_or(Decimal::ONE);
        if spread < self.min_edge_after_fees * dec!(2) {
            debug!(market = %ticker, spread = %spread, "Spread too tight");
            return None;
        }

        // Low confidence filter
        if fv.confidence < 0.2 {
            debug!(market = %ticker, confidence = fv.confidence, "Confidence too low");
            return None;
        }

        let fair = fv.price;

        // Compute inventory-based spread adjustment
        let inventory = position
            .map(|p| p.net_inventory())
            .unwrap_or(Decimal::ZERO);

        let inv_spread_adj = self.inventory_skew_coeff * inventory.abs() * dec!(0.1);

        // Volatility adjustment (simple: wider when spread is wide)
        let vol_adj = self.volatility_widen_coeff * (spread - dec!(0.02)).max(Decimal::ZERO);

        let total_half_spread = self.base_half_spread + inv_spread_adj + vol_adj;

        // Compute raw bid/ask
        let mut bid_price = fair - total_half_spread;
        let mut ask_price = fair + total_half_spread;

        // Inventory skew: shift both quotes
        let skew = -self.inventory_skew_coeff * inventory * dec!(0.01);
        bid_price += skew;
        ask_price += skew;

        // Round to cent (0.01)
        bid_price = round_down_to_cent(bid_price);
        ask_price = round_up_to_cent(ask_price);

        // Clamp
        let min_price = dec!(0.01);
        let max_price = dec!(0.99);
        bid_price = bid_price.max(min_price);
        ask_price = ask_price.min(max_price);

        // Ensure ask > bid
        if ask_price <= bid_price {
            ask_price = bid_price + dec!(0.01);
            if ask_price > max_price {
                return None;
            }
        }

        // Compute size
        let confidence_factor = Decimal::try_from(fv.confidence).unwrap_or(dec!(0.5));
        let qty = (self.default_order_size * confidence_factor)
            .max(Decimal::ONE)
            .min(self.max_order_size);
        let qty = qty.round_dp(0);

        debug!(
            market = %ticker,
            fair = %fair,
            bid = %bid_price,
            ask = %ask_price,
            qty = %qty,
            inventory = %inventory,
            "Quotes generated"
        );

        Some(TargetQuote {
            market_ticker: ticker.clone(),
            yes_bid: Some(PriceLevel {
                price: bid_price,
                quantity: qty,
            }),
            yes_ask: Some(PriceLevel {
                price: ask_price,
                quantity: qty,
            }),
            reason: format!(
                "fair={fair} spread={spread} inv={inventory} conf={:.2}",
                fv.confidence
            ),
        })
    }
}

fn round_down_to_cent(d: Decimal) -> Decimal {
    (d * dec!(100)).floor() / dec!(100)
}

fn round_up_to_cent(d: Decimal) -> Decimal {
    (d * dec!(100)).ceil() / dec!(100)
}
