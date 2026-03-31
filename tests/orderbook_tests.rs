use kalshi_bot::orderbook::OrderBook;
use kalshi_bot::types::{PriceLevel, Side};
use rust_decimal_macros::dec;

fn pl(
    price: impl Into<rust_decimal::Decimal>,
    qty: impl Into<rust_decimal::Decimal>,
) -> PriceLevel {
    PriceLevel {
        price: price.into(),
        quantity: qty.into(),
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
    assert!(book.is_empty());
}

#[test]
fn test_snapshot_application() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100)), pl(dec!(0.42), dec!(50))],
        vec![pl(dec!(0.55), dec!(80)), pl(dec!(0.60), dec!(30))],
        1,
    );

    let best_bid = book.best_yes_bid().unwrap();
    assert_eq!(best_bid.price, dec!(0.42));
    assert_eq!(best_bid.quantity, dec!(50));

    let best_no = book.best_no_bid().unwrap();
    assert_eq!(best_no.price, dec!(0.60));
    assert_eq!(best_no.quantity, dec!(30));
    assert!(!book.is_empty());
}

#[test]
fn test_implied_yes_ask() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(80))],
        1,
    );
    // implied yes ask = 1.00 - best_no_bid = 1.00 - 0.55 = 0.45
    assert_eq!(book.implied_yes_ask().unwrap(), dec!(0.45));
}

#[test]
fn test_mid_price() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(80))],
        1,
    );
    // mid = (0.40 + 0.45) / 2 = 0.425
    assert_eq!(book.mid().unwrap(), dec!(0.425));
}

#[test]
fn test_spread() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(80))],
        1,
    );
    // spread = 0.45 - 0.40 = 0.05
    assert_eq!(book.spread().unwrap(), dec!(0.05));
}

#[test]
fn test_microprice_symmetric() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(100))],
        1,
    );
    // microprice = (bid_p * no_q + ask_p * yes_q) / (yes_q + no_q)
    //            = (0.40 * 100 + 0.45 * 100) / 200 = 0.425
    assert_eq!(book.microprice().unwrap(), dec!(0.425));
}

#[test]
fn test_microprice_asymmetric() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(200))],
        vec![pl(dec!(0.55), dec!(100))],
        1,
    );
    // More bid qty → microprice closer to ask (higher than 0.425)
    let mp = book.microprice().unwrap();
    assert!(mp > dec!(0.425));
}

#[test]
fn test_order_imbalance_balanced() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(100))],
        1,
    );
    assert_eq!(book.order_imbalance().unwrap(), dec!(0));
}

#[test]
fn test_order_imbalance_positive() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(300))],
        vec![pl(dec!(0.55), dec!(100))],
        1,
    );
    // (300 - 100) / 400 = 0.5
    assert_eq!(book.order_imbalance().unwrap(), dec!(0.5));
}

#[test]
fn test_delta_add_new_level() {
    let mut book = OrderBook::new();
    book.apply_snapshot(vec![pl(dec!(0.40), dec!(100))], vec![], 1);
    book.apply_delta(Side::Yes, dec!(0.41), dec!(50), 2);
    let best = book.best_yes_bid().unwrap();
    assert_eq!(best.price, dec!(0.41));
    assert_eq!(best.quantity, dec!(50));
}

#[test]
fn test_delta_update_existing() {
    let mut book = OrderBook::new();
    book.apply_snapshot(vec![pl(dec!(0.40), dec!(100))], vec![], 1);
    book.apply_delta(Side::Yes, dec!(0.40), dec!(50), 2);
    assert_eq!(book.best_yes_bid().unwrap().quantity, dec!(150));
}

#[test]
fn test_delta_remove_level() {
    let mut book = OrderBook::new();
    book.apply_snapshot(vec![pl(dec!(0.40), dec!(100))], vec![], 1);
    book.apply_delta(Side::Yes, dec!(0.40), dec!(-100), 2);
    assert!(book.best_yes_bid().is_none());
    assert!(book.is_empty());
}

#[test]
fn test_delta_negative_removes_partial() {
    let mut book = OrderBook::new();
    book.apply_snapshot(vec![pl(dec!(0.40), dec!(50))], vec![], 1);
    // Removing more than exists should also remove the level
    book.apply_delta(Side::Yes, dec!(0.40), dec!(-100), 2);
    assert!(book.best_yes_bid().is_none());
}

#[test]
fn test_snapshot_replaces_previous() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(100))],
        vec![pl(dec!(0.55), dec!(80))],
        1,
    );
    // Apply a completely different snapshot
    book.apply_snapshot(
        vec![pl(dec!(0.50), dec!(200))],
        vec![pl(dec!(0.60), dec!(150))],
        2,
    );
    let best = book.best_yes_bid().unwrap();
    assert_eq!(best.price, dec!(0.50));
    assert_eq!(best.quantity, dec!(200));
}

#[test]
fn test_zero_quantity_ignored_in_snapshot() {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![pl(dec!(0.40), dec!(0)), pl(dec!(0.42), dec!(50))],
        vec![],
        1,
    );
    assert_eq!(book.best_yes_bid().unwrap().price, dec!(0.42));
}

#[test]
fn test_locked_market_no_mid() {
    let mut book = OrderBook::new();
    // yes bid at 0.60, no bid at 0.40 → implied yes ask = 0.60 = bid → locked
    book.apply_snapshot(
        vec![pl(dec!(0.60), dec!(100))],
        vec![pl(dec!(0.40), dec!(100))],
        1,
    );
    assert!(book.mid().is_none());
}
