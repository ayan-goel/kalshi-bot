/// Execution reconcile-side logic tests.
///
/// `ExecutionEngine::reconcile` requires a live `PgPool` and REST client, making full
/// integration testing of the reconcile method a DB-backed concern.  These tests validate
/// the repricing-threshold decision logic in isolation using the real types from the crate
/// so that any change to the types breaks these tests immediately.
use kalshi_bot::state::LiveOrder;
use kalshi_bot::types::{Action, MarketTicker, OrderStatus, PriceLevel, Side};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Mirrors the repricing decision logic in `ExecutionEngine::reconcile_side`.
///
/// Returns `true` if the live order needs to be replaced.
fn should_reprice(live: &LiveOrder, target: &PriceLevel, threshold: Decimal) -> bool {
    let price_diff = (live.price - target.price).abs();
    let qty_diff = (live.remaining_count - target.quantity).abs();
    price_diff >= threshold || qty_diff >= Decimal::ONE
}

fn make_order(price: Decimal, qty: Decimal) -> LiveOrder {
    LiveOrder {
        order_id: "test-order".to_string(),
        market_ticker: MarketTicker::from("TEST"),
        side: Side::Yes,
        action: Action::Buy,
        price,
        remaining_count: qty,
        fill_count: Decimal::ZERO,
        status: OrderStatus::Resting,
        client_order_id: None,
    }
}

fn make_target(price: Decimal, qty: Decimal) -> PriceLevel {
    PriceLevel { price, quantity: qty }
}

#[test]
fn test_matching_price_and_qty_holds() {
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.45), dec!(5));
    assert!(!should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_price_change_above_threshold_replaces() {
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.47), dec!(5));
    // diff = 0.02 ≥ threshold 0.01 → reprice
    assert!(should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_price_change_below_threshold_holds() {
    let live = make_order(dec!(0.450), dec!(5));
    let target = make_target(dec!(0.455), dec!(5));
    // diff = 0.005 < threshold 0.01 → hold
    assert!(!should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_qty_change_at_or_above_one_replaces() {
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.45), dec!(10));
    // qty diff = 5 ≥ 1 → reprice
    assert!(should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_small_qty_change_holds() {
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.45), dec!(5) + dec!(0.5));
    // qty diff = 0.5 < 1 → hold
    assert!(!should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_both_price_and_qty_change_replaces() {
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.50), dec!(10));
    assert!(should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_exact_threshold_price_replaces() {
    // price diff = exactly threshold → should reprice (≥, not >)
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.46), dec!(5));
    assert!(should_reprice(&live, &target, dec!(0.01)));
}

#[test]
fn test_exact_threshold_qty_replaces() {
    // qty diff = exactly 1 → should reprice (≥, not >)
    let live = make_order(dec!(0.45), dec!(5));
    let target = make_target(dec!(0.45), dec!(6));
    assert!(should_reprice(&live, &target, dec!(0.01)));
}
