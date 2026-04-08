use chrono::Utc;
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
    disconnect_timeout_secs: u64,
    max_capital_per_market: Decimal,
    max_portfolio_utilization: Decimal,
    max_fair_deviation: Decimal,
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
            disconnect_timeout_secs: config.disconnect_timeout_secs,
            max_capital_per_market: config.max_capital_per_market,
            max_portfolio_utilization: config.max_portfolio_utilization,
            max_fair_deviation: config.max_fair_deviation,
        }
    }

    pub fn disconnect_timeout_secs(&self) -> u64 {
        self.disconnect_timeout_secs
    }

    /// Check if kill switch should trigger. Returns None if OK, Some(reason) if triggered.
    pub fn kill_switch_check(&self, state: &StateEngine) -> Option<String> {
        if self.cancel_all_on_disconnect
            && state.ever_connected()
            && state.connectivity() == ConnectivityState::Disconnected
        {
            let disconnected_for_secs = state.disconnected_for_secs(Utc::now()).unwrap_or(0);
            if disconnected_for_secs >= self.disconnect_timeout_secs as i64 {
                return Some(format!(
                    "Exchange disconnected for {}s (timeout {}s)",
                    disconnected_for_secs, self.disconnect_timeout_secs
                ));
            }
        }

        // Skip daily-loss check until the equity baseline has been established.
        // daily_start_equity is 0 at boot before the first REST balance sync completes.
        if state.daily_start_equity() > Decimal::ZERO
            && state.daily_total_pnl() < -self.max_loss_daily
        {
            return Some(format!(
                "Daily loss {} exceeds limit {}",
                state.daily_total_pnl(),
                self.max_loss_daily
            ));
        }

        let reserved = state.total_reserved();
        if reserved > self.max_total_reserved {
            return Some(format!(
                "Total reserved {} exceeds limit {}",
                reserved, self.max_total_reserved
            ));
        }

        // Portfolio utilization check
        let balance = state.balance().available;
        if balance > Decimal::ZERO {
            let utilization = reserved / (balance + reserved);
            if utilization > self.max_portfolio_utilization {
                return Some(format!(
                    "Portfolio utilization {:.2}% exceeds limit {:.2}%",
                    utilization * Decimal::new(100, 0),
                    self.max_portfolio_utilization * Decimal::new(100, 0)
                ));
            }
        }

        None
    }

    /// Approve or reject a single desired action.
    pub fn approve(&self, action: &DesiredAction, state: &StateEngine) -> RiskDecision {
        match action {
            DesiredAction::CreateOrder {
                market_ticker,
                side,
                action: order_action,
                price,
                quantity,
                ..
            } => {
                if state.open_order_count() >= self.max_open_orders as usize {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Open order count {} at limit {}",
                            state.open_order_count(),
                            self.max_open_orders
                        ),
                    };
                }

                // Position-aware inventory check: allow orders that REDUCE exposure
                if let Some(pos) = state.get_position(market_ticker) {
                    let net = pos.net_inventory();
                    let abs_net = net.abs();
                    let limit = Decimal::from(self.max_market_inventory);

                    if abs_net >= limit {
                        let would_reduce = match (side, order_action) {
                            (Side::Yes, Action::Sell) => net > Decimal::ZERO,
                            (Side::No, Action::Sell) => net < Decimal::ZERO,
                            (Side::Yes, Action::Buy) => net < Decimal::ZERO,
                            (Side::No, Action::Buy) => net > Decimal::ZERO,
                        };
                        if !would_reduce {
                            return RiskDecision::Rejected {
                                reason: format!(
                                    "Market {} inventory {} at limit {}, order would increase",
                                    market_ticker, abs_net, self.max_market_inventory
                                ),
                            };
                        }
                    }
                }

                // Per-market notional
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

                // Per-market capital cap
                if existing_notional + notional > self.max_capital_per_market {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Market {} capital would be {} (limit {})",
                            market_ticker,
                            existing_notional + notional,
                            self.max_capital_per_market
                        ),
                    };
                }

                // Total reserved capital
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

    /// Check a target quote before it's converted to orders.
    /// Validates all levels: inverted spreads, fat-finger deviations, and expired markets.
    pub fn check_target_quote(
        &self,
        quote: &TargetQuote,
        state: &StateEngine,
        fair_price: Option<Decimal>,
    ) -> RiskDecision {
        // Inverted spread check: each level's ask must exceed the corresponding bid
        let check_len = quote.yes_bids.len().min(quote.yes_asks.len());
        for i in 0..check_len {
            if quote.yes_asks[i].price <= quote.yes_bids[i].price {
                return RiskDecision::Rejected {
                    reason: format!(
                        "Inverted spread at level {}: bid={} >= ask={}",
                        i, quote.yes_bids[i].price, quote.yes_asks[i].price
                    ),
                };
            }
        }

        // Fat-finger: reject if any level deviates too far from fair value
        if let Some(fair) = fair_price {
            for (i, bid) in quote.yes_bids.iter().enumerate() {
                let dev = (bid.price - fair).abs();
                if dev > self.max_fair_deviation {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Bid level {} price {:.4} deviates {:.4} from fair {:.4} (limit {:.4})",
                            i, bid.price, dev, fair, self.max_fair_deviation
                        ),
                    };
                }
            }
            for (i, ask) in quote.yes_asks.iter().enumerate() {
                let dev = (ask.price - fair).abs();
                if dev > self.max_fair_deviation {
                    return RiskDecision::Rejected {
                        reason: format!(
                            "Ask level {} price {:.4} deviates {:.4} from fair {:.4} (limit {:.4})",
                            i, ask.price, dev, fair, self.max_fair_deviation
                        ),
                    };
                }
            }
        }

        // Expiry guard: don't quote markets too close to expiry
        if let Some(meta) = state.get_market_meta(&quote.market_ticker) {
            let hours = meta.hours_to_expiry();
            if hours < 0.5 {
                return RiskDecision::Rejected {
                    reason: format!(
                        "Market {} expiring in {:.1}h, too close",
                        quote.market_ticker, hours
                    ),
                };
            }
        }

        RiskDecision::Approved
    }
}
