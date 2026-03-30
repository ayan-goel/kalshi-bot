use rust_decimal::Decimal;

use crate::config::RiskConfig;
use crate::state::StateEngine;
use crate::types::*;

pub struct RiskEngine {
    max_loss_daily: Decimal,
    max_market_notional: Decimal,
    max_market_inventory: i64,
    max_total_reserved: Decimal,
    max_open_orders: u32,
    cancel_all_on_disconnect: bool,
}

impl RiskEngine {
    pub fn new(config: &RiskConfig) -> Self {
        Self {
            max_loss_daily: config.max_loss_daily,
            max_market_notional: config.max_market_notional,
            max_market_inventory: config.max_market_inventory_contracts,
            max_total_reserved: config.max_total_reserved,
            max_open_orders: config.max_open_orders,
            cancel_all_on_disconnect: config.cancel_all_on_disconnect,
        }
    }

    /// Check if kill switch should trigger. Returns None if OK, Some(reason) if triggered.
    pub fn kill_switch_check(&self, state: &StateEngine) -> Option<String> {
        // Check connectivity
        if self.cancel_all_on_disconnect
            && state.connectivity() == ConnectivityState::Disconnected
        {
            return Some("Exchange disconnected".to_string());
        }

        // Check daily PnL
        if state.daily_pnl() < -self.max_loss_daily {
            return Some(format!(
                "Daily loss {} exceeds limit {}",
                state.daily_pnl(),
                self.max_loss_daily
            ));
        }

        // Check total reserved capital
        let reserved = state.total_reserved();
        if reserved > self.max_total_reserved {
            return Some(format!(
                "Total reserved {} exceeds limit {}",
                reserved, self.max_total_reserved
            ));
        }

        None
    }

    /// Approve or reject a single desired action.
    pub fn approve(&self, action: &DesiredAction, state: &StateEngine) -> RiskDecision {
        match action {
            DesiredAction::CreateOrder {
                market_ticker,
                price,
                quantity,
                ..
            } => {
                // Check open order count
                if state.open_order_count() >= self.max_open_orders as usize {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Open order count {} at limit {}",
                            state.open_order_count(),
                            self.max_open_orders
                        ),
                    };
                }

                // Check per-market inventory
                if let Some(pos) = state.get_position(market_ticker) {
                    let net = pos.net_inventory().abs();
                    if net >= Decimal::from(self.max_market_inventory) {
                        return RiskDecision::Rejected {
                            reason: format!(
                                "Market {} inventory {} at limit {}",
                                market_ticker, net, self.max_market_inventory
                            ),
                        };
                    }
                }

                // Check per-market notional
                let notional = *price * *quantity;
                let existing_notional: Decimal = state
                    .orders_for_market(market_ticker)
                    .iter()
                    .map(|o| o.price * o.remaining_count)
                    .sum();
                if existing_notional + notional > self.max_market_notional {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Market {} notional would be {} (limit {})",
                            market_ticker,
                            existing_notional + notional,
                            self.max_market_notional
                        ),
                    };
                }

                // Check total reserved
                let new_reserved = state.total_reserved() + notional;
                if new_reserved > self.max_total_reserved {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Total reserved would be {} (limit {})",
                            new_reserved, self.max_total_reserved
                        ),
                    };
                }

                RiskDecision::Approved
            }
            DesiredAction::CancelOrder { .. } => RiskDecision::Approved,
        }
    }

    /// Quick check on a target quote (both bid and ask).
    pub fn check_target_quote(
        &self,
        _quote: &TargetQuote,
        _state: &StateEngine,
    ) -> RiskDecision {
        // Target quotes are checked individually when converted to DesiredActions
        RiskDecision::Approved
    }
}
