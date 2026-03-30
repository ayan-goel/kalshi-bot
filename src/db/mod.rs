use anyhow::{Context, Result};
use rust_decimal::Decimal;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::AppConfig;

pub async fn init_pool(config: &AppConfig) -> Result<PgPool> {
    let url = config.database_url()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&url)
        .await
        .context("Failed to connect to Postgres")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("Failed to run migrations")?;

    Ok(pool)
}

// ── Order persistence ──

pub async fn insert_order(
    pool: &PgPool,
    order_id: &str,
    market_ticker: &str,
    side: &str,
    action: &str,
    price: Decimal,
    quantity: Decimal,
    status: &str,
    client_order_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO orders (order_id, market_ticker, side, action, price, quantity, status, client_order_id, created_ts, updated_ts)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
        ON CONFLICT (order_id) DO UPDATE SET
            status = EXCLUDED.status,
            quantity = EXCLUDED.quantity,
            updated_ts = NOW()
        "#,
    )
    .bind(order_id)
    .bind(market_ticker)
    .bind(side)
    .bind(action)
    .bind(price)
    .bind(quantity)
    .bind(status)
    .bind(client_order_id)
    .execute(pool)
    .await
    .context("Failed to insert/update order")?;
    Ok(())
}

pub async fn update_order_status(pool: &PgPool, order_id: &str, status: &str) -> Result<()> {
    sqlx::query("UPDATE orders SET status = $1, updated_ts = NOW() WHERE order_id = $2")
        .bind(status)
        .bind(order_id)
        .execute(pool)
        .await
        .context("Failed to update order status")?;
    Ok(())
}

// ── Fill persistence ──

pub async fn insert_fill(
    pool: &PgPool,
    fill_id: &str,
    order_id: &str,
    market_ticker: &str,
    side: &str,
    action: &str,
    price: Decimal,
    quantity: Decimal,
    fee: Decimal,
    is_taker: bool,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fills (fill_id, order_id, market_ticker, side, action, price, quantity, fee, is_taker, fill_ts)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
        ON CONFLICT (fill_id) DO NOTHING
        "#,
    )
    .bind(fill_id)
    .bind(order_id)
    .bind(market_ticker)
    .bind(side)
    .bind(action)
    .bind(price)
    .bind(quantity)
    .bind(fee)
    .bind(is_taker)
    .execute(pool)
    .await
    .context("Failed to insert fill")?;
    Ok(())
}

// ── Position persistence ──

pub async fn upsert_position(
    pool: &PgPool,
    market_ticker: &str,
    yes_contracts: Decimal,
    no_contracts: Decimal,
    realized_pnl: Decimal,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO positions (market_ticker, yes_contracts, no_contracts, realized_pnl, updated_ts)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (market_ticker) DO UPDATE SET
            yes_contracts = EXCLUDED.yes_contracts,
            no_contracts = EXCLUDED.no_contracts,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_ts = NOW()
        "#,
    )
    .bind(market_ticker)
    .bind(yes_contracts)
    .bind(no_contracts)
    .bind(realized_pnl)
    .execute(pool)
    .await
    .context("Failed to upsert position")?;
    Ok(())
}

// ── Strategy decision logging ──

pub async fn insert_strategy_decision(
    pool: &PgPool,
    market_ticker: &str,
    fair_value: Decimal,
    inventory: Decimal,
    target_quotes: &serde_json::Value,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO strategy_decisions (market_ticker, ts, fair_value, inventory, target_quotes, reason)
        VALUES ($1, NOW(), $2, $3, $4, $5)
        "#,
    )
    .bind(market_ticker)
    .bind(fair_value)
    .bind(inventory)
    .bind(target_quotes)
    .bind(reason)
    .execute(pool)
    .await
    .context("Failed to insert strategy decision")?;
    Ok(())
}

// ── Risk event logging ──

pub async fn insert_risk_event(
    pool: &PgPool,
    severity: &str,
    component: &str,
    market_ticker: Option<&str>,
    message: &str,
    payload: Option<&serde_json::Value>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO risk_events (ts, severity, component, market_ticker, message, payload)
        VALUES (NOW(), $1, $2, $3, $4, $5)
        "#,
    )
    .bind(severity)
    .bind(component)
    .bind(market_ticker)
    .bind(message)
    .bind(payload)
    .execute(pool)
    .await
    .context("Failed to insert risk event")?;
    Ok(())
}

// ── Bot config persistence ──

pub async fn get_config(pool: &PgPool, key: &str) -> Result<Option<serde_json::Value>> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT value FROM bot_config WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .context("Failed to read bot_config")?;
    Ok(row.map(|r| r.0))
}

pub async fn set_config(pool: &PgPool, key: &str, value: &serde_json::Value) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO bot_config (key, value, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .context("Failed to set bot_config")?;
    Ok(())
}

pub async fn get_all_config(pool: &PgPool) -> Result<Vec<(String, serde_json::Value)>> {
    let rows: Vec<(String, serde_json::Value)> =
        sqlx::query_as("SELECT key, value FROM bot_config ORDER BY key")
            .fetch_all(pool)
            .await
            .context("Failed to read bot_config")?;
    Ok(rows)
}

// ── PnL snapshots ──

