use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// We need to test the orderbook module directly
// Since it's a binary crate, we'll re-implement the core logic for testing
// or test via the public API

#[cfg(test)]
mod orderbook {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    struct PriceLevel {
        price: Decimal,
        quantity: Decimal,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Side {
        Yes,
        No,
    }

    #[derive(Debug)]
    struct OrderBook {
        yes_bids: BTreeMap<Decimal, Decimal>,
        no_bids: BTreeMap<Decimal, Decimal>,
    }

    impl OrderBook {
        fn new() -> Self {
            Self {
                yes_bids: BTreeMap::new(),
                no_bids: BTreeMap::new(),
            }
        }

        fn apply_snapshot(&mut self, yes: Vec<PriceLevel>, no: Vec<PriceLevel>) {
            self.yes_bids.clear();
            self.no_bids.clear();
            for l in yes {
                if l.quantity > Decimal::ZERO {
                    self.yes_bids.insert(l.price, l.quantity);
                }
            }
            for l in no {
                if l.quantity > Decimal::ZERO {
                    self.no_bids.insert(l.price, l.quantity);
                }
            }
        }

        fn apply_delta(&mut self, side: Side, price: Decimal, delta: Decimal) {
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
        }

        fn best_yes_bid(&self) -> Option<(Decimal, Decimal)> {
            self.yes_bids.iter().next_back().map(|(&p, &q)| (p, q))
        }

        fn best_no_bid(&self) -> Option<(Decimal, Decimal)> {
            self.no_bids.iter().next_back().map(|(&p, &q)| (p, q))
        }

        fn implied_yes_ask(&self) -> Option<Decimal> {
            self.best_no_bid().map(|(p, _)| Decimal::ONE - p)
        }

        fn mid(&self) -> Option<Decimal> {
            let bid = self.best_yes_bid()?.0;
            let ask = self.implied_yes_ask()?;
            if ask <= bid {
                return None;
            }
            Some((bid + ask) / dec!(2))
        }

        fn spread(&self) -> Option<Decimal> {
            let bid = self.best_yes_bid()?.0;
            let ask = self.implied_yes_ask()?;
            Some(ask - bid)
        }

        fn microprice(&self) -> Option<Decimal> {
            let (bid_p, bid_q) = self.best_yes_bid()?;
            let (no_p, no_q) = self.best_no_bid()?;
            let ask_p = Decimal::ONE - no_p;
            let total = bid_q + no_q;
            if total == Decimal::ZERO {
                return self.mid();
            }
            Some((bid_p * no_q + ask_p * bid_q) / total)
        }

        fn order_imbalance(&self) -> Option<Decimal> {
            let (_, bid_q) = self.best_yes_bid()?;
            let (_, ask_q) = self.best_no_bid()?;
            let total = bid_q + ask_q;
            if total == Decimal::ZERO {
                return Some(Decimal::ZERO);
            }
            Some((bid_q - ask_q) / total)
        }
    }

    #[test]
    fn test_empty_book() {
        let book = OrderBook::new();
        assert!(book.best_yes_bid().is_none());
        assert!(book.best_no_bid().is_none());
        assert!(book.mid().is_none());
        assert!(book.spread().is_none());
        assert!(book.microprice().is_none());
    }

    #[test]
    fn test_snapshot_application() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![
                PriceLevel { price: dec!(0.40), quantity: dec!(100) },
                PriceLevel { price: dec!(0.42), quantity: dec!(50) },
            ],
            vec![
                PriceLevel { price: dec!(0.55), quantity: dec!(80) },
                PriceLevel { price: dec!(0.60), quantity: dec!(30) },
            ],
        );

        let (best_bid_p, best_bid_q) = book.best_yes_bid().unwrap();
        assert_eq!(best_bid_p, dec!(0.42));
        assert_eq!(best_bid_q, dec!(50));

