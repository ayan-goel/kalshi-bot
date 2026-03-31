use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use tokio::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::StrategyConfig;
use crate::exchange::models::CreateOrderRequest;
use crate::exchange::rest::KalshiRestClient;
use crate::risk::RiskEngine;
use crate::state::{LiveOrder, StateEngine};
use crate::types::*;

/// Execution engine: diffs target quotes against live orders, emits create/cancel actions.
pub struct ExecutionEngine {
    rest_client: KalshiRestClient,
    db_pool: PgPool,
    repricing_threshold: Decimal,
    min_rest_ms: u64,
    last_action_time: HashMap<MarketTicker, Instant>,
}

impl ExecutionEngine {
    pub fn new(rest_client: KalshiRestClient, db_pool: PgPool, config: &StrategyConfig) -> Self {
        Self {
            rest_client,
            db_pool,
            repricing_threshold: config.repricing_threshold,
            min_rest_ms: config.min_rest_ms,
            last_action_time: HashMap::new(),
        }
    }

    /// Cancel all open orders (kill switch / shutdown).
    pub async fn cancel_all(&mut self, state: &StateEngine) {
        let order_ids: Vec<String> = state.open_orders().keys().cloned().collect();

        if order_ids.is_empty() {
            return;
        }

        info!(count = order_ids.len(), "Cancelling all orders");

        // Batch cancel in groups of 20
        for chunk in order_ids.chunks(20) {
            match self.rest_client.batch_cancel_orders(chunk.to_vec()).await {
                Ok(orders) => {
                    info!(cancelled = orders.len(), "Batch cancel success");
                }
                Err(e) => {
                    warn!(error = %e, "Batch cancel failed, trying individual cancels");
                    for oid in chunk {
                        if let Err(e) = self.rest_client.cancel_order(oid).await {
                            warn!(order_id = %oid, error = %e, "Individual cancel failed");
                        }
                    }
                }
            }
        }

        let _ = crate::db::insert_risk_event(
            &self.db_pool,
            "critical",
            "execution",
            None,
            "Cancel-all triggered",
            None,
        )
        .await;
    }

