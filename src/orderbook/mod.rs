use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

use crate::types::{PriceLevel, Side};

/// Per-market L2 order book. Stores aggregated price levels for yes and no sides.
/// In Kalshi's binary model, yes_bids and no_bids are the only sides;
/// asks are implied by complementarity (yes ask = 1.00 - no bid price).
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub yes_bids: BTreeMap<Decimal, Decimal>,
    pub no_bids: BTreeMap<Decimal, Decimal>,
    pub last_update: DateTime<Utc>,
    pub last_seq: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            yes_bids: BTreeMap::new(),
            no_bids: BTreeMap::new(),
            last_update: Utc::now(),
            last_seq: 0,
        }
    }

    pub fn apply_snapshot(
        &mut self,
        yes_bids: Vec<PriceLevel>,
        no_bids: Vec<PriceLevel>,
        seq: u64,
    ) {
        self.yes_bids.clear();
        self.no_bids.clear();

        for level in yes_bids {
            if level.quantity > Decimal::ZERO {
                self.yes_bids.insert(level.price, level.quantity);
            }
        }
        for level in no_bids {
            if level.quantity > Decimal::ZERO {
                self.no_bids.insert(level.price, level.quantity);
            }
        }

        self.last_update = Utc::now();
        self.last_seq = seq;
    }

    pub fn apply_delta(&mut self, side: Side, price: Decimal, delta: Decimal, seq: u64) {
        let book = match side {
            Side::Yes => &mut self.yes_bids,
            Side::No => &mut self.no_bids,
        };

        let current = book.get(&price).copied().unwrap_or(Decimal::ZERO);
        let new_qty = current + delta;

        if new_qty <= Decimal::ZERO {
            book.remove(&price);
        } else {
            book.insert(price, new_qty);
        }

        self.last_update = Utc::now();
        self.last_seq = seq;
    }

    /// Best (highest) yes bid price and quantity.
    pub fn best_yes_bid(&self) -> Option<PriceLevel> {
        self.yes_bids.iter().next_back().map(|(&p, &q)| PriceLevel {
            price: p,
            quantity: q,
        })
    }

    /// Best (highest) no bid price and quantity.
    pub fn best_no_bid(&self) -> Option<PriceLevel> {
        self.no_bids.iter().next_back().map(|(&p, &q)| PriceLevel {
            price: p,
            quantity: q,
        })
    }

    /// Implied yes ask = 1.00 - best_no_bid. This is the cheapest offer to sell YES.
    pub fn implied_yes_ask(&self) -> Option<Decimal> {
        self.best_no_bid()
            .map(|nb| Decimal::ONE - nb.price)
    }

    /// Mid price: average of best yes bid and implied yes ask.
    pub fn mid(&self) -> Option<Decimal> {
        let bid = self.best_yes_bid()?.price;
        let ask = self.implied_yes_ask()?;
        if ask <= bid {
            return None;
        }
        Some((bid + ask) / Decimal::TWO)
    }

    /// Spread between implied yes ask and best yes bid.
    pub fn spread(&self) -> Option<Decimal> {
        let bid = self.best_yes_bid()?.price;
        let ask = self.implied_yes_ask()?;
        Some(ask - bid)
    }

    /// Microprice / volume-weighted mid.
    /// microprice = (bid_price * ask_qty + ask_price * bid_qty) / (bid_qty + ask_qty)
    pub fn microprice(&self) -> Option<Decimal> {
        let yes_bid = self.best_yes_bid()?;
        let no_bid = self.best_no_bid()?;
        let ask_price = Decimal::ONE - no_bid.price;

        let total_qty = yes_bid.quantity + no_bid.quantity;
        if total_qty == Decimal::ZERO {
            return self.mid();
        }

        Some(
            (yes_bid.price * no_bid.quantity + ask_price * yes_bid.quantity) / total_qty,
        )
    }

    /// Order imbalance at top of book: (bid_qty - ask_qty) / (bid_qty + ask_qty).
    /// Ranges from -1 to 1. Positive means more bid pressure.
    pub fn order_imbalance(&self) -> Option<Decimal> {
        let bid = self.best_yes_bid()?;
        let ask = self.best_no_bid()?;
        let total = bid.quantity + ask.quantity;
        if total == Decimal::ZERO {
            return Some(Decimal::ZERO);
        }
        Some((bid.quantity - ask.quantity) / total)
    }

    /// Whether the book is stale (no update in given duration).
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        Utc::now() - self.last_update > max_age
    }

    pub fn is_empty(&self) -> bool {
        self.yes_bids.is_empty() && self.no_bids.is_empty()
    }
}
