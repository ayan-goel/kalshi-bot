use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::exchange::models::{MarketResponse, OrderResponse};
use crate::orderbook::OrderBook;
use crate::types::*;

/// Metadata about a market stored alongside its order book.
#[derive(Debug, Clone, Serialize)]
pub struct MarketMeta {
    pub event_ticker: Option<String>,
    pub category: Option<String>,
    pub close_time: Option<DateTime<Utc>>,
    pub latest_expiration_time: Option<DateTime<Utc>>,
    pub volume_24h: f64,
    pub open_interest: f64,
    pub score: f64,
    pub tick_size: Decimal,
    pub tick_min: Decimal,
    pub tick_max: Decimal,
}

impl Default for MarketMeta {
    fn default() -> Self {
        Self {
            event_ticker: None,
            category: None,
            close_time: None,
            latest_expiration_time: None,
            volume_24h: 0.0,
            open_interest: 0.0,
            score: 0.0,
            tick_size: Decimal::new(1, 2), // 0.01 default
            tick_min: Decimal::new(1, 2),  // 0.01
            tick_max: Decimal::new(99, 2), // 0.99
        }
    }
}

impl MarketMeta {
    pub fn from_market_response(m: &MarketResponse, score: f64) -> Self {
        let tick_size = m
            .price_ranges
            .as_ref()
            .and_then(|pr| pr.first())
            .and_then(|r| r.step.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(1, 2));

        let tick_min = m
            .price_ranges
            .as_ref()
            .and_then(|pr| pr.first())
            .and_then(|r| r.start.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(1, 2));

        let tick_max = m
            .price_ranges
            .as_ref()
            .and_then(|pr| pr.last())
            .and_then(|r| r.end.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(99, 2));

        Self {
            event_ticker: m.event_ticker.clone(),
            category: m.category.clone(),
            close_time: m
                .close_time
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|d| d.with_timezone(&Utc)),
            latest_expiration_time: m
                .latest_expiration_time
                .as_deref()
                .or(m.expiration_time.as_deref())
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|d| d.with_timezone(&Utc)),
            volume_24h: m
                .volume_24h_fp
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            open_interest: m
                .open_interest_fp
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            score,
            tick_size,
            tick_min,
            tick_max,
        }
    }

    pub fn hours_to_expiry(&self) -> f64 {
        self.latest_expiration_time
            .or(self.close_time)
            .map(|exp| (exp - Utc::now()).num_seconds() as f64 / 3600.0)
            .unwrap_or(f64::MAX)
    }
}

pub struct StateEngine {
    books: HashMap<MarketTicker, OrderBook>,
    open_orders: HashMap<String, LiveOrder>,
    positions: HashMap<MarketTicker, Position>,
    balance: Balance,
    connectivity: ConnectivityState,
    ever_connected: bool,
    session_started_at: Option<DateTime<Utc>>,
    session_start_equity: Option<Decimal>,
    session_realized_pnl: Decimal,
    daily_realized_pnl: Decimal,
    daily_realized_day: NaiveDate,
    daily_start_equity: Decimal,
    disconnected_at: Option<DateTime<Utc>>,
    last_connected_at: Option<DateTime<Utc>>,
    db_pool: PgPool,
    recent_trades: HashMap<MarketTicker, Vec<RecentTrade>>,
    market_meta: HashMap<MarketTicker, MarketMeta>,
    event_groups: HashMap<String, Vec<MarketTicker>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveOrder {
    pub order_id: String,
    pub market_ticker: MarketTicker,
    pub side: Side,
    pub action: Action,
    pub price: Decimal,
    pub remaining_count: Decimal,
    pub fill_count: Decimal,
    pub status: OrderStatus,
    pub client_order_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecentTrade {
    pub price: Decimal,
    pub taker_side: Side,
    pub ts: chrono::DateTime<chrono::Utc>,
}

impl StateEngine {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            books: HashMap::new(),
            open_orders: HashMap::new(),
            positions: HashMap::new(),
            balance: Balance {
                available: Decimal::ZERO,
                portfolio_value: Decimal::ZERO,
            },
            connectivity: ConnectivityState::Disconnected,
            ever_connected: false,
            session_started_at: None,
            session_start_equity: None,
            session_realized_pnl: Decimal::ZERO,
            daily_realized_pnl: Decimal::ZERO,
            daily_realized_day: Utc::now().date_naive(),
            daily_start_equity: Decimal::ZERO,
            disconnected_at: None,
            last_connected_at: None,
            db_pool,
            recent_trades: HashMap::new(),
            market_meta: HashMap::new(),
            event_groups: HashMap::new(),
        }
    }

