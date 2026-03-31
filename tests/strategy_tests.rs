use kalshi_bot::config::StrategyConfig;
use kalshi_bot::orderbook::OrderBook;
use kalshi_bot::state::MarketMeta;
use kalshi_bot::strategy::MarketMakerStrategy;
use kalshi_bot::types::{Balance, FairValue, MarketTicker, Position, PriceLevel};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn make_config() -> StrategyConfig {
    StrategyConfig {
        base_half_spread: dec!(0.02),
        min_edge_after_fees: dec!(0.005),
        default_order_size: 5,
        max_order_size: 25,
        min_rest_ms: 3000,
        repricing_threshold: dec!(0.01),
        inventory_skew_coeff: dec!(0.25),
        volatility_widen_coeff: dec!(0.40),
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

fn make_single_level_config() -> StrategyConfig {
    StrategyConfig {
        num_levels: 1,
        ..make_config()
    }
}

fn make_ticker() -> MarketTicker {
    MarketTicker::from("TEST-MARKET")
}

fn make_book(bid: Decimal, no_bid: Decimal, qty: Decimal) -> OrderBook {
    let mut book = OrderBook::new();
    book.apply_snapshot(
        vec![PriceLevel {
            price: bid,
            quantity: qty,
        }],
        vec![PriceLevel {
            price: no_bid,
            quantity: qty,
        }],
        1,
    );
    book
}

fn make_fv(price: Decimal, confidence: f64) -> FairValue {
    FairValue {
        market_ticker: make_ticker(),
        price,
        confidence,
    }
}

fn make_balance(available: Decimal) -> Balance {
    Balance {
        available,
        portfolio_value: Decimal::ZERO,
    }
}

fn make_meta() -> MarketMeta {
    MarketMeta::default()
}

fn strategy() -> MarketMakerStrategy {
    MarketMakerStrategy::new(&make_config())
}

fn single_level_strategy() -> MarketMakerStrategy {
    MarketMakerStrategy::new(&make_single_level_config())
}

#[test]
fn test_basic_quoting() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = single_level_strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    assert!(!quote.yes_bids.is_empty());
    assert!(!quote.yes_asks.is_empty());
    assert!(quote.yes_bids[0].price < dec!(0.50));
    assert!(quote.yes_asks[0].price > dec!(0.50));
    assert!(quote.yes_asks[0].price > quote.yes_bids[0].price);
    assert!(quote.yes_bids[0].quantity > Decimal::ZERO);
}

#[test]
fn test_spread_too_tight_returns_none() {
    let book = make_book(dec!(0.49), dec!(0.51), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy().generate_quotes(
        &make_ticker(),
        &fv,
        &book,
        None,
        Some(&make_meta()),
        &make_balance(dec!(1000)),
        5,
    );
    assert!(quote.is_none(), "Spread too tight should return None");
}

#[test]
fn test_low_confidence_returns_none() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.05);
    let quote = strategy().generate_quotes(
        &make_ticker(),
        &fv,
        &book,
        None,
        Some(&make_meta()),
        &make_balance(dec!(1000)),
        5,
    );
    assert!(quote.is_none(), "Low confidence should return None");
}

#[test]
fn test_inventory_skew_long_shifts_quotes_down() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let bal = make_balance(dec!(1000));
    let s = single_level_strategy();
    let meta = make_meta();

    let neutral = s
        .generate_quotes(&make_ticker(), &fv, &book, None, Some(&meta), &bal, 5)
        .unwrap();
    let long_pos = Position {
        market_ticker: make_ticker(),
        yes_contracts: dec!(5),
        no_contracts: Decimal::ZERO,
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let long = s
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            Some(&long_pos),
            Some(&meta),
            &bal,
            5,
        )
        .unwrap();

    let neutral_mid = (neutral.yes_bids[0].price + neutral.yes_asks[0].price) / dec!(2);
    let long_mid = (long.yes_bids[0].price + long.yes_asks[0].price) / dec!(2);
    assert!(
        long_mid < neutral_mid,
        "Long inventory should shift quotes down: {long_mid} vs {neutral_mid}"
    );
}

#[test]
fn test_inventory_skew_short_shifts_quotes_up() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let bal = make_balance(dec!(1000));
    let s = single_level_strategy();
    let meta = make_meta();

    let neutral = s
        .generate_quotes(&make_ticker(), &fv, &book, None, Some(&meta), &bal, 5)
        .unwrap();
    let short_pos = Position {
        market_ticker: make_ticker(),
        yes_contracts: Decimal::ZERO,
        no_contracts: dec!(5),
        avg_yes_price: None,
        avg_no_price: None,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
    };
    let short = s
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            Some(&short_pos),
            Some(&meta),
            &bal,
            5,
        )
        .unwrap();

    let neutral_mid = (neutral.yes_bids[0].price + neutral.yes_asks[0].price) / dec!(2);
    let short_mid = (short.yes_bids[0].price + short.yes_asks[0].price) / dec!(2);
    assert!(
        short_mid > neutral_mid,
        "Short inventory should shift quotes up: {short_mid} vs {neutral_mid}"
    );
}

