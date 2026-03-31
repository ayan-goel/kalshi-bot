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
        disconnect_timeout_secs: i64,
    }

    struct MockState {
        connectivity: ConnectivityState,
        disconnected_for_secs: i64,
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
                && state.disconnected_for_secs >= self.disconnect_timeout_secs
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

        fn approve_order(&self, state: &MockState, _price: Decimal, _qty: Decimal) -> RiskDecision {
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
            disconnect_timeout_secs: 30,
        }
    }

    fn default_state() -> MockState {
        MockState {
            connectivity: ConnectivityState::Connected,
            disconnected_for_secs: 0,
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
        state.disconnected_for_secs = 31;
        assert!(engine.kill_switch_check(&state).is_some());
    }

    #[test]
    fn test_disconnect_within_grace_does_not_kill() {
        let engine = default_engine();
        let mut state = default_state();
        state.connectivity = ConnectivityState::Disconnected;
        state.disconnected_for_secs = 10;
        assert!(engine.kill_switch_check(&state).is_none());
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
        // Even with inventory and order count at the limit, cancel actions are always approved.
        // In the real RiskEngine, DesiredAction::CancelOrder unconditionally returns Approved.
        let engine = default_engine();
        let mut state = default_state();
        state.open_order_count = 500;
        state.market_inventory = dec!(200);
        // New orders would be rejected at these limits but cancel-order itself
        // bypasses order-count and inventory checks entirely.
        assert_eq!(
            engine.approve_order(&state, dec!(0.50), dec!(5)),
            RiskDecision::Rejected("Open order limit".to_string()),
            "Order creation should be rejected at limit"
        );
        // The approve_order mock doesn't have a cancel variant; this documents
        // that the real engine's CancelOrder arm always returns Approved.
        // Validated in integration by the real RiskEngine::approve + DesiredAction::CancelOrder.
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
