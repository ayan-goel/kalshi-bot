use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[cfg(test)]
mod strategy {
    use super::*;

    struct StrategyParams {
        base_half_spread: Decimal,
        min_edge_after_fees: Decimal,
        default_order_size: Decimal,
        max_order_size: Decimal,
        inventory_skew_coeff: Decimal,
        volatility_widen_coeff: Decimal,
    }

    #[derive(Debug)]
    struct Quote {
        bid: Decimal,
        ask: Decimal,
        qty: Decimal,
    }

    fn default_params() -> StrategyParams {
        StrategyParams {
            base_half_spread: dec!(0.02),
            min_edge_after_fees: dec!(0.015),
            default_order_size: dec!(5),
            max_order_size: dec!(25),
            inventory_skew_coeff: dec!(0.25),
            volatility_widen_coeff: dec!(0.40),
        }
    }

    fn generate_quote(
        params: &StrategyParams,
        fair: Decimal,
        spread: Decimal,
        inventory: Decimal,
        confidence: f64,
    ) -> Option<Quote> {
        // Participation filter
        if spread < params.min_edge_after_fees * dec!(2) {
            return None;
        }
        if confidence < 0.2 {
            return None;
        }

        let inv_spread_adj = params.inventory_skew_coeff * inventory.abs() * dec!(0.1);
        let vol_adj =
            params.volatility_widen_coeff * (spread - dec!(0.02)).max(Decimal::ZERO);

        let total_half_spread = params.base_half_spread + inv_spread_adj + vol_adj;

        let skew = -params.inventory_skew_coeff * inventory * dec!(0.01);
        let mut bid = fair - total_half_spread + skew;
        let mut ask = fair + total_half_spread + skew;

        // Round
        bid = (bid * dec!(100)).floor() / dec!(100);
        ask = (ask * dec!(100)).ceil() / dec!(100);

        bid = bid.max(dec!(0.01));
        ask = ask.min(dec!(0.99));

        if ask <= bid {
            ask = bid + dec!(0.01);
            if ask > dec!(0.99) {
                return None;
            }
        }

        let conf_dec = Decimal::try_from(confidence).unwrap_or(dec!(0.5));
        let qty = (params.default_order_size * conf_dec)
            .max(Decimal::ONE)
            .min(params.max_order_size)
            .round_dp(0);

        Some(Quote { bid, ask, qty })
    }

    #[test]
    fn test_basic_quoting() {
        let params = default_params();
        let quote = generate_quote(&params, dec!(0.50), dec!(0.05), dec!(0), 0.8).unwrap();

        assert!(quote.bid < dec!(0.50));
        assert!(quote.ask > dec!(0.50));
        assert!(quote.ask > quote.bid);
        assert!(quote.qty > Decimal::ZERO);
    }

    #[test]
    fn test_spread_too_tight_filtered() {
        let params = default_params();
        // min_edge * 2 = 0.03. Spread of 0.02 < 0.03 -> filtered
        let quote = generate_quote(&params, dec!(0.50), dec!(0.02), dec!(0), 0.8);
        assert!(quote.is_none());
    }

    #[test]
    fn test_low_confidence_filtered() {
        let params = default_params();
        let quote = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 0.1);
        assert!(quote.is_none());
    }

    #[test]
    fn test_inventory_skew_long() {
        let params = default_params();
        let neutral = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 0.8).unwrap();
        let long = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(5), 0.8).unwrap();

        // Long inventory -> midpoint of quotes should shift down
        let neutral_mid = (neutral.bid + neutral.ask) / dec!(2);
        let long_mid = (long.bid + long.ask) / dec!(2);
        assert!(
            long_mid < neutral_mid,
            "Long midpoint {long_mid} should be below neutral {neutral_mid}"
        );
    }

    #[test]
    fn test_inventory_skew_short() {
        let params = default_params();
        let neutral = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 0.8).unwrap();
        let short = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(-5), 0.8).unwrap();

        // Short inventory -> midpoint of quotes should shift up
        let neutral_mid = (neutral.bid + neutral.ask) / dec!(2);
        let short_mid = (short.bid + short.ask) / dec!(2);
        assert!(
            short_mid > neutral_mid,
            "Short midpoint {short_mid} should be above neutral {neutral_mid}"
        );
    }

    #[test]
    fn test_bid_ask_clamping() {
        let params = default_params();
        // Fair near 0 -> bid should be clamped to 0.01
        let quote = generate_quote(&params, dec!(0.02), dec!(0.10), dec!(0), 0.8).unwrap();
        assert!(quote.bid >= dec!(0.01));
        assert!(quote.ask <= dec!(0.99));
        assert!(quote.ask > quote.bid);
    }

    #[test]
    fn test_bid_ask_at_high_fair() {
        let params = default_params();
        let quote = generate_quote(&params, dec!(0.98), dec!(0.10), dec!(0), 0.8).unwrap();
        assert!(quote.bid >= dec!(0.01));
        assert!(quote.ask <= dec!(0.99));
        assert!(quote.ask > quote.bid);
    }

    #[test]
    fn test_order_size_scales_with_confidence() {
        let params = default_params();
        let low_conf = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 0.3).unwrap();
        let high_conf = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 0.9).unwrap();

        assert!(high_conf.qty > low_conf.qty);
    }

    #[test]
    fn test_order_size_bounded() {
        let params = default_params();
        let quote = generate_quote(&params, dec!(0.50), dec!(0.10), dec!(0), 1.0).unwrap();

        assert!(quote.qty >= Decimal::ONE);
        assert!(quote.qty <= params.max_order_size);
    }

    #[test]
    fn test_wider_market_spread_widens_quotes() {
        let params = default_params();
        let narrow = generate_quote(&params, dec!(0.50), dec!(0.04), dec!(0), 0.8).unwrap();
        let wide = generate_quote(&params, dec!(0.50), dec!(0.20), dec!(0), 0.8).unwrap();

        let narrow_spread = narrow.ask - narrow.bid;
        let wide_spread = wide.ask - wide.bid;
        assert!(wide_spread > narrow_spread);
    }
}
