CREATE TABLE IF NOT EXISTS markets (
    market_ticker TEXT PRIMARY KEY,
    event_ticker TEXT,
    series_ticker TEXT,
    title TEXT,
    category TEXT,
    expiration_ts TIMESTAMPTZ,
    status TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS book_snapshots (
    id BIGSERIAL PRIMARY KEY,
    market_ticker TEXT NOT NULL,
    ts TIMESTAMPTZ NOT NULL,
    yes_bids JSONB NOT NULL,
    no_bids JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_book_snapshots_market_ts ON book_snapshots(market_ticker, ts DESC);

CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    market_ticker TEXT NOT NULL,
    side TEXT NOT NULL,
    action TEXT NOT NULL,
    price NUMERIC NOT NULL,
    quantity NUMERIC NOT NULL,
    status TEXT NOT NULL,
    client_order_id TEXT,
    created_ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    raw JSONB
);
CREATE INDEX IF NOT EXISTS idx_orders_market ON orders(market_ticker);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);

CREATE TABLE IF NOT EXISTS fills (
    fill_id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    market_ticker TEXT NOT NULL,
    side TEXT NOT NULL,
    action TEXT NOT NULL,
    price NUMERIC NOT NULL,
    quantity NUMERIC NOT NULL,
    fee NUMERIC DEFAULT 0,
    is_taker BOOLEAN DEFAULT FALSE,
    fill_ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    raw JSONB
);
CREATE INDEX IF NOT EXISTS idx_fills_market_ts ON fills(market_ticker, fill_ts DESC);
CREATE INDEX IF NOT EXISTS idx_fills_order ON fills(order_id);

CREATE TABLE IF NOT EXISTS positions (
    market_ticker TEXT PRIMARY KEY,
    yes_contracts NUMERIC NOT NULL DEFAULT 0,
    no_contracts NUMERIC NOT NULL DEFAULT 0,
    avg_yes_price NUMERIC,
    avg_no_price NUMERIC,
    realized_pnl NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl NUMERIC NOT NULL DEFAULT 0,
    updated_ts TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS strategy_decisions (
    id BIGSERIAL PRIMARY KEY,
    market_ticker TEXT NOT NULL,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    fair_value NUMERIC NOT NULL,
    inventory NUMERIC NOT NULL,
    target_quotes JSONB NOT NULL,
    features JSONB,
    reason TEXT
);
CREATE INDEX IF NOT EXISTS idx_strategy_decisions_market_ts ON strategy_decisions(market_ticker, ts DESC);

CREATE TABLE IF NOT EXISTS risk_events (
    id BIGSERIAL PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    severity TEXT NOT NULL,
    component TEXT NOT NULL,
    market_ticker TEXT,
    message TEXT NOT NULL,
    payload JSONB
);
CREATE INDEX IF NOT EXISTS idx_risk_events_ts ON risk_events(ts DESC);
CREATE INDEX IF NOT EXISTS idx_risk_events_severity ON risk_events(severity);