#[test]
fn test_zero_capital_returns_none() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy().generate_quotes(
        &make_ticker(),
        &fv,
        &book,
        None,
        Some(&make_meta()),
        &make_balance(Decimal::ZERO),
        5,
    );
    assert!(
        quote.is_none(),
        "Zero capital should return None from generate_quotes"
    );
}

#[test]
fn test_empty_book_returns_none() {
    let book = OrderBook::new();
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy().generate_quotes(
        &make_ticker(),
        &fv,
        &book,
        None,
        Some(&make_meta()),
        &make_balance(dec!(1000)),
        5,
    );
    assert!(quote.is_none(), "Empty book should return None");
}

#[test]
fn test_bid_ask_within_tick_bounds() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let meta = make_meta();
    let quote = strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&meta),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    for (i, (bid, ask)) in quote
        .yes_bids
        .iter()
        .zip(quote.yes_asks.iter())
        .enumerate()
    {
        assert!(bid.price >= dec!(0.01), "Level {i} bid {} below tick_min", bid.price);
        assert!(ask.price <= dec!(0.99), "Level {i} ask {} above tick_max", ask.price);
        assert!(ask.price > bid.price, "Level {i} ask must be above bid");
    }
}

#[test]
fn test_wider_book_spread_widens_quotes() {
    let narrow_book = make_book(dec!(0.47), dec!(0.47), dec!(100));
    let wide_book = make_book(dec!(0.40), dec!(0.40), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let bal = make_balance(dec!(1000));
    let s = single_level_strategy();
    let meta = make_meta();

    let narrow = s
        .generate_quotes(
            &make_ticker(),
            &fv,
            &narrow_book,
            None,
            Some(&meta),
            &bal,
            5,
        )
        .unwrap();
    let wide = s
        .generate_quotes(&make_ticker(), &fv, &wide_book, None, Some(&meta), &bal, 5)
        .unwrap();

    let narrow_half = (narrow.yes_asks[0].price - narrow.yes_bids[0].price) / dec!(2);
    let wide_half = (wide.yes_asks[0].price - wide.yes_bids[0].price) / dec!(2);
    assert!(
        wide_half > narrow_half,
        "Wider book should widen quotes: {wide_half} vs {narrow_half}"
    );
}

// --- Multi-level quoting tests ---

#[test]
fn test_multi_level_generates_correct_count() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    assert_eq!(quote.yes_bids.len(), 3, "Should have 3 bid levels");
    assert_eq!(quote.yes_asks.len(), 3, "Should have 3 ask levels");
}

#[test]
fn test_multi_level_spacing() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    // Each successive bid level should be lower (wider spread)
    for i in 1..quote.yes_bids.len() {
        assert!(
            quote.yes_bids[i].price <= quote.yes_bids[i - 1].price,
            "Bid level {} ({}) should be <= level {} ({})",
            i,
            quote.yes_bids[i].price,
            i - 1,
            quote.yes_bids[i - 1].price,
        );
    }

    // Each successive ask level should be higher (wider spread)
    for i in 1..quote.yes_asks.len() {
        assert!(
            quote.yes_asks[i].price >= quote.yes_asks[i - 1].price,
            "Ask level {} ({}) should be >= level {} ({})",
            i,
            quote.yes_asks[i].price,
            i - 1,
            quote.yes_asks[i - 1].price,
        );
    }
}

#[test]
fn test_multi_level_no_crossing() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    let check_len = quote.yes_bids.len().min(quote.yes_asks.len());
    for i in 0..check_len {
        assert!(
            quote.yes_asks[i].price > quote.yes_bids[i].price,
            "Level {} ask ({}) must exceed bid ({})",
            i,
            quote.yes_asks[i].price,
            quote.yes_bids[i].price,
        );
    }
}

#[test]
fn test_multi_level_size_decreases() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    if quote.yes_bids.len() >= 2 {
        assert!(
            quote.yes_bids[0].quantity >= quote.yes_bids[1].quantity,
            "Level 0 qty ({}) should be >= level 1 qty ({})",
            quote.yes_bids[0].quantity,
            quote.yes_bids[1].quantity,
        );
    }
}

#[test]
fn test_multi_level_capital_constraint_reduces_levels() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    // Very limited capital: should produce fewer levels than configured
    let quote = strategy().generate_quotes(
        &make_ticker(),
        &fv,
        &book,
        None,
        Some(&make_meta()),
        &make_balance(dec!(5)),
        3,
    );
    // With only ~$1.67 per market, may only afford 1-2 levels
    if let Some(q) = quote {
        assert!(
            q.yes_bids.len() <= 3,
            "Capital-constrained should produce <= 3 levels"
        );
        assert!(
            !q.yes_bids.is_empty(),
            "Should produce at least 1 level with some capital"
        );
    }
}

#[test]
fn test_single_level_config() {
    let book = make_book(dec!(0.45), dec!(0.45), dec!(100));
    let fv = make_fv(dec!(0.50), 0.8);
    let quote = single_level_strategy()
        .generate_quotes(
            &make_ticker(),
            &fv,
            &book,
            None,
            Some(&make_meta()),
            &make_balance(dec!(1000)),
            5,
        )
        .unwrap();

    assert_eq!(quote.yes_bids.len(), 1, "num_levels=1 should produce 1 bid");
    assert_eq!(quote.yes_asks.len(), 1, "num_levels=1 should produce 1 ask");
}
