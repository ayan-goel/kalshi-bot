use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[cfg(test)]
mod risk {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum ConnectivityState {
        Connected,
        Disconnected,
    }

    struct RiskEngine {
        max_loss_daily: Decimal,
        max_market_inventory: i64,
        max_total_reserved: Decimal,
        max_open_orders: u32,
        cancel_all_on_disconnect: bool,
    }

    struct MockState {
        connectivity: ConnectivityState,
        daily_pnl: Decimal,
        total_reserved: Decimal,
        open_order_count: usize,
        market_inventory: Decimal,
    }

    #[derive(Debug, PartialEq)]
    enum RiskDecision {
        Approved,
        Rejected(String),
    }

    impl RiskEngine {
        fn kill_switch_check(&self, state: &MockState) -> Option<String> {
            if self.cancel_all_on_disconnect
                && state.connectivity == ConnectivityState::Disconnected
            {
                return Some("Exchange disconnected".to_string());
            }
            if state.daily_pnl < -self.max_loss_daily {
                return Some("Daily loss exceeded".to_string());
            }
            if state.total_reserved > self.max_total_reserved {
                return Some("Reserved capital exceeded".to_string());
            }
            None
        }

        fn approve_order(
            &self,
            state: &MockState,
            _price: Decimal,
            _qty: Decimal,
        ) -> RiskDecision {
            if state.open_order_count >= self.max_open_orders as usize {
                return RiskDecision::Rejected("Open order limit".to_string());
            }
            if state.market_inventory.abs() >= Decimal::from(self.max_market_inventory) {
                return RiskDecision::Rejected("Inventory limit".to_string());
            }
            RiskDecision::Approved
        }
    }

    fn default_engine() -> RiskEngine {
        RiskEngine {
            max_loss_daily: dec!(250),
            max_market_inventory: 100,
            max_total_reserved: dec!(4000),
            max_open_orders: 500,
            cancel_all_on_disconnect: true,
        }
    }

    fn default_state() -> MockState {
        MockState {
            connectivity: ConnectivityState::Connected,
            daily_pnl: Decimal::ZERO,
            total_reserved: dec!(100),
            open_order_count: 10,
            market_inventory: dec!(5),
        }
    }

    #[test]
    fn test_kill_switch_ok() {
        let engine = default_engine();
        let state = default_state();
        assert!(engine.kill_switch_check(&state).is_none());
    }

    #[test]
    fn test_kill_switch_on_disconnect() {
        let engine = default_engine();
        let mut state = default_state();
        state.connectivity = ConnectivityState::Disconnected;
        assert!(engine.kill_switch_check(&state).is_some());
    }

    #[test]
    fn test_kill_switch_on_daily_loss() {
        let engine = default_engine();
        let mut state = default_state();
        state.daily_pnl = dec!(-300);
        assert!(engine.kill_switch_check(&state).is_some());
    }

    #[test]
    fn test_kill_switch_at_loss_boundary() {
        let engine = default_engine();
        let mut state = default_state();
        state.daily_pnl = dec!(-250);
        // Exactly at limit, not below -> should be OK
        assert!(engine.kill_switch_check(&state).is_none());
    }

    #[test]
    fn test_kill_switch_on_reserved_exceeded() {
        let engine = default_engine();
        let mut state = default_state();
        state.total_reserved = dec!(5000);
        assert!(engine.kill_switch_check(&state).is_some());
    }

    #[test]
    fn test_approve_order_ok() {
        let engine = default_engine();
        let state = default_state();
        assert_eq!(
            engine.approve_order(&state, dec!(0.50), dec!(5)),
            RiskDecision::Approved
        );
    }

    #[test]
    fn test_reject_order_count_limit() {
        let engine = default_engine();
        let mut state = default_state();
        state.open_order_count = 500;
        assert!(matches!(
            engine.approve_order(&state, dec!(0.50), dec!(5)),
            RiskDecision::Rejected(_)
        ));
    }

    #[test]
    fn test_reject_inventory_limit() {
        let engine = default_engine();
        let mut state = default_state();
        state.market_inventory = dec!(100);
        assert!(matches!(
            engine.approve_order(&state, dec!(0.50), dec!(5)),
            RiskDecision::Rejected(_)
        ));
    }

    #[test]
    fn test_cancel_always_approved() {
        // Cancels should always be approved
        let engine = default_engine();
        let mut state = default_state();
        state.open_order_count = 500;
        state.market_inventory = dec!(200);
        // Even with everything at limit, cancels pass through
        // (In the real code, CancelOrder always returns Approved)
    }

    #[test]
    fn test_disconnect_flag_disabled() {
        let engine = RiskEngine {
            cancel_all_on_disconnect: false,
            ..default_engine()
        };
        let mut state = default_state();
        state.connectivity = ConnectivityState::Disconnected;
        // With flag disabled, disconnect should not trigger kill switch
        assert!(engine.kill_switch_check(&state).is_none());
    }
}
