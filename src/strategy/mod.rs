use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::market_scanner::maker_fee;
use crate::orderbook::OrderBook;
use crate::state::MarketMeta;
use crate::types::*;

const LEVEL_SIZE_WEIGHTS: [f64; 5] = [1.0, 0.6, 0.4, 0.3, 0.2];

/// Inventory-aware, fee-aware, multi-level market-making strategy.
///
/// For each market:
/// 1. Compute fee-aware minimum half-spread
/// 2. Add inventory spread widening + volatility widening + expiry widening
/// 3. Generate multiple quote levels with increasing spread and decreasing size
/// 4. Inventory skew shifts all quotes
/// 5. Tick-size snapping from market metadata
/// 6. Capital-aware sizing
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
    num_levels: u32,
    level_spread_increment: Decimal,
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
            num_levels: config.num_levels.max(1),
            level_spread_increment: config.level_spread_increment,
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

        let (tick_size, tick_min, tick_max) = match meta {
            Some(m) => (m.tick_size, m.tick_min, m.tick_max),
            None => (dec!(0.01), dec!(0.01), dec!(0.99)),
        };

        let bid_estimate = (fv.price - self.base_half_spread).max(tick_min);
        let ask_estimate = (fv.price + self.base_half_spread).min(tick_max);
        let fee_per_side = (maker_fee(bid_estimate) + maker_fee(ask_estimate)) / dec!(2);
        let fee_half_spread = fee_per_side + self.min_edge_after_fees;
        let effective_base = self.base_half_spread.max(fee_half_spread);

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

        let inv_spread_adj = self.inventory_skew_coeff * inventory.abs() * self.inv_spread_scale;
        let vol_adj =
            self.volatility_widen_coeff * (spread - self.vol_baseline_spread).max(Decimal::ZERO);
        let expiry_adj = self.compute_expiry_widening(meta);

        let base_total_half_spread = effective_base + inv_spread_adj + vol_adj + expiry_adj;

        let skew = -self.inventory_skew_coeff * inventory * self.inv_skew_scale;

        let total_capital_for_sizing = self.compute_total_capital_budget(balance, max_markets);

        let mut yes_bids: Vec<PriceLevel> = Vec::new();
        let mut yes_asks: Vec<PriceLevel> = Vec::new();
        let mut capital_used = Decimal::ZERO;

        for level in 0..self.num_levels {
            let level_extra = self.level_spread_increment * Decimal::from(level);
            let half_spread = base_total_half_spread + level_extra;

            let mut bid_price = fair - half_spread + skew;
            let mut ask_price = fair + half_spread + skew;

            bid_price = snap_down(bid_price, tick_size);
            ask_price = snap_up(ask_price, tick_size);

            bid_price = bid_price.max(tick_min);
            ask_price = ask_price.min(tick_max);

            if ask_price <= bid_price {
                ask_price = bid_price + tick_size;
                if ask_price > tick_max {
                    break;
                }
            }

            let size_weight = LEVEL_SIZE_WEIGHTS
                .get(level as usize)
                .copied()
                .unwrap_or(0.2);
            let qty = self.compute_level_size(
                fv,
                size_weight,
                bid_price,
                total_capital_for_sizing,
                &mut capital_used,
            );

            let qty = match qty {
                Some(q) => q,
                None => break,
            };

            yes_bids.push(PriceLevel {
                price: bid_price,
                quantity: qty,
            });
            yes_asks.push(PriceLevel {
                price: ask_price,
                quantity: qty,
            });
        }

        if yes_bids.is_empty() {
            return None;
        }

        debug!(
            market = %ticker,
            fair = %fair,
            levels = yes_bids.len(),
            inventory = %inventory,
            fee_hs = %fee_half_spread,
            expiry_adj = %expiry_adj,
            "Multi-level quotes generated"
        );

        Some(TargetQuote {
            market_ticker: ticker.clone(),
            yes_bids,
            yes_asks,
            reason: format!(
                "fair={fair} spread={spread} inv={inventory} conf={:.2} fee_hs={fee_half_spread} lvls={}",
                fv.confidence, self.num_levels
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

        let widen = self.expiry_widen_coeff
            * Decimal::from_f64_retain(1.0 / hours).unwrap_or(Decimal::ZERO);
        widen.min(dec!(0.10))
    }

    fn compute_total_capital_budget(&self, balance: &Balance, max_markets: u32) -> Decimal {
        let markets = Decimal::from(max_markets.max(1));
        balance.available / markets
    }

    fn compute_level_size(
        &self,
        fv: &FairValue,
        size_weight: f64,
        bid_price: Decimal,
        capital_budget: Decimal,
        capital_used: &mut Decimal,
    ) -> Option<Decimal> {
        let confidence_factor = Decimal::try_from(fv.confidence).unwrap_or(dec!(0.5));
        let weight = Decimal::try_from(size_weight).unwrap_or(dec!(1));
        let base_qty = (self.default_order_size * confidence_factor * weight)
            .max(Decimal::ONE)
            .min(self.max_order_size);

        let remaining_capital = capital_budget - *capital_used;
        if remaining_capital <= Decimal::ZERO {
            return None;
        }

        let max_by_capital = if bid_price > Decimal::ZERO {
            (remaining_capital / bid_price).floor()
        } else {
            self.max_order_size
        };

        let qty = base_qty.min(max_by_capital).round_dp(0);
        if qty.is_zero() {
            return None;
        }

        *capital_used += qty * bid_price;
        Some(qty)
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