    /// Reconcile target quotes with live orders: cancel stale, create missing, update changed.
    /// Returns the set of markets that returned `invalid_order` so the caller can blacklist them.
    pub async fn reconcile(
        &mut self,
        state: &StateEngine,
        targets: &[TargetQuote],
        risk_engine: &RiskEngine,
    ) -> Vec<MarketTicker> {
        let mut invalid_order_markets: Vec<MarketTicker> = Vec::new();
        // Build a map of desired quotes by market
        let mut desired: HashMap<MarketTicker, &TargetQuote> = HashMap::new();
        for target in targets {
            desired.insert(target.market_ticker.clone(), target);
        }

        // Collect all market tickers involved
        let mut all_markets: Vec<MarketTicker> = desired.keys().cloned().collect();
        for order in state.open_orders().values() {
            if !all_markets.contains(&order.market_ticker) {
                all_markets.push(order.market_ticker.clone());
            }
        }

        let mut cancels: Vec<String> = Vec::new();
        let mut creates: Vec<CreateOrderRequest> = Vec::new();

        for market in &all_markets {
            // Anti-churn: skip if we acted too recently
            if let Some(last) = self.last_action_time.get(market) {
                if last.elapsed().as_millis() < self.min_rest_ms as u128 {
                    continue;
                }
            }

            let live_orders = state.orders_for_market(market);
            let target = desired.get(market);

            // Separate live orders into bid-side and ask-side
            // Bug 15: sort for deterministic primary selection
            let mut live_bids: Vec<&LiveOrder> = live_orders
                .iter()
                .filter(|o| o.action == Action::Buy && o.side == Side::Yes)
                .copied()
                .collect();
            live_bids.sort_by(|a, b| b.price.cmp(&a.price)); // highest bid first

            let mut live_asks: Vec<&LiveOrder> = live_orders
                .iter()
                .filter(|o| {
                    // Selling YES or buying NO are equivalent ask-side actions
                    (o.action == Action::Sell && o.side == Side::Yes)
                        || (o.action == Action::Buy && o.side == Side::No)
                })
                .copied()
                .collect();
            live_asks.sort_by(|a, b| a.price.cmp(&b.price)); // lowest ask first

            // Bug 14: cancel any orders that don't match bid or ask categories
            // (e.g. Sell No) to prevent order leaks
            let classified_ids: std::collections::HashSet<&str> = live_bids
                .iter()
                .chain(live_asks.iter())
                .map(|o| o.order_id.as_str())
                .collect();
            for order in &live_orders {
                if !classified_ids.contains(order.order_id.as_str()) {
                    debug!(
                        order_id = %order.order_id,
                        market = %market,
                        "Cancelling unclassified order to prevent leak"
                    );
                    cancels.push(order.order_id.clone());
                }
            }

            match target {
                Some(tq) => {
                    // Reconcile bid side
                    self.reconcile_side(
                        &live_bids,
                        tq.yes_bid.as_ref(),
                        market,
                        Side::Yes,
                        Action::Buy,
                        &mut cancels,
                        &mut creates,
                    );

                    // Reconcile ask side (we sell YES, equivalent to buying NO)
                    self.reconcile_side(
                        &live_asks,
                        tq.yes_ask.as_ref(),
                        market,
                        Side::Yes,
                        Action::Sell,
                        &mut cancels,
                        &mut creates,
                    );

                    // Log decision
                    let _ = crate::db::insert_strategy_decision(
                        &self.db_pool,
                        &market.0,
                        tq.yes_bid
                            .as_ref()
                            .map(|b| b.price)
                            .unwrap_or(Decimal::ZERO),
                        state
                            .get_position(market)
                            .map(|p| p.net_inventory())
                            .unwrap_or(Decimal::ZERO),
                        &serde_json::to_value(tq.reason.clone()).unwrap_or_default(),
                        &tq.reason,
                    )
                    .await;
                }
                None => {
                    // No target for this market: cancel all live orders
                    for order in &live_bids {
                        cancels.push(order.order_id.clone());
                    }
                    for order in &live_asks {
                        cancels.push(order.order_id.clone());
                    }
                }
            }
        }

        // Execute cancels — Bug 2: fall back to per-order cancel if batch fails
        if !cancels.is_empty() {
            debug!(count = cancels.len(), "Executing cancels");
            for chunk in cancels.chunks(20) {
                if let Err(e) = self.rest_client.batch_cancel_orders(chunk.to_vec()).await {
                    warn!(
                        error = %e,
                        count = chunk.len(),
                        "Batch cancel failed in reconcile, falling back to per-order"
                    );
                    for oid in chunk {
                        if let Err(e2) = self.rest_client.cancel_order(oid).await {
                            warn!(order_id = %oid, error = %e2, "Individual cancel failed in reconcile");
                        }
                    }
                }
            }
        }

        // Execute creates — Bug 10: check risk approval before each order
        for req in &creates {
            let ticker = MarketTicker::from(req.ticker.as_str());

            // Reconstruct typed values for the risk check
            let side = match req.side.as_str() {
                "yes" => Side::Yes,
                _ => Side::No,
            };
            let order_action = match req.action.as_str() {
                "buy" => Action::Buy,
                _ => Action::Sell,
            };
            let price = req
                .yes_price_dollars
                .as_deref()
                .or(req.no_price_dollars.as_deref())
                .and_then(|s| s.parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO);
            let quantity = req.count.map(Decimal::from).unwrap_or(Decimal::ONE);

            let desired_action = DesiredAction::CreateOrder {
                market_ticker: ticker.clone(),
                side,
                action: order_action,
                price,
                quantity,
                client_order_id: req.client_order_id.clone().unwrap_or_default(),
            };

            match risk_engine.approve(&desired_action, state) {
                RiskDecision::Approved => {}
                RiskDecision::Rejected { reason } => {
                    warn!(ticker = %ticker, reason = %reason, "Order rejected by risk engine");
                    continue;
                }
                RiskDecision::KillSwitch { reason } => {
                    warn!(ticker = %ticker, reason = %reason, "Kill switch from risk engine during create");
                    continue;
                }
            }

            match self.rest_client.create_order(req).await {
                Ok(resp) => {
                    info!(
                        order_id = %resp.order_id,
                        ticker = %req.ticker,
                        action = %req.action,
                        side = %req.side,
                        price = req.yes_price_dollars.as_deref().or(req.no_price_dollars.as_deref()).unwrap_or("?"),
                        count = ?req.count,
                        "Order created"
                    );
                    self.last_action_time.insert(ticker, Instant::now());
                }
                Err(e) => {
                    let err_str = format!("{e:?}");
                    let req_json = serde_json::to_string(req).unwrap_or_default();
                    warn!(
                        ticker = %req.ticker,
                        action = %req.action,
                        price = req.yes_price_dollars.as_deref().or(req.no_price_dollars.as_deref()).unwrap_or("?"),
                        error = %e,
                        request_json = %req_json,
                        "Order creation failed"
                    );
                    // Surface markets that return invalid_order so the caller
                    // can blacklist them for the current session.
                    if err_str.contains("invalid_order") {
                        invalid_order_markets.push(ticker);
                    }
                }
            }
        }

        invalid_order_markets
    }