pub async fn insert_pnl_snapshot(
    pool: &PgPool,
    realized_pnl: Decimal,
    unrealized_pnl: Decimal,
    balance: Decimal,
    portfolio_value: Decimal,
    open_order_count: i32,
    active_market_count: i32,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO pnl_snapshots (ts, realized_pnl, unrealized_pnl, balance, portfolio_value, open_order_count, active_market_count)
        VALUES (NOW(), $1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(realized_pnl)
    .bind(unrealized_pnl)
    .bind(balance)
    .bind(portfolio_value)
    .bind(open_order_count)
    .bind(active_market_count)
    .execute(pool)
    .await
    .context("Failed to insert pnl_snapshot")?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PnlSnapshotRow {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub balance: Decimal,
    pub portfolio_value: Decimal,
    pub open_order_count: i32,
    pub active_market_count: i32,
}

pub async fn get_pnl_snapshots(pool: &PgPool, limit: i64) -> Result<Vec<PnlSnapshotRow>> {
    let rows: Vec<(
        chrono::DateTime<chrono::Utc>,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        i32,
        i32,
    )> = sqlx::query_as(
        "SELECT ts, realized_pnl, unrealized_pnl, balance, portfolio_value, open_order_count, active_market_count FROM pnl_snapshots ORDER BY ts DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to read pnl_snapshots")?;

    Ok(rows
        .into_iter()
        .map(
            |(ts, realized_pnl, unrealized_pnl, balance, portfolio_value, open_order_count, active_market_count)| {
                PnlSnapshotRow {
                    ts,
                    realized_pnl,
                    unrealized_pnl,
                    balance,
                    portfolio_value,
                    open_order_count,
                    active_market_count,
                }
            },
        )
        .collect())
}

// ── Recent fills from DB ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct FillRow {
    pub fill_id: String,
    pub order_id: String,
    pub market_ticker: String,
    pub side: String,
    pub action: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub fee: Decimal,
    pub is_taker: bool,
    pub fill_ts: chrono::DateTime<chrono::Utc>,
}

pub async fn get_recent_fills(pool: &PgPool, limit: i64) -> Result<Vec<FillRow>> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        Decimal,
        Decimal,
        Decimal,
        bool,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT fill_id, order_id, market_ticker, side, action, price, quantity, fee, is_taker, fill_ts FROM fills ORDER BY fill_ts DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to read recent fills")?;

    Ok(rows
        .into_iter()
        .map(
            |(fill_id, order_id, market_ticker, side, action, price, quantity, fee, is_taker, fill_ts)| {
                FillRow {
                    fill_id,
                    order_id,
                    market_ticker,
                    side,
                    action,
                    price,
                    quantity,
                    fee,
                    is_taker,
                    fill_ts,
                }
            },
        )
        .collect())
}

// ── Recent orders from DB ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderRow {
    pub order_id: String,
    pub market_ticker: String,
    pub side: String,
    pub action: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub status: String,
    pub created_ts: chrono::DateTime<chrono::Utc>,
}

pub async fn get_recent_orders(pool: &PgPool, status: Option<&str>, limit: i64) -> Result<Vec<OrderRow>> {
    let rows: Vec<(String, String, String, String, Decimal, Decimal, String, chrono::DateTime<chrono::Utc>)> = if let Some(st) = status {
        sqlx::query_as(
            "SELECT order_id, market_ticker, side, action, price, quantity, status, created_ts FROM orders WHERE status = $1 ORDER BY updated_ts DESC LIMIT $2",
        )
        .bind(st)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT order_id, market_ticker, side, action, price, quantity, status, created_ts FROM orders ORDER BY updated_ts DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(order_id, market_ticker, side, action, price, quantity, status, created_ts)| {
                OrderRow {
                    order_id,
                    market_ticker,
                    side,
                    action,
                    price,
                    quantity,
                    status,
                    created_ts,
                }
            },
        )
        .collect())
}

// ── Risk events from DB ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct RiskEventRow {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub severity: String,
    pub component: String,
    pub market_ticker: Option<String>,
    pub message: String,
    pub payload: Option<serde_json::Value>,
}

pub async fn get_risk_events(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<RiskEventRow>> {
    let rows: Vec<(
        chrono::DateTime<chrono::Utc>,
        String,
        String,
        Option<String>,
        String,
        Option<serde_json::Value>,
    )> = sqlx::query_as(
        "SELECT ts, severity, component, market_ticker, message, payload FROM risk_events ORDER BY ts DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to read risk_events")?;

    Ok(rows
        .into_iter()
        .map(|(ts, severity, component, market_ticker, message, payload)| RiskEventRow {
            ts,
            severity,
            component,
            market_ticker,
            message,
            payload,
        })
        .collect())
}

// ── Strategy decisions from DB ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct StrategyDecisionRow {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub market_ticker: String,
    pub fair_value: Decimal,
    pub inventory: Decimal,
    pub reason: String,
}

pub async fn get_strategy_decisions(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<StrategyDecisionRow>> {
    let rows: Vec<(chrono::DateTime<chrono::Utc>, String, Decimal, Decimal, String)> = sqlx::query_as(
        "SELECT ts, market_ticker, fair_value, inventory, reason FROM strategy_decisions ORDER BY ts DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to read strategy_decisions")?;

    Ok(rows
        .into_iter()
        .map(|(ts, market_ticker, fair_value, inventory, reason)| StrategyDecisionRow {
            ts,
            market_ticker,
            fair_value,
            inventory,
            reason,
        })
        .collect())
}