    pub fn set_balance(&mut self, balance: Balance) {
        self.balance = balance;
    }

    pub fn ensure_book(&mut self, ticker: MarketTicker) {
        self.books.entry(ticker).or_insert_with(OrderBook::new);
    }

    pub fn get_book(&self, ticker: &MarketTicker) -> Option<&OrderBook> {
        self.books.get(ticker)
    }

    pub fn get_position(&self, ticker: &MarketTicker) -> Option<&Position> {
        self.positions.get(ticker)
    }

    pub fn balance(&self) -> &Balance {
        &self.balance
    }

    pub fn connectivity(&self) -> ConnectivityState {
        self.connectivity
    }

    pub fn ever_connected(&self) -> bool {
        self.ever_connected
    }

    pub fn open_orders(&self) -> &HashMap<String, LiveOrder> {
        &self.open_orders
    }

    pub fn open_order_count(&self) -> usize {
        self.open_orders.len()
    }

    pub fn session_started_at(&self) -> Option<DateTime<Utc>> {
        self.session_started_at
    }

    pub fn session_start_equity(&self) -> Option<Decimal> {
        self.session_start_equity
    }

    pub fn session_realized_pnl(&self) -> Decimal {
        self.session_realized_pnl
    }

    pub fn daily_realized_pnl(&self) -> Decimal {
        if self.daily_realized_day == Utc::now().date_naive() {
            self.daily_realized_pnl
        } else {
            Decimal::ZERO
        }
    }

    pub fn current_equity(&self) -> Decimal {
        self.balance.available + self.balance.portfolio_value
    }

    pub fn session_total_pnl(&self) -> Decimal {
        let Some(base) = self.session_start_equity else {
            return Decimal::ZERO;
        };
        self.current_equity() - base
    }

    pub fn session_unrealized_pnl(&self) -> Decimal {
        self.session_total_pnl() - self.session_realized_pnl
    }

    pub fn daily_total_pnl(&self) -> Decimal {
        self.current_equity() - self.daily_start_equity
    }

    pub fn daily_unrealized_pnl(&self) -> Decimal {
        self.daily_total_pnl() - self.daily_realized_pnl()
    }

    pub fn daily_start_equity(&self) -> Decimal {
        self.daily_start_equity
    }

    pub fn disconnected_at(&self) -> Option<DateTime<Utc>> {
        self.disconnected_at
    }

    pub fn disconnected_for_secs(&self, now: DateTime<Utc>) -> Option<i64> {
        self.disconnected_at
            .map(|ts| (now - ts).num_seconds())
            .map(|secs| secs.max(0))
    }

    /// Set daily PnL baseline without starting a session.
    /// Called at boot so the API returns correct daily PnL before trading starts.
    pub fn set_daily_baseline(
        &mut self,
        day: chrono::NaiveDate,
        daily_realized_pnl: Decimal,
        daily_start_equity: Decimal,
    ) {
        self.daily_realized_day = day;
        self.daily_realized_pnl = daily_realized_pnl;
        self.daily_start_equity = daily_start_equity;
    }

    pub fn initialize_pnl_context(
        &mut self,
        now: DateTime<Utc>,
        daily_realized_pnl: Decimal,
        daily_start_equity: Decimal,
    ) {
        self.session_started_at = Some(now);
        self.session_start_equity = Some(self.current_equity());
        self.session_realized_pnl = Decimal::ZERO;
        self.daily_realized_day = now.date_naive();
        self.daily_realized_pnl = daily_realized_pnl;
        self.daily_start_equity = daily_start_equity;
    }

