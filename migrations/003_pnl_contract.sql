-- Phase 3: enrich pnl_snapshots with canonical session/daily metrics

ALTER TABLE pnl_snapshots
    ADD COLUMN IF NOT EXISTS equity NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS session_pnl NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS daily_realized_pnl NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS daily_unrealized_pnl NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS daily_pnl NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS session_started_at TIMESTAMPTZ NULL;

-- Backfill historical rows so pre-migration data doesn't plot as zero spikes.
UPDATE pnl_snapshots
SET equity = balance + portfolio_value
WHERE equity = 0;

UPDATE pnl_snapshots
SET session_pnl = realized_pnl + unrealized_pnl
WHERE session_pnl = 0 AND session_started_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_pnl_snapshots_session_started_at
    ON pnl_snapshots(session_started_at DESC);
