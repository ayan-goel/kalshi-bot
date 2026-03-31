use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::market_scanner::maker_fee;
use crate::orderbook::OrderBook;
use crate::state::MarketMeta;
use crate::types::*;

/// Inventory-aware, fee-aware market-making strategy.
///
/// For each market:
/// 1. Compute fee-aware minimum half-spread
/// 2. Add inventory spread widening + volatility widening + expiry widening
/// 3. Inventory skew shifts both quotes
/// 4. Tick-size snapping from market metadata
/// 5. Capital-aware sizing
pub struct MarketMakerStrategy {
    base_half_spread: Decimal,
    min_edge_after_fees: Decimal,
    default_order_size: Decimal,
    max_order_size: Decimal,
    inventory_skew_coeff: Decimal,
    volatility_widen_coeff: Decimal,
    inv_spread_scale: Decimal,
    inv_skew_scale: Decimal,
    vol_baseline_spread: Decimal,
    expiry_widen_coeff: Decimal,
    expiry_widen_threshold_hours: f64,
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
            inv_spread_scale: config.inv_spread_scale,
            inv_skew_scale: config.inv_skew_scale,
            vol_baseline_spread: config.vol_baseline_spread,
            expiry_widen_coeff: config.expiry_widen_coeff,
            expiry_widen_threshold_hours: config.expiry_widen_threshold_hours,
        }
    }

    pub fn generate_quotes(
        &self,
        ticker: &MarketTicker,
        fv: &FairValue,
        book: &OrderBook,
        position: Option<&Position>,
        meta: Option<&MarketMeta>,
        balance: &Balance,
        max_markets: u32,
    ) -> Option<TargetQuote> {
        if book.is_empty() {
            return None;
        }

        if book.is_stale(chrono::Duration::seconds(60)) {
            debug!(market = %ticker, "Skipping stale book");
            return None;
        }

        let spread = book.spread().unwrap_or(Decimal::ONE);

        // Tick bounds from metadata (needed for fee estimation below)
        let (tick_size, tick_min, tick_max) = match meta {
            Some(m) => (m.tick_size, m.tick_min, m.tick_max),
            None => (dec!(0.01), dec!(0.01), dec!(0.99)),
        };

        // Bug 12: compute fee floor using approximated bid/ask prices rather than
        // fair value so the floor is accurate at extreme mids (near 0 or 1).
        let bid_estimate = (fv.price - self.base_half_spread).max(tick_min);
        let ask_estimate = (fv.price + self.base_half_spread).min(tick_max);
        let fee_per_side = (maker_fee(bid_estimate) + maker_fee(ask_estimate)) / dec!(2);
        let fee_half_spread = fee_per_side + self.min_edge_after_fees;
        let effective_base = self.base_half_spread.max(fee_half_spread);

        // Check if observable spread can cover fees
        if spread < fee_half_spread * dec!(2) {
            debug!(
                market = %ticker,
                spread = %spread,
                min_spread = %(fee_half_spread * dec!(2)),
                "Spread too tight for fees"
            );
            return None;
        }

        if fv.confidence < 0.1 {
            debug!(market = %ticker, confidence = fv.confidence, "Confidence too low");
            return None;
        }

        let fair = fv.price;

        let inventory = position.map(|p| p.net_inventory()).unwrap_or(Decimal::ZERO);

        // Inventory-based spread widening (configurable scale, was hardcoded 0.1)
        let inv_spread_adj = self.inventory_skew_coeff * inventory.abs() * self.inv_spread_scale;

        // Volatility widening (configurable baseline, was hardcoded 0.02)
        let vol_adj =
            self.volatility_widen_coeff * (spread - self.vol_baseline_spread).max(Decimal::ZERO);

        // Time-to-expiry widening
        let expiry_adj = self.compute_expiry_widening(meta);

        let total_half_spread = effective_base + inv_spread_adj + vol_adj + expiry_adj;

        let mut bid_price = fair - total_half_spread;
        let mut ask_price = fair + total_half_spread;

        // Inventory skew (configurable scale, was hardcoded 0.01)
        let skew = -self.inventory_skew_coeff * inventory * self.inv_skew_scale;
        bid_price += skew;
        ask_price += skew;

        // Tick-size snapping (tick_size/tick_min/tick_max already set above)
        bid_price = snap_down(bid_price, tick_size);
        ask_price = snap_up(ask_price, tick_size);

        bid_price = bid_price.max(tick_min);
        ask_price = ask_price.min(tick_max);

        if ask_price <= bid_price {
            ask_price = bid_price + tick_size;
            if ask_price > tick_max {
                return None;
            }
        }

        // Capital-aware sizing — Bug 9: return None when capital allows zero contracts
        let qty = self.compute_size(fv, balance, max_markets, bid_price)?;

        debug!(
            market = %ticker,
            fair = %fair,
            bid = %bid_price,
            ask = %ask_price,
            qty = %qty,
            inventory = %inventory,
            fee_hs = %fee_half_spread,
            expiry_adj = %expiry_adj,
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
                "fair={fair} spread={spread} inv={inventory} conf={:.2} fee_hs={fee_half_spread}",
                fv.confidence
            ),
        })
    }

    fn compute_expiry_widening(&self, meta: Option<&MarketMeta>) -> Decimal {
        let hours = match meta {
            Some(m) => m.hours_to_expiry(),
            None => return Decimal::ZERO,
        };

        if hours <= 0.0 || hours > self.expiry_widen_threshold_hours {
            return Decimal::ZERO;
        }

        // Widen as expiry approaches: coeff / hours_remaining
        let widen = self.expiry_widen_coeff
            * Decimal::from_f64_retain(1.0 / hours).unwrap_or(Decimal::ZERO);
        widen.min(dec!(0.10)) // cap at 10 cents extra
    }

    fn compute_size(
        &self,
        fv: &FairValue,
        balance: &Balance,
        max_markets: u32,
        bid_price: Decimal,
    ) -> Option<Decimal> {
        let confidence_factor = Decimal::try_from(fv.confidence).unwrap_or(dec!(0.5));
        let base_qty = (self.default_order_size * confidence_factor)
            .max(Decimal::ONE)
            .min(self.max_order_size);

        // Capital-aware: don't commit more than balance / max_markets per market
        let markets = Decimal::from(max_markets.max(1));
        let capital_per_market = balance.available / markets;
        let max_by_capital = if bid_price > Decimal::ZERO {
            (capital_per_market / bid_price).floor()
        } else {
            self.max_order_size
        };

        // Bug 9: if the capital cap allows zero contracts, don't force a 1-contract order
        // that would exceed the per-market capital budget.
        let qty = base_qty.min(max_by_capital).round_dp(0);
        if qty.is_zero() {
            None
        } else {
            Some(qty)
        }
    }
}

/// Snap price down to nearest tick.
fn snap_down(price: Decimal, tick: Decimal) -> Decimal {
    if tick <= Decimal::ZERO {
        return price;
    }
    (price / tick).floor() * tick
}

/// Snap price up to nearest tick.
fn snap_up(price: Decimal, tick: Decimal) -> Decimal {
    if tick <= Decimal::ZERO {
        return price;
    }
    (price / tick).ceil() * tick
}