    pub fn roll_daily_context(&mut self, now: DateTime<Utc>) {
        let day = now.date_naive();
        if day != self.daily_realized_day {
            self.daily_realized_day = day;
            self.daily_realized_pnl = Decimal::ZERO;
            self.daily_start_equity = self.current_equity();
        }
    }

    pub fn total_reserved(&self) -> Decimal {
        self.open_orders
            .values()
            .map(|o| o.price * o.remaining_count)
            .sum()
    }

    pub fn orders_for_market(&self, ticker: &MarketTicker) -> Vec<&LiveOrder> {
        self.open_orders
            .values()
            .filter(|o| o.market_ticker == *ticker)
            .collect()
    }

    pub fn positions(&self) -> &HashMap<MarketTicker, Position> {
        &self.positions
    }

    pub fn books(&self) -> &HashMap<MarketTicker, OrderBook> {
        &self.books
    }

    /// Number of markets that are both subscribed (have a book) AND have loaded metadata.
    /// This is the true count of markets being quoted in the current tick.
    pub fn active_market_count(&self) -> usize {
        self.books
            .keys()
            .filter(|t| self.market_meta.contains_key(t))
            .count()
    }

    /// Clear books, metadata, orders, and event groups at the start of a new trading session.
    /// Leaves positions and balance intact for reconcile_startup to reload.
    pub fn clear_books_and_meta(&mut self) {
        self.books.clear();
        self.market_meta.clear();
        self.open_orders.clear();
        self.event_groups.clear();
        self.recent_trades.clear();
    }

    /// Mid-price portfolio estimate for quoting/strategy use only.
    /// NOT used for equity or PnL — those use `balance.portfolio_value` from Kalshi's API.
    pub fn compute_portfolio_value(&self) -> rust_decimal::Decimal {
        use rust_decimal::Decimal;
        self.positions
            .iter()
            .map(|(ticker, pos)| {
                let mid = self
                    .books
                    .get(ticker)
                    .and_then(|b| b.mid())
                    .unwrap_or(Decimal::ZERO);
                pos.yes_contracts * mid + pos.no_contracts * (Decimal::ONE - mid)
            })
            .sum()
    }

    /// Compute realized PnL by summing all positions' realized_pnl.
    pub fn compute_realized_pnl(&self) -> rust_decimal::Decimal {
        self.positions.values().map(|p| p.realized_pnl).sum()
    }

    pub fn db_pool(&self) -> &PgPool {
        &self.db_pool
    }

    pub fn get_market_meta(&self, ticker: &MarketTicker) -> Option<&MarketMeta> {
        self.market_meta.get(ticker)
    }

    pub fn set_market_meta(&mut self, ticker: MarketTicker, meta: MarketMeta) {
        // Bug 11: dedup before pushing so repeated calls don't bloat the group
        if let Some(ref et) = meta.event_ticker {
            let group = self.event_groups.entry(et.clone()).or_default();
            if !group.contains(&ticker) {
                group.push(ticker.clone());
            }
        }
        self.market_meta.insert(ticker, meta);
    }

    pub fn market_meta_map(&self) -> &HashMap<MarketTicker, MarketMeta> {
        &self.market_meta
    }

    pub fn event_groups(&self) -> &HashMap<String, Vec<MarketTicker>> {
        &self.event_groups
    }

