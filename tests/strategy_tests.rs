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

#[test]
fn test_basic_quoting() {
    // Spread = 0.10, fair = 0.50, balanced book → should get bid < 0.50 < ask
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

    assert!(quote.yes_bid.as_ref().unwrap().price < dec!(0.50));
    assert!(quote.yes_ask.as_ref().unwrap().price > dec!(0.50));
    assert!(quote.yes_ask.as_ref().unwrap().price > quote.yes_bid.as_ref().unwrap().price);
    assert!(quote.yes_bid.as_ref().unwrap().quantity > Decimal::ZERO);
}

#[test]
fn test_spread_too_tight_returns_none() {
    // If market spread (0.02) < fee_half_spread * 2, strategy should not quote
    let book = make_book(dec!(0.49), dec!(0.51), dec!(100)); // spread = 0.02
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
    let fv = make_fv(dec!(0.50), 0.05); // below 0.1 threshold
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
    let s = strategy();
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

    let neutral_mid = (neutral.yes_bid.unwrap().price + neutral.yes_ask.unwrap().price) / dec!(2);
    let long_mid = (long.yes_bid.unwrap().price + long.yes_ask.unwrap().price) / dec!(2);
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
    let s = strategy();
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

    let neutral_mid = (neutral.yes_bid.unwrap().price + neutral.yes_ask.unwrap().price) / dec!(2);
    let short_mid = (short.yes_bid.unwrap().price + short.yes_ask.unwrap().price) / dec!(2);
    assert!(
        short_mid > neutral_mid,
        "Short inventory should shift quotes up: {short_mid} vs {neutral_mid}"
    );
}

#[test]
fn test_zero_capital_returns_none() {
    // With zero available balance, compute_size should return None → generate_quotes returns None
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
    let meta = make_meta(); // tick_min = 0.01, tick_max = 0.99
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
    let bid = quote.yes_bid.unwrap().price;
    let ask = quote.yes_ask.unwrap().price;
    assert!(bid >= dec!(0.01), "Bid {bid} below tick_min");
    assert!(ask <= dec!(0.99), "Ask {ask} above tick_max");
    assert!(ask > bid, "Ask must be above bid");
}

#[test]
fn test_wider_book_spread_widens_quotes() {
    // Wider observable spread → vol_adj → wider quotes
    let narrow_book = make_book(dec!(0.47), dec!(0.47), dec!(100)); // spread 0.06
    let wide_book = make_book(dec!(0.40), dec!(0.40), dec!(100)); // spread 0.20
    let fv = make_fv(dec!(0.50), 0.8);
    let bal = make_balance(dec!(1000));
    let s = strategy();
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

    let narrow_half =
        (narrow.yes_ask.as_ref().unwrap().price - narrow.yes_bid.as_ref().unwrap().price) / dec!(2);
    let wide_half =
        (wide.yes_ask.as_ref().unwrap().price - wide.yes_bid.as_ref().unwrap().price) / dec!(2);
    assert!(
        wide_half > narrow_half,
        "Wider book should widen quotes: {wide_half} vs {narrow_half}"
    );
}
