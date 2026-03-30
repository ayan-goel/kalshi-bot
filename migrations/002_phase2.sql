-- Phase 2: bot_config, pnl_snapshots, bot_state_history

CREATE TABLE IF NOT EXISTS bot_config (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS pnl_snapshots (
    id BIGSERIAL PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    realized_pnl NUMERIC NOT NULL,
    unrealized_pnl NUMERIC NOT NULL,
    balance NUMERIC NOT NULL,
    portfolio_value NUMERIC NOT NULL,
    open_order_count INT NOT NULL,
    active_market_count INT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pnl_snapshots_ts ON pnl_snapshots(ts DESC);

CREATE TABLE IF NOT EXISTS bot_state_history (
    id BIGSERIAL PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    trigger TEXT NOT NULL,
    details JSONB
);
CREATE INDEX IF NOT EXISTS idx_bot_state_history_ts ON bot_state_history(ts DESC);