    pub fn sibling_tickers(&self, ticker: &MarketTicker) -> Vec<MarketTicker> {
        let event_ticker = self
            .market_meta
            .get(ticker)
            .and_then(|m| m.event_ticker.as_ref());
        match event_ticker {
            Some(et) => self
                .event_groups
                .get(et)
                .map(|v| v.iter().filter(|t| *t != ticker).cloned().collect())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Remove a market from the active set (for rescan swaps).
    pub fn remove_market(&mut self, ticker: &MarketTicker) {
        // Bug 13: also remove from event_groups to prevent stale sibling lists
        if let Some(meta) = self.market_meta.get(ticker) {
            if let Some(et) = meta.event_ticker.clone() {
                if let Some(group) = self.event_groups.get_mut(&et) {
                    group.retain(|t| t != ticker);
                    if group.is_empty() {
                        self.event_groups.remove(&et);
                    }
                }
            }
        }
        self.books.remove(ticker);
        self.market_meta.remove(ticker);
    }

    pub fn recent_trade_sign(&self, ticker: &MarketTicker) -> Decimal {
        let trades = match self.recent_trades.get(ticker) {
            Some(t) => t,
            None => return Decimal::ZERO,
        };

        if trades.is_empty() {
            return Decimal::ZERO;
        }

        let sum: Decimal = trades
            .iter()
            .map(|t| match t.taker_side {
                Side::Yes => Decimal::ONE,
                Side::No => -Decimal::ONE,
            })
            .sum();

        sum / Decimal::from(trades.len() as i64)
    }

    pub fn upsert_order(&mut self, resp: OrderResponse) {
        let status = resp.to_internal_status();
        match status {
            OrderStatus::Resting => {
                self.open_orders.insert(
                    resp.order_id.clone(),
                    LiveOrder {
                        order_id: resp.order_id.clone(),
                        market_ticker: MarketTicker::from(resp.ticker.as_str()),
                        side: resp.to_internal_side(),
                        action: resp.to_internal_action(),
                        price: resp.price_dollars().unwrap_or_default(),
                        remaining_count: resp.remaining_qty(),
                        fill_count: resp.fill_qty(),
                        status,
                        client_order_id: resp.client_order_id.clone(),
                    },
                );
            }
            _ => {
                self.open_orders.remove(&resp.order_id);
            }
        }
    }

    /// Remove an order from the in-memory state by order ID (e.g. when we detect
    /// on a periodic sync that the exchange no longer has it resting).
    pub fn remove_order(&mut self, order_id: &str) {
        self.open_orders.remove(order_id);
    }

    pub fn upsert_position(&mut self, pos: Position) {
        self.positions.insert(pos.market_ticker.clone(), pos);
    }

    pub fn clear_all(&mut self) {
        self.books.clear();
        self.open_orders.clear();
        self.positions.clear();
        self.balance = Balance {
            available: Decimal::ZERO,
            portfolio_value: Decimal::ZERO,
        };
        self.connectivity = ConnectivityState::Disconnected;
        self.ever_connected = false;
        self.session_started_at = None;
        self.session_start_equity = None;
        self.session_realized_pnl = Decimal::ZERO;
        self.daily_realized_pnl = Decimal::ZERO;
        self.daily_realized_day = Utc::now().date_naive();
        self.daily_start_equity = Decimal::ZERO;
        self.disconnected_at = None;
        self.last_connected_at = None;
        self.recent_trades.clear();
        self.market_meta.clear();
        self.event_groups.clear();
    }

    pub async fn process_event(&mut self, event: ExchangeEvent) {
        match event {
            ExchangeEvent::BookSnapshot {
                market_ticker,
                yes_bids,
                no_bids,
                seq,
            } => {
                if let Some(book) = self.books.get_mut(&market_ticker) {
                    book.apply_snapshot(yes_bids, no_bids, seq);
                    debug!(market = %market_ticker, seq = seq, "Book snapshot applied");
                } else {
                    warn!(
                        market = %market_ticker,
                        "Ignoring snapshot for unknown market (outside active set)"
                    );
                }
            }
            ExchangeEvent::BookDelta {
                market_ticker,
                side,
                price,
                delta,
                seq,
            } => {
                if let Some(book) = self.books.get_mut(&market_ticker) {
                    book.apply_delta(side, price, delta, seq);
                } else {
                    warn!(
                        market = %market_ticker,
                        "Ignoring delta for unknown market (outside active set)"
                    );
                }
            }
            ExchangeEvent::Trade {
                market_ticker,
                price,
                taker_side,
                ts,
                ..
            } => {
                let trades = self.recent_trades.entry(market_ticker).or_default();
                trades.push(RecentTrade {
                    price,
                    taker_side,
                    ts,
                });
                if trades.len() > 50 {
                    trades.drain(0..trades.len() - 50);
                }
            }
            ExchangeEvent::Fill {
                trade_id,
                order_id,
                market_ticker,
                side,
                action,
                price,
                count,
                fee,
                is_taker,
                ts,
            } => {
                info!(
                    fill_id = %trade_id,
                    order_id = %order_id,
                    market = %market_ticker,
                    side = %side,
                    price = %price,
                    count = %count,
                    fee = %fee,
                    "Fill received"
                );

                self.roll_daily_context(ts);

                let pos = self
                    .positions
                    .entry(market_ticker.clone())
                    .or_insert(Position {
                        market_ticker: market_ticker.clone(),
                        yes_contracts: Decimal::ZERO,
                        no_contracts: Decimal::ZERO,
                        avg_yes_price: None,
                        avg_no_price: None,
                        realized_pnl: Decimal::ZERO,
                        unrealized_pnl: Decimal::ZERO,
                    });

                // Compute realized PnL, handling position flips (long→short, short→long).
                //
                // Rules:
                //  - Buying while long (or flat): update avg cost, no realized PnL (only -fee).
                //  - Buying while short: realize (short_entry - buy_price) * covering_qty - fee.
                //    If the buy exceeds the short, the excess opens a new long.
                //  - Selling while long: realize (sell_price - long_entry) * closing_qty - fee.
                //    If the sell exceeds the long, the excess opens a new short.
                //  - Selling while short (or flat): update short avg, no realized PnL (only -fee).
                let realized = match (side, action) {
                    (Side::Yes, Action::Buy) => {
                        let prev_qty = pos.yes_contracts;
                        let pnl = if prev_qty < Decimal::ZERO {
                            // Covering a short YES position.
                            let covering = prev_qty.abs().min(count);
                            let short_avg = pos.avg_yes_price.unwrap_or(price);
                            (short_avg - price) * covering - fee
                        } else {
                            -fee // opening or adding to a long — no realized PnL
                        };

                        pos.yes_contracts += count;
                        let new_qty = pos.yes_contracts;

                        if new_qty > Decimal::ZERO {
                            if prev_qty >= Decimal::ZERO {
                                // Stayed long: update weighted avg cost basis.
                                let avg = pos.avg_yes_price.unwrap_or(price);
                                pos.avg_yes_price = Some(
                                    (avg * prev_qty + price * count) / new_qty,
                                );
                            } else {
                                // Flipped from short to long: avg is the new buy price.
                                pos.avg_yes_price = Some(price);
                            }
                        } else if new_qty == Decimal::ZERO {
                            pos.avg_yes_price = None;
                        }
                        // If still short (new_qty < 0), avg stays as the original short entry.

                        pnl
                    }
                    (Side::Yes, Action::Sell) => {
                        let prev_qty = pos.yes_contracts;
                        let pnl = if prev_qty > Decimal::ZERO {
                            // Closing / reducing a long YES position.
                            let closing = prev_qty.min(count);
                            let long_avg = pos.avg_yes_price.unwrap_or(price);
                            (price - long_avg) * closing - fee
                        } else {
                            -fee // opening or adding to a short — no realized PnL
                        };

                        pos.yes_contracts -= count;
                        let new_qty = pos.yes_contracts;

                        if new_qty < Decimal::ZERO {
                            if prev_qty <= Decimal::ZERO {
                                // Stayed short: update weighted avg short entry.
                                let abs_prev = prev_qty.abs();
                                let avg = pos.avg_yes_price.unwrap_or(price);
                                pos.avg_yes_price = Some(
                                    (avg * abs_prev + price * count) / new_qty.abs(),
                                );
                            } else {
                                // Flipped from long to short: avg is the new sell price.
                                pos.avg_yes_price = Some(price);
                            }
                        } else if new_qty == Decimal::ZERO {
                            pos.avg_yes_price = None;
                        }
                        // If still long (new_qty > 0), avg stays as the original long entry.

                        pnl
                    }
                    (Side::No, Action::Buy) => {
                        let prev_qty = pos.no_contracts;
                        let pnl = if prev_qty < Decimal::ZERO {
                            let covering = prev_qty.abs().min(count);
                            let short_avg = pos.avg_no_price.unwrap_or(price);
                            (short_avg - price) * covering - fee
                        } else {
                            -fee
                        };

                        pos.no_contracts += count;
                        let new_qty = pos.no_contracts;

                        if new_qty > Decimal::ZERO {
                            if prev_qty >= Decimal::ZERO {
                                let avg = pos.avg_no_price.unwrap_or(price);
                                pos.avg_no_price = Some(
                                    (avg * prev_qty + price * count) / new_qty,
                                );
                            } else {
                                pos.avg_no_price = Some(price);
                            }
                        } else if new_qty == Decimal::ZERO {
                            pos.avg_no_price = None;
                        }

                        pnl
                    }
                    (Side::No, Action::Sell) => {
                        let prev_qty = pos.no_contracts;
                        let pnl = if prev_qty > Decimal::ZERO {
                            let closing = prev_qty.min(count);
                            let long_avg = pos.avg_no_price.unwrap_or(price);
                            (price - long_avg) * closing - fee
                        } else {
                            -fee
                        };

                        pos.no_contracts -= count;
                        let new_qty = pos.no_contracts;

                        if new_qty < Decimal::ZERO {
                            if prev_qty <= Decimal::ZERO {
                                let abs_prev = prev_qty.abs();
                                let avg = pos.avg_no_price.unwrap_or(price);
                                pos.avg_no_price = Some(
                                    (avg * abs_prev + price * count) / new_qty.abs(),
                                );
                            } else {
                                pos.avg_no_price = Some(price);
                            }
                        } else if new_qty == Decimal::ZERO {
                            pos.avg_no_price = None;
                        }

                        pnl
                    }
                };

                self.session_realized_pnl += realized;
                self.daily_realized_pnl += realized;
                pos.realized_pnl += realized;

                let _ = crate::db::insert_fill(
                    &self.db_pool,
                    &trade_id,
                    &order_id,
                    &market_ticker.0,
                    &side.to_string(),
                    &action.to_string(),
                    price,
                    count,
                    fee,
                    is_taker,
                )
                .await;

                let _ = crate::db::upsert_position(
                    &self.db_pool,
                    &market_ticker.0,
                    pos.yes_contracts,
                    pos.no_contracts,
                    pos.realized_pnl,
                )
                .await;
            }
            ExchangeEvent::OrderUpdate {
                order_id,
                market_ticker,
                status,
                side,
                action,
                price,
                remaining_count,
                fill_count,
            } => {
                debug!(
                    order_id = %order_id,
                    market = %market_ticker,
                    status = ?status,
                    "Order update"
                );

                match status {
                    OrderStatus::Resting => {
                        self.open_orders.insert(
                            order_id.clone(),
                            LiveOrder {
                                order_id: order_id.clone(),
                                market_ticker: market_ticker.clone(),
                                side,
                                action,
                                price,
                                remaining_count,
                                fill_count,
                                status,
                                client_order_id: None,
                            },
                        );
                    }
                    _ => {
                        self.open_orders.remove(&order_id);
                    }
                }

                let _ = crate::db::insert_order(
                    &self.db_pool,
                    &order_id,
                    &market_ticker.0,
                    &side.to_string(),
                    &action.to_string(),
                    price,
                    remaining_count,
                    &format!("{:?}", status).to_lowercase(),
                    None,
                )
                .await;
            }
            ExchangeEvent::Connected => {
                info!("Exchange connected");
                self.connectivity = ConnectivityState::Connected;
                self.ever_connected = true;
                self.disconnected_at = None;
                self.last_connected_at = Some(Utc::now());
            }
            ExchangeEvent::Disconnected => {
                warn!("Exchange disconnected");
                self.connectivity = ConnectivityState::Disconnected;
                if self.disconnected_at.is_none() {
                    self.disconnected_at = Some(Utc::now());
                }
            }
            ExchangeEvent::BookResyncNeeded { market_ticker } => {
                // Handled by the trading loop (REST snapshot fetch + re-apply).
                // Log here so we have a record of when resyncs were triggered.
                warn!(market = %market_ticker, "Book resync requested due to sequence gap");
            }
        }
    }
}
