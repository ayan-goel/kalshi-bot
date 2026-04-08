use kalshi_bot::config::StrategyConfig;
use kalshi_bot::fair_value::FairValueEngine;
use kalshi_bot::orderbook::OrderBook;
use kalshi_bot::state::MarketMeta;
use kalshi_bot::types::{MarketTicker, Position, PriceLevel};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn make_config() -> StrategyConfig {
    StrategyConfig {
        base_half_spread: dec!(0.02),
        min_edge_after_fees: dec!(0.005),
        default_order_size: 1,
        max_order_size: 3,
        min_rest_ms: 3000,
        repricing_threshold: dec!(0.01),
        inventory_skew_coeff: dec!(0.30),
        volatility_widen_coeff: dec!(0.50),
        tick_interval_ms: 2000,
        order_imbalance_alpha: dec!(0.05),
        trade_sign_alpha: dec!(0.02),
        inventory_penalty_k1: dec!(0.03),
        inventory_penalty_k3: dec!(0.001),
        inv_spread_scale: dec!(0.10),
        inv_skew_scale: dec!(0.01),
        vol_baseline_spread: dec!(0.02),
        expiry_widen_coeff: dec!(0.01),
        expiry_widen_threshold_hours: 4.0,
        event_half_spread_multiplier: dec!(3),
        event_threshold: dec!(0.05),
        event_decay_seconds: 30,
        num_levels: 3,
        level_spread_increment: dec!(0.01),
    }
}

fn make_ticker() -> MarketTicker {
    MarketTicker::from("TEST-MARKET")
}

/// Build a two-sided book with yes bid at `bid_price` and no bid at `no_bid_price`
/// so that implied_yes_ask = 1 - no_bid_price.
fn make_book(bid_price: Decimal, no_bid_price: Decimal, qty: Decimal) -> OrderBook {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![PriceLevel {
            price: bid_price,
            quantity: qty,
        }],
        vec![PriceLevel {
            price: no_bid_price,
            quantity: qty,
        }],
        1,
    );
    book
}

/// Book with symmetric qty centred at ~0.50:
/// yes bid = 0.45 @ 100, no bid = 0.45 @ 100 → ask = 0.55, mid = 0.50
fn midpoint_book() -> OrderBook {
    make_book(dec!(0.45), dec!(0.45), dec!(100))
}

fn engine() -> FairValueEngine {
    FairValueEngine::new(&make_config())
}

#[test]
fn test_fair_value_empty_book_returns_none() {
    let book = OrderBook::new();
    let result = engine().compute(&make_ticker(), &book, None, Decimal::ZERO, None);
    assert!(result.is_none());
}

#[test]
fn test_fair_value_no_inventory_no_imbalance() {
    // Symmetric book → microprice = 0.50, imbalance = 0, no inventory, no trade sign
    let book = midpoint_book();
    let fv = engine()
        .compute(&make_ticker(), &book, None, Decimal::ZERO, None)
        .unwrap();
    // raw = 0.50 + 0 + 0 + 0 = 0.50
    assert_eq!(fv.price, dec!(0.50));
}

