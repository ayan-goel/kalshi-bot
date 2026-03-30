use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use tracing::debug;

use crate::state::StateEngine;
use crate::types::TargetQuote;

/// Cross-market consistency checker.
///
/// For markets within the same event, their implied YES probabilities
/// should sum to approximately 1.0 (for mutually exclusive outcomes).
///
/// When the sum deviates, this adjusts quotes:
/// - Sum > 1 + tolerance: widen ask on overpriced siblings
/// - Sum < 1 - tolerance: tighten bid on underpriced siblings
pub struct CrossMarketChecker {
    tolerance: Decimal,
}

impl CrossMarketChecker {
    pub fn new() -> Self {
        Self {
            tolerance: dec!(0.05), // 5% tolerance before adjusting
        }
    }

    /// Adjust target quotes for cross-market consistency.
    /// Returns the quotes with any needed adjustments applied.
    pub fn adjust_quotes(
        &self,
        quotes: Vec<TargetQuote>,
        state: &StateEngine,
    ) -> Vec<TargetQuote> {
        // Group quotes by event_ticker
        let mut event_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, q) in quotes.iter().enumerate() {
            if let Some(meta) = state.get_market_meta(&q.market_ticker) {
                if let Some(ref et) = meta.event_ticker {
                    event_groups.entry(et.clone()).or_default().push(i);
                }
            }
        }

        let mut adjusted = quotes;

        for (event_ticker, indices) in &event_groups {
            if indices.len() < 2 {
                continue;
            }

            // Calculate implied probability sum from mid-prices
            let mut prob_sum = Decimal::ZERO;
            let mut mid_prices: Vec<(usize, Decimal)> = Vec::new();

            for &idx in indices {
                let ticker = &adjusted[idx].market_ticker;
                if let Some(book) = state.get_book(ticker) {
                    if let Some(mid) = book.mid() {
                        prob_sum += mid;
                        mid_prices.push((idx, mid));
                    }
                }
            }

            if mid_prices.is_empty() {
                continue;
            }

            let excess = prob_sum - Decimal::ONE;

            if excess.abs() <= self.tolerance {
                continue;
            }

            debug!(
                event = %event_ticker,
                prob_sum = %prob_sum,
                excess = %excess,
                siblings = mid_prices.len(),
                "Cross-market inconsistency detected"
            );

            // Distribute the adjustment proportionally across siblings
            let adjustment_per_market = excess / Decimal::from(mid_prices.len() as i64);

            for &(idx, _mid) in &mid_prices {
                let q = &mut adjusted[idx];

                if excess > Decimal::ZERO {
                    // Overpriced as a group: widen asks (raise them) to reduce exposure
                    if let Some(ref mut ask) = q.yes_ask {
                        let shift = adjustment_per_market.abs().min(dec!(0.03));
                        ask.price += shift;
                    }
                } else {
                    // Underpriced as a group: tighten bids (raise them) to capture value
                    if let Some(ref mut bid) = q.yes_bid {
                        let shift = adjustment_per_market.abs().min(dec!(0.03));
                        bid.price += shift;
                    }
                }
            }
        }

        adjusted
    }
}
