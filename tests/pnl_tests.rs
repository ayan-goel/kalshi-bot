/// Unit tests for realized PnL accounting logic.
///
/// These test the round-trip PnL formula in isolation, mirroring the logic
/// in StateEngine::process_event for Fill events (src/state/mod.rs).
///
/// The rule: realized PnL is only recorded when a position is REDUCED.
/// Opening a position records only -fee as realized (cost of entry).
/// Closing records (exit_price - avg_entry_price) * qty - fee.
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Minimal position tracker mirroring the logic in state/mod.rs process_event fill branch.
#[derive(Debug, Default)]
struct TestPosition {
    yes_contracts: Decimal,
    no_contracts: Decimal,
    avg_yes_price: Option<Decimal>,
    avg_no_price: Option<Decimal>,
    realized_pnl: Decimal,
}

impl TestPosition {
    fn fill_yes_buy(&mut self, price: Decimal, count: Decimal, fee: Decimal) -> Decimal {
        let prev_qty = self.yes_contracts;
        if prev_qty > Decimal::ZERO {
            let avg = self.avg_yes_price.unwrap_or(price);
            self.avg_yes_price = Some((avg * prev_qty + price * count) / (prev_qty + count));
        } else {
            self.avg_yes_price = Some(price);
        }
        self.yes_contracts += count;
        let realized = -fee;
        self.realized_pnl += realized;
        realized
    }

    fn fill_yes_sell(&mut self, price: Decimal, count: Decimal, fee: Decimal) -> Decimal {
        let avg = self.avg_yes_price.unwrap_or(price);
        let pnl = (price - avg) * count - fee;
        self.yes_contracts -= count;
        if self.yes_contracts <= Decimal::ZERO {
            self.avg_yes_price = None;
        }
        self.realized_pnl += pnl;
        pnl
    }
}

fn fee(price: Decimal, count: Decimal) -> Decimal {
    // Kalshi maker fee ~1.75% of notional
    (price * count * dec!(0.0175)).round_dp(4)
}

#[test]
fn test_buy_open_only_costs_fee() {
    let mut pos = TestPosition::default();
    let f = fee(dec!(0.40), dec!(5));
    let realized = pos.fill_yes_buy(dec!(0.40), dec!(5), f);

    // Opening: realized PnL == -fee only
    assert_eq!(realized, -f);
    assert_eq!(pos.yes_contracts, dec!(5));
    assert_eq!(pos.avg_yes_price, Some(dec!(0.40)));
    assert_eq!(pos.realized_pnl, -f);
}

#[test]
fn test_round_trip_profitable() {
    let mut pos = TestPosition::default();
    let buy_fee = fee(dec!(0.40), dec!(5));
    let sell_fee = fee(dec!(0.42), dec!(5));

    pos.fill_yes_buy(dec!(0.40), dec!(5), buy_fee);
    let realized_close = pos.fill_yes_sell(dec!(0.42), dec!(5), sell_fee);

    // Realized on close = (0.42 - 0.40) * 5 - sell_fee = 0.10 - sell_fee
    let expected_close = dec!(0.02) * dec!(5) - sell_fee;
    assert_eq!(realized_close, expected_close);

    // Total session realized = -buy_fee + (0.10 - sell_fee)
    let expected_total = -buy_fee + expected_close;
    assert_eq!(pos.realized_pnl, expected_total);
    assert!(pos.realized_pnl > Decimal::ZERO, "Profitable round-trip should show positive PnL");
    assert_eq!(pos.yes_contracts, dec!(0));
    assert_eq!(pos.avg_yes_price, None);
}

#[test]
fn test_round_trip_losing() {
    let mut pos = TestPosition::default();
    let buy_fee = fee(dec!(0.55), dec!(3));
    let sell_fee = fee(dec!(0.50), dec!(3));

    pos.fill_yes_buy(dec!(0.55), dec!(3), buy_fee);
    let realized_close = pos.fill_yes_sell(dec!(0.50), dec!(3), sell_fee);

    // (0.50 - 0.55) * 3 - sell_fee = -0.15 - sell_fee
    let expected_close = (dec!(0.50) - dec!(0.55)) * dec!(3) - sell_fee;
    assert_eq!(realized_close, expected_close);
    assert!(pos.realized_pnl < Decimal::ZERO, "Losing round-trip should show negative PnL");
}

#[test]
fn test_partial_close_updates_position_correctly() {
    let mut pos = TestPosition::default();
    pos.fill_yes_buy(dec!(0.40), dec!(10), fee(dec!(0.40), dec!(10)));
    // Close half
    pos.fill_yes_sell(dec!(0.42), dec!(5), fee(dec!(0.42), dec!(5)));

    assert_eq!(pos.yes_contracts, dec!(5));
    assert_eq!(pos.avg_yes_price, Some(dec!(0.40))); // avg unchanged
}

#[test]
fn test_average_cost_basis_updates_on_add() {
    let mut pos = TestPosition::default();
    pos.fill_yes_buy(dec!(0.40), dec!(4), fee(dec!(0.40), dec!(4)));
    pos.fill_yes_buy(dec!(0.60), dec!(4), fee(dec!(0.60), dec!(4)));

    // avg = (0.40*4 + 0.60*4) / 8 = 4.0 / 8 = 0.50
    let avg = pos.avg_yes_price.unwrap();
    assert_eq!(avg, dec!(0.50));
    assert_eq!(pos.yes_contracts, dec!(8));
}

#[test]
fn test_full_close_clears_avg_price() {
    let mut pos = TestPosition::default();
    pos.fill_yes_buy(dec!(0.45), dec!(3), fee(dec!(0.45), dec!(3)));
    pos.fill_yes_sell(dec!(0.50), dec!(3), fee(dec!(0.50), dec!(3)));

    assert_eq!(pos.yes_contracts, dec!(0));
    assert_eq!(pos.avg_yes_price, None);
}
