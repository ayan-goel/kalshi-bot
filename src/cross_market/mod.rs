use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use tracing::debug;

use crate::state::StateEngine;
use crate::types::TargetQuote;

/// Cross-market consistency checker.
///
/// Applies ONLY to true mutex pairs: exactly 2 sibling markets whose YES
/// probabilities must sum to 1.0 (e.g. team A wins / team B wins).
///
/// Sports markets often have many sibling markets under one event_ticker
/// (totals: "over 4", "over 5", ...) that are NOT mutually exclusive —
/// their YES probs can and do sum to > 1 legitimately.  We guard against
/// this by only acting when there are exactly 2 siblings AND their sum is
/// between 0.5 and 1.5 (plausibly a mutex pair, not a totals fan-out).
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
    pub fn adjust_quotes(&self, quotes: Vec<TargetQuote>, state: &StateEngine) -> Vec<TargetQuote> {
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
            // Only handle true mutex pairs (exactly 2 sibling markets).
            // Sports totals/spreads fan-out into many markets under one event —
            // they are NOT mutex and must not be summed to 1.
            if indices.len() != 2 {
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

            if mid_prices.len() != 2 {
                continue;
            }

            // Sanity: a mutex pair should sum to ~0.9–1.1.
            // If sum is wildly off (e.g. 2.25 for sports totals), skip.
            if prob_sum < dec!(0.70) || prob_sum > dec!(1.30) {
                debug!(
                    event = %event_ticker,
                    prob_sum = %prob_sum,
                    "Cross-market: skipping non-mutex pair (prob_sum out of range)"
                );
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
                "Cross-market consistency adjustment"
            );

            // Shift each side by half the excess
            let shift = (excess / dec!(2)).abs().min(dec!(0.02));

            for &(idx, _mid) in &mid_prices {
                let q = &mut adjusted[idx];

                if excess > Decimal::ZERO {
                    // Both markets together are "overpriced" vs 1.0 sum
                    // → widen each ask slightly
                    if let Some(ref mut ask) = q.yes_ask {
                        ask.price += shift;
                    }
                } else {
                    // Sum < 1.0: "underpriced" → tighten each bid slightly
                    if let Some(ref mut bid) = q.yes_bid {
                        bid.price += shift;
                    }
                }
            }
        }

        adjusted
    }
}
