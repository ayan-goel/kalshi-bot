use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[cfg(test)]
mod execution {
    use super::*;

    #[derive(Debug, Clone)]
    struct LiveOrder {
        order_id: String,
        price: Decimal,
        remaining_count: Decimal,
    }

    #[derive(Debug, Clone)]
    struct TargetLevel {
        price: Decimal,
        quantity: Decimal,
    }

    #[derive(Debug, PartialEq)]
    enum DiffAction {
        Hold,
        Cancel(String),
        Create(Decimal, Decimal),
        Replace(String, Decimal, Decimal),
    }

    fn diff_side(
        live: Option<&LiveOrder>,
        desired: Option<&TargetLevel>,
        repricing_threshold: Decimal,
    ) -> DiffAction {
        match (live, desired) {
            (None, None) => DiffAction::Hold,
            (Some(order), None) => DiffAction::Cancel(order.order_id.clone()),
            (None, Some(target)) => DiffAction::Create(target.price, target.quantity),
            (Some(order), Some(target)) => {
                let price_diff = (order.price - target.price).abs();
                let qty_diff = (order.remaining_count - target.quantity).abs();
                if price_diff >= repricing_threshold || qty_diff >= Decimal::ONE {
                    DiffAction::Replace(order.order_id.clone(), target.price, target.quantity)
                } else {
                    DiffAction::Hold
                }
            }
        }
    }

    #[test]
    fn test_no_live_no_desired() {
        let result = diff_side(None, None, dec!(0.01));
        assert_eq!(result, DiffAction::Hold);
    }

    #[test]
    fn test_live_no_desired_cancels() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5),
        };
        let result = diff_side(Some(&live), None, dec!(0.01));
        assert_eq!(result, DiffAction::Cancel("order-1".to_string()));
    }

    #[test]
    fn test_no_live_desired_creates() {
        let target = TargetLevel {
            price: dec!(0.45),
            quantity: dec!(5),
        };
        let result = diff_side(None, Some(&target), dec!(0.01));
        assert_eq!(result, DiffAction::Create(dec!(0.45), dec!(5)));
    }

    #[test]
    fn test_matching_holds() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5),
        };
        let target = TargetLevel {
            price: dec!(0.45),
            quantity: dec!(5),
        };
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(result, DiffAction::Hold);
    }

    #[test]
    fn test_price_change_replaces() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5),
        };
        let target = TargetLevel {
            price: dec!(0.47),
            quantity: dec!(5),
        };
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(
            result,
            DiffAction::Replace("order-1".to_string(), dec!(0.47), dec!(5))
        );
    }

    #[test]
    fn test_small_price_change_holds() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.450),
            remaining_count: dec!(5),
        };
        let target = TargetLevel {
            price: dec!(0.455),
            quantity: dec!(5),
        };
        // diff = 0.005 < threshold 0.01 -> hold
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(result, DiffAction::Hold);
    }

    #[test]
    fn test_qty_change_replaces() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5),
        };
        let target = TargetLevel {
            price: dec!(0.45),
            quantity: dec!(10),
        };
        // qty diff = 5 >= 1 -> replace
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(
            result,
            DiffAction::Replace("order-1".to_string(), dec!(0.45), dec!(10))
        );
    }

    #[test]
    fn test_small_qty_change_holds() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5.0),
        };
        let target = TargetLevel {
            price: dec!(0.45),
            quantity: dec!(5.5),
        };
        // qty diff = 0.5 < 1 -> hold
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(result, DiffAction::Hold);
    }

    #[test]
    fn test_both_price_and_qty_change() {
        let live = LiveOrder {
            order_id: "order-1".to_string(),
            price: dec!(0.45),
            remaining_count: dec!(5),
        };
        let target = TargetLevel {
            price: dec!(0.50),
            quantity: dec!(10),
        };
        let result = diff_side(Some(&live), Some(&target), dec!(0.01));
        assert_eq!(
            result,
            DiffAction::Replace("order-1".to_string(), dec!(0.50), dec!(10))
        );
    }
}