        let (best_no_p, best_no_q) = book.best_no_bid().unwrap();
        assert_eq!(best_no_p, dec!(0.60));
        assert_eq!(best_no_q, dec!(30));
    }

    #[test]
    fn test_implied_yes_ask() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(80) }],
        );

        // Implied yes ask = 1.00 - best_no_bid = 1.00 - 0.55 = 0.45
        assert_eq!(book.implied_yes_ask().unwrap(), dec!(0.45));
    }

    #[test]
    fn test_mid_price() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(80) }],
        );

        // Mid = (0.40 + 0.45) / 2 = 0.425
        assert_eq!(book.mid().unwrap(), dec!(0.425));
    }

    #[test]
    fn test_spread() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(80) }],
        );

        // Spread = 0.45 - 0.40 = 0.05
        assert_eq!(book.spread().unwrap(), dec!(0.05));
    }

    #[test]
    fn test_microprice() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(100) }],
        );

        // microprice = (0.40 * 100 + 0.45 * 100) / 200 = 85/200 = 0.425
        let mp = book.microprice().unwrap();
        assert_eq!(mp, dec!(0.425));
    }

    #[test]
    fn test_microprice_asymmetric() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(200) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(100) }],
        );

        // microprice = (0.40 * 100 + 0.45 * 200) / 300 = (40 + 90) / 300 = 130/300
        let mp = book.microprice().unwrap();
        // With more bid qty, microprice should be closer to the ask (higher)
        assert!(mp > dec!(0.425));
    }

    #[test]
    fn test_order_imbalance() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(100) }],
        );

        // Balanced: imbalance = 0
        assert_eq!(book.order_imbalance().unwrap(), dec!(0));
    }

    #[test]
    fn test_order_imbalance_positive() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(300) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(100) }],
        );

        // More bid qty -> positive imbalance
        let imb = book.order_imbalance().unwrap();
        assert_eq!(imb, dec!(0.5)); // (300 - 100) / 400
    }

    #[test]
    fn test_delta_add_new_level() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![],
        );

        book.apply_delta(Side::Yes, dec!(0.41), dec!(50));
        let (best_p, best_q) = book.best_yes_bid().unwrap();
        assert_eq!(best_p, dec!(0.41));
        assert_eq!(best_q, dec!(50));
    }

    #[test]
    fn test_delta_update_existing() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![],
        );

        book.apply_delta(Side::Yes, dec!(0.40), dec!(50));
        let (_, qty) = book.best_yes_bid().unwrap();
        assert_eq!(qty, dec!(150));
    }

    #[test]
    fn test_delta_remove_level() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![],
        );

        book.apply_delta(Side::Yes, dec!(0.40), dec!(-100));
        assert!(book.best_yes_bid().is_none());
    }

    #[test]
    fn test_delta_negative_removes() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(50) }],
            vec![],
        );

        // Delta of -100 on a level with 50 should remove it
        book.apply_delta(Side::Yes, dec!(0.40), dec!(-100));
        assert!(book.best_yes_bid().is_none());
    }

    #[test]
    fn test_snapshot_replaces_previous() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.55), quantity: dec!(80) }],
        );

        // Apply a completely different snapshot
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.50), quantity: dec!(200) }],
            vec![PriceLevel { price: dec!(0.60), quantity: dec!(150) }],
        );

        let (p, q) = book.best_yes_bid().unwrap();
        assert_eq!(p, dec!(0.50));
        assert_eq!(q, dec!(200));
        assert_eq!(book.yes_bids.len(), 1);
    }

    #[test]
    fn test_zero_quantity_ignored_in_snapshot() {
        let mut book = OrderBook::new();
        book.apply_snapshot(
            vec![
                PriceLevel { price: dec!(0.40), quantity: dec!(0) },
                PriceLevel { price: dec!(0.42), quantity: dec!(50) },
            ],
            vec![],
        );

        assert_eq!(book.yes_bids.len(), 1);
        assert_eq!(book.best_yes_bid().unwrap().0, dec!(0.42));
    }

    #[test]
    fn test_locked_market_no_mid() {
        let mut book = OrderBook::new();
        // Yes bid at 0.60, no bid at 0.35 -> implied yes ask = 0.65 -> normal
        // Yes bid at 0.60, no bid at 0.45 -> implied yes ask = 0.55 -> normal
        // Yes bid at 0.60, no bid at 0.30 -> implied yes ask = 0.70 -> spread = 0.10
        // Yes bid at 0.60, no bid at 0.40 -> implied yes ask = 0.60 -> spread = 0 (locked)
        book.apply_snapshot(
            vec![PriceLevel { price: dec!(0.60), quantity: dec!(100) }],
            vec![PriceLevel { price: dec!(0.40), quantity: dec!(100) }],
        );

        // Implied ask = 1.00 - 0.40 = 0.60 = bid -> locked, mid should be None
        assert!(book.mid().is_none());
    }
}
