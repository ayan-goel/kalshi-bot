use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::BTreeMap;

#[cfg(test)]
mod fair_value {
    use super::*;

    struct FairValueEngine {
        order_imbalance_alpha: Decimal,
        inventory_penalty_k1: Decimal,
        inventory_penalty_k3: Decimal,
    }

    struct BookSignals {
        microprice: Decimal,
        imbalance: Decimal,
    }

    impl FairValueEngine {
        fn compute(&self, signals: &BookSignals, inventory: Decimal) -> Decimal {
            let imbalance_adj = self.order_imbalance_alpha * signals.imbalance;
            let inv_adj = -self.inventory_penalty_k1 * inventory
                - self.inventory_penalty_k3 * inventory * inventory * inventory;

            let raw = signals.microprice + imbalance_adj + inv_adj;
            raw.max(dec!(0.01)).min(dec!(0.99))
        }
    }

    fn default_engine() -> FairValueEngine {
        FairValueEngine {
            order_imbalance_alpha: dec!(0.05),
            inventory_penalty_k1: dec!(0.03),
            inventory_penalty_k3: dec!(0.001),
        }
    }

    #[test]
    fn test_fair_value_no_inventory() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(0),
        };

        let fv = engine.compute(&signals, Decimal::ZERO);
        assert_eq!(fv, dec!(0.50));
    }

    #[test]
    fn test_fair_value_positive_imbalance() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(0.5), // More bid pressure
        };

        let fv = engine.compute(&signals, Decimal::ZERO);
        // 0.50 + 0.05 * 0.5 = 0.525
        assert_eq!(fv, dec!(0.525));
    }

    #[test]
    fn test_fair_value_negative_imbalance() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(-0.5), // More ask pressure
        };

        let fv = engine.compute(&signals, Decimal::ZERO);
        // 0.50 + 0.05 * (-0.5) = 0.475
        assert_eq!(fv, dec!(0.475));
    }

    #[test]
    fn test_fair_value_long_inventory_pushes_down() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(0),
        };

        let fv = engine.compute(&signals, dec!(10));
        // 0.50 + (-0.03 * 10) + (-0.001 * 1000) = 0.50 - 0.30 - 1.00 -> clamped to 0.01
        assert_eq!(fv, dec!(0.01));
    }

    #[test]
    fn test_fair_value_small_long_inventory() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(0),
        };

        let fv = engine.compute(&signals, dec!(2));
        // 0.50 + (-0.03 * 2) + (-0.001 * 8) = 0.50 - 0.06 - 0.008 = 0.432
        assert_eq!(fv, dec!(0.432));
    }

    #[test]
    fn test_fair_value_short_inventory_pushes_up() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.50),
            imbalance: dec!(0),
        };

        let fv = engine.compute(&signals, dec!(-2));
        // 0.50 + (-0.03 * -2) + (-0.001 * -8) = 0.50 + 0.06 + 0.008 = 0.568
        assert_eq!(fv, dec!(0.568));
    }

    #[test]
    fn test_fair_value_clamp_high() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.98),
            imbalance: dec!(1.0),
        };

        let fv = engine.compute(&signals, dec!(-5));
        // Should be clamped to 0.99
        assert!(fv <= dec!(0.99));
    }

    #[test]
    fn test_fair_value_clamp_low() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.02),
            imbalance: dec!(-1.0),
        };

        let fv = engine.compute(&signals, dec!(5));
        assert!(fv >= dec!(0.01));
    }

    #[test]
    fn test_deterministic_for_same_inputs() {
        let engine = default_engine();
        let signals = BookSignals {
            microprice: dec!(0.55),
            imbalance: dec!(0.3),
        };

        let fv1 = engine.compute(&signals, dec!(3));
        let fv2 = engine.compute(&signals, dec!(3));
        assert_eq!(fv1, fv2);
    }
}