#[test]
fn test_fair_value_independent_of_long_inventory() {
    // Inventory adjustment was removed from FairValueEngine (BUG-11 fix).
    // Fair value should equal microprice + signals regardless of position size.
    let book = midpoint_book();
    let pos_large = Position {
        market_ticker: make_ticker(),
        yes_contracts: dec!(10),
        no_contracts: Decimal::ZERO,
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let fv_large = engine()
        .compute(&make_ticker(), &book, Some(&pos_large), Decimal::ZERO, None)
        .unwrap();

    let fv_flat = engine()
        .compute(&make_ticker(), &book, None, Decimal::ZERO, None)
        .unwrap();

    // Fair value is the same regardless of inventory — skew belongs in the strategy.
    assert_eq!(fv_large.price, fv_flat.price);
    assert_eq!(fv_flat.price, dec!(0.50));
}

#[test]
fn test_fair_value_independent_of_short_inventory() {
    let book = midpoint_book();
    let pos_short = Position {
        market_ticker: make_ticker(),
        yes_contracts: Decimal::ZERO,
        no_contracts: dec!(5),
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let fv_short = engine()
        .compute(&make_ticker(), &book, Some(&pos_short), Decimal::ZERO, None)
        .unwrap();

    let fv_flat = engine()
        .compute(&make_ticker(), &book, None, Decimal::ZERO, None)
        .unwrap();

    assert_eq!(fv_short.price, fv_flat.price);
    assert_eq!(fv_flat.price, dec!(0.50));
}

#[test]
fn test_fair_value_positive_imbalance_increases_price() {
    // More bid qty than ask qty → positive imbalance → fair up
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![PriceLevel {
            price: dec!(0.45),
            quantity: dec!(300),
        }],
        vec![PriceLevel {
            price: dec!(0.45),
            quantity: dec!(100),
        }],
        1,
    );
    let fv_engine = engine();
    let fv_balanced = {
        let b = midpoint_book();
        fv_engine
            .compute(&make_ticker(), &b, None, Decimal::ZERO, None)
            .unwrap()
    };
    let fv_imbal = fv_engine
        .compute(&make_ticker(), &book, None, Decimal::ZERO, None)
        .unwrap();
    assert!(
        fv_imbal.price > fv_balanced.price,
        "imbalanced fair {fv_imbal:?} should be higher"
    );
}

#[test]
fn test_fair_value_clamp_high() {
    // Make a book near 0.98 with short position to push above 0.99
    let book = make_book(dec!(0.95), dec!(0.04), dec!(100));
    let pos = Position {
        market_ticker: make_ticker(),
        yes_contracts: Decimal::ZERO,
        no_contracts: dec!(5),
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let fv = engine()
        .compute(&make_ticker(), &book, Some(&pos), Decimal::ZERO, None)
        .unwrap();
    assert!(fv.price <= dec!(0.99));
}

#[test]
fn test_fair_value_clamp_low() {
    // Make a book near 0.02 with large long position
    let book = make_book(dec!(0.02), dec!(0.97), dec!(100));
    let pos = Position {
        market_ticker: make_ticker(),
        yes_contracts: dec!(5),
        no_contracts: Decimal::ZERO,
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let fv = engine()
        .compute(&make_ticker(), &book, Some(&pos), Decimal::ZERO, None)
        .unwrap();
    assert!(fv.price >= dec!(0.01));
}

#[test]
fn test_fair_value_deterministic() {
    let book = midpoint_book();
    let fv1 = engine()
        .compute(&make_ticker(), &book, None, dec!(0.5), None)
        .unwrap();
    let fv2 = engine()
        .compute(&make_ticker(), &book, None, dec!(0.5), None)
        .unwrap();
    assert_eq!(fv1.price, fv2.price);
    // Confidences are based on current time (staleness), so may differ by tiny amounts;
    // check they are within a reasonable range
    assert!((fv1.confidence - fv2.confidence).abs() < 0.01);
}

#[test]
fn test_fair_value_meta_tick_bounds() {
    let book = midpoint_book();
    let mut meta = MarketMeta::default();
    meta.tick_min = dec!(0.40);
    meta.tick_max = dec!(0.60);
    // Fair should be clamped to [0.40, 0.60] regardless
    let fv = engine()
        .compute(&make_ticker(), &book, None, Decimal::ZERO, Some(&meta))
        .unwrap();
    assert!(fv.price >= dec!(0.40));
    assert!(fv.price <= dec!(0.60));
}

#[test]
fn test_confidence_between_zero_and_one() {
    let book = midpoint_book();
    let fv = engine()
        .compute(&make_ticker(), &book, None, Decimal::ZERO, None)
        .unwrap();
    assert!(fv.confidence > 0.0 && fv.confidence <= 1.0);
}