    fn reconcile_side(
        &self,
        live: &[&LiveOrder],
        desired: Option<&PriceLevel>,
        market: &MarketTicker,
        side: Side,
        action: Action,
        cancels: &mut Vec<String>,
        creates: &mut Vec<CreateOrderRequest>,
    ) {
        match (live.first(), desired) {
            (None, None) => {
                // Nothing to do
            }
            (Some(order), None) => {
                // Live order exists but no desired -> cancel
                cancels.push(order.order_id.clone());
                // Cancel extras too
                for extra in &live[1..] {
                    cancels.push(extra.order_id.clone());
                }
            }
            (None, Some(target)) => {
                // No live order, desired exists -> create
                creates.push(build_create_request(market, side, action, target));
            }
            (Some(order), Some(target)) => {
                // Cancel extras first
                for extra in &live[1..] {
                    cancels.push(extra.order_id.clone());
                }

                // Check if repricing needed
                let price_diff = (order.price - target.price).abs();
                let qty_diff = (order.remaining_count - target.quantity).abs();

                if price_diff >= self.repricing_threshold || qty_diff >= Decimal::ONE {
                    cancels.push(order.order_id.clone());
                    creates.push(build_create_request(market, side, action, target));
                }
                // else: hold
            }
        }
    }
}

fn build_create_request(
    market: &MarketTicker,
    side: Side,
    action: Action,
    level: &PriceLevel,
) -> CreateOrderRequest {
    let client_id = Uuid::new_v4().to_string();

    // Kalshi requires prices as fixed-point strings with exactly 4 decimal places
    let price_str = format!("{:.4}", level.price);
    let (yes_price_dollars, no_price_dollars) = match side {
        Side::Yes => (Some(price_str), None),
        Side::No => (None, Some(price_str)),
    };

    CreateOrderRequest {
        ticker: market.0.clone(),
        side: side.to_string(),
        action: action.to_string(),
        order_type: "limit".to_string(),
        count: Some(level.quantity.to_string().parse::<i64>().unwrap_or(1)),
        count_fp: Some(format!("{:.2}", level.quantity)),
        yes_price: None,
        no_price: None,
        yes_price_dollars,
        no_price_dollars,
        client_order_id: Some(client_id),
        time_in_force: Some("good_till_canceled".to_string()),
        post_only: Some(true),
    }
}
