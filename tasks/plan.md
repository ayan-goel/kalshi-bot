# Kalshi Bot — Stabilization Plan

Source: SPEC.md  
Goal: fix all 13 bugs so the bot runs a 4-hour demo session without spurious kill-switch
or state corruption. Profitability comes after correctness.

---

## Dependency Graph

```
BUG-4 (balance sync)
  └─► BUG-5 (kill switch)
  └─► BUG-3 (PnL accounting)

BUG-1 (market count) ←─ BUG-2 (metadata fetch)

BUG-7 (stale state) ─► BUG-10 (active_market_count)

BUG-6 (blacklist)   — independent
BUG-8 (config reload) — independent
BUG-9 (startup retry) — independent
BUG-11 (double inv skew) ─ independent (strategy-layer only)
BUG-12 (price units)    ─ independent audit
BUG-13 (log missing meta) ─ independent 1-liner
```

All Phase 1 tasks must be complete before Phase 2. Phases 3–5 can be done in any order.

---

## Phase 1 — Stop the Bleeding (correctness + safety)

### Task 1: Eliminate redundant balance sync; stabilize kill switch

**Bugs fixed:** BUG-4, BUG-5  
**Files:** `src/state/mod.rs`, `src/main.rs`, `src/risk/mod.rs`

#### Changes

**`src/state/mod.rs` — `process_event` fill branch (line ~562–568)**

Remove the three in-memory balance mutations from the fill handler:
```rust
// DELETE these three lines:
self.session_realized_pnl += cash_delta;
self.daily_realized_pnl += cash_delta;
self.balance.available += cash_delta;
```
The periodic REST sync is the single source of truth for `balance`. Fill events only
need to update `positions` and the `db::insert_fill` call.

Also remove `cash_delta` computation entirely — it's only used by those three lines.

**`src/main.rs` — trading-loop `order_sync_tick` branch (~line 633)**

Remove the duplicate `get_balance()` and `get_positions()` calls from
`order_sync_tick`. Keep only the resting-orders reconciliation (the pruning of stale
orders). Balance and positions are already synced by the data-sync task every 120s.

**`src/risk/mod.rs` — `kill_switch_check` (~line 56)**

Add a guard before the daily-loss check:
```rust
// Skip daily-loss check until the baseline has been established
if self.daily_start_equity == Decimal::ZERO {
    // baseline not set yet — skip
} else if state.daily_total_pnl() < -self.max_loss_daily {
    ...
}
```
The `RiskEngine` needs access to `daily_start_equity`. Either pass it as a parameter
to `kill_switch_check(&self, state: &StateEngine)` (state already exposes it via
`daily_start_equity()` if you add that accessor), or add the guard inside `StateEngine`.

#### Verification

1. Start the bot in demo. Watch dashboard PnL for 5 minutes. Balance and PnL values
   must not jump at the 2-minute mark.
2. Trigger a fill in demo (place a resting order near the market price). After the
   fill, `session_realized_pnl` in the dashboard must NOT change — it stays at 0
   until fills are round-tripped (fixed in Task 3).
3. `daily_start_equity` is 0 on first startup → kill switch does not fire.

---

### Task 2: Fix market count enforcement and metadata fetch

**Bugs fixed:** BUG-1, BUG-2  
**Files:** `src/main.rs` (rescan branch ~line 682–733)

#### Changes

**Prune dropped markets after rescan**

After `scanner.select_markets(…)` returns `new_tickers`, compute the set of markets
that should be removed:

```rust
// Collect tickers currently active but NOT in the new desired set
let new_set: HashSet<String> = new_tickers.iter().cloned().collect();
let to_remove: Vec<MarketTicker> = {
    let engine = state_engine.read().await;
    engine.market_meta_map()
        .keys()
        .filter(|t| !new_set.contains(&t.0))
        .cloned()
        .collect()
};

if !to_remove.is_empty() {
    let mut engine = state_engine.write().await;
    for ticker in &to_remove {
        engine.remove_market(ticker);
    }
    // Build list of remaining tickers for WS
    let remaining: Vec<String> = engine.books().keys().map(|k| k.0.clone()).collect();
    let _ = ws_cmd_tx.send(WsCommand::SubscribeMarkets(remaining)).await;
    tracing::info!(removed = to_remove.len(), "Pruned markets dropped from top-N");
}
```

**Fix metadata fetch for new markets**

Replace the broken `get_markets(None, Some(1))` call with a lookup from a pre-fetched
map (same pattern as `reconcile_startup`):

```rust
// Before the "add new markets" loop, fetch all market data once:
let all_market_data = rest_client.get_all_markets(Some("open"), None, None)
    .await.unwrap_or_default();
let market_data_map: HashMap<&str, &MarketResponse> = all_market_data
    .iter().map(|m| (m.ticker.as_str(), m)).collect();

// Then inside the new-market loop, replace the broken call:
if let Some(m) = market_data_map.get(ticker.as_str()) {
    let meta = MarketMeta::from_market_response(m, sm.score);
    engine.set_market_meta(mt, meta);
}
```

#### Verification

1. Set `max_markets_active: 5` in config. Start bot. After startup,
   `engine.active_market_count()` == 5.
2. Wait for one rescan cycle (default 15 min, or lower for testing). Market count
   remains ≤ 5.
3. Set `max_markets_active: 3` in config, restart bot. After rescan, count drops to 3.
4. For any newly-added market, confirm `market_meta` has correct `tick_size`, `close_time`,
   and `category` (visible via the `/api/markets` route or logs).

---

## Checkpoint A

After Tasks 1 and 2, run a 30-minute demo session:
- [ ] Bot stays in `running` state for the full 30 minutes
- [ ] `active_markets` in status endpoint equals configured `max_markets_active`
- [ ] Dashboard PnL line is smooth (no 2-minute jumps)
- [ ] No kill-switch fires

Do not proceed to Phase 2 until Checkpoint A passes.

---

## Phase 2 — State Lifecycle

### Task 3: Fix PnL accounting (realized PnL = round-trip cash)

**Bug fixed:** BUG-3  
**Files:** `src/state/mod.rs`  
**Depends on:** Task 1 (balance no longer mutated by fills)

#### Context

After Task 1, the fill handler no longer touches `balance` or `session_realized_pnl`.
Now fix what `session_realized_pnl` should mean: cumulative realized cash from
round-trip trades (sell proceeds − buy cost − fees for completed round-trips).

The cleanest correct approach: treat `session_total_pnl()` (equity delta) as the
ground truth, and derive realized vs unrealized by comparing equity against open
position marks. This is already mostly correct once the balance sync noise is gone
(Task 1 fixes that).

However, the `session_realized_pnl` counter is still used in the dashboard to split
"realized" vs "unrealized". The fix is to compute it properly from fills:

```rust
// On fill, compute realized portion only when reducing a position:
let pos = &mut self.positions.entry(market_ticker.clone()).or_default();

let realized = match (side, action) {
    // Selling YES contracts → realize gain/loss vs avg_yes_price
    (Side::Yes, Action::Sell) if pos.yes_contracts > Decimal::ZERO => {
        let avg = pos.avg_yes_price.unwrap_or(price);
        (price - avg) * count - fee
    }
    // Buying to cover a short NO position
    (Side::No, Action::Buy) if pos.no_contracts > Decimal::ZERO => {
        let avg = pos.avg_no_price.unwrap_or(price);
        (avg - price) * count - fee
    }
    // Opening a new position → no realized PnL, just pay the fee
    _ => -fee,
};

self.session_realized_pnl += realized;
self.daily_realized_pnl += realized;
```

Also update the avg price on buys:
```rust
match (side, action) {
    (Side::Yes, Action::Buy) => {
        let prev_qty = pos.yes_contracts;
        let avg = pos.avg_yes_price.unwrap_or(price);
        pos.avg_yes_price = Some((avg * prev_qty + price * count) / (prev_qty + count));
        pos.yes_contracts += count;
    }
    (Side::Yes, Action::Sell) => { pos.yes_contracts -= count; }
    (Side::No, Action::Buy) => {
        let prev_qty = pos.no_contracts;
        let avg = pos.avg_no_price.unwrap_or(price);
        pos.avg_no_price = Some((avg * prev_qty + price * count) / (prev_qty + count));
        pos.no_contracts += count;
    }
    (Side::No, Action::Sell) => { pos.no_contracts -= count; }
}
```

#### Verification

1. In demo: buy 5 YES contracts at 0.40 → `session_realized_pnl` changes only by −fee.
2. Sell those 5 YES contracts at 0.42 → `session_realized_pnl` increases by
   (0.02 × 5 − fees) ≈ $0.10 − fees.
3. `session_total_pnl()` and `session_realized_pnl + session_unrealized_pnl` stay consistent.

---

### Task 4: Clear stale state on restart; fix active_market_count

**Bugs fixed:** BUG-7, BUG-10  
**Files:** `src/main.rs` (run_trading_loop), `src/state/mod.rs`

#### Changes

**Clear state before reconcile_startup**

At the top of `run_trading_loop`, before calling `reconcile_startup`, clear the engine:
```rust
{
    let mut engine = state_engine.write().await;
    engine.clear_books_and_meta();  // new method — clears books, market_meta, event_groups
    // Do NOT clear positions or balance — those are loaded by reconcile_startup
}
```

Add `clear_books_and_meta()` to `StateEngine`:
```rust
pub fn clear_books_and_meta(&mut self) {
    self.books.clear();
    self.market_meta.clear();
    self.event_groups.clear();
    self.recent_trades.clear();
    // Leave positions, open_orders, balance intact for reconcile_startup to reload
}
```

**Fix `active_market_count()`**

Change the method to count the intersection:
```rust
pub fn active_market_count(&self) -> usize {
    self.books.keys()
        .filter(|t| self.market_meta.contains_key(t))
        .count()
}
```

#### Verification

1. Start bot, let it stabilize with 5 markets. Stop bot. Start bot again.
2. After restart, `active_market_count()` == 5 (not 5 + leftover from prior session).
3. `engine.books().len()` == `engine.market_meta_map().len()` == `active_market_count()`.

---

## Checkpoint B

After Tasks 3 and 4:
- [ ] Realized PnL in dashboard matches expected value from manual trade calculation
- [ ] Stop → start cycle shows correct market count, not accumulated count
- [ ] Session PnL at session start is always 0

---

## Phase 3 — Reliability

### Task 5: Fix market blacklisting — reset on success

**Bug fixed:** BUG-6  
**File:** `src/main.rs` (~line 821)

#### Changes

Track successful markets in the reconcile return value (currently `reconcile` only
returns failed markets). Add a new return: `(Vec<MarketTicker>, Vec<MarketTicker>)` —
`(failed, succeeded)`. Or simpler: pass the failure counter map into the execution
engine and reset in-place on success.

Simplest fix within current architecture — add a `clear_failure` path in the trading loop:

```rust
// In ExecutionEngine::reconcile, after a successful create_order:
// Return both failed markets AND markets that had a successful order this tick.
// In main.rs trading loop:
for market in succeeded_markets {
    market_failures.remove(&market);
    skip_markets.remove(&market);
}
```

Actually simpler: clear `market_failures[market]` on any successful order for that
market. If a market gets un-blacklisted, also remove it from `skip_markets`.

#### Verification

1. Force an `invalid_order` error for a market (e.g. by temporarily setting an
   impossible price). After 3 failures, market is in `skip_markets`.
2. Fix the condition. After the next successful order for that market, confirm
   `skip_markets` no longer contains it.

---

### Task 6: Add startup retry for transient exchange errors

**Bug fixed:** BUG-9  
**File:** `src/main.rs` (`reconcile_startup`)

#### Changes

Wrap steps 1-3 in a retry helper:
```rust
async fn retry<T, F, Fut>(op: F, max_attempts: u32, label: &str) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut delay = Duration::from_secs(2);
    for attempt in 1..=max_attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if attempt < max_attempts => {
                tracing::warn!("{label} failed (attempt {attempt}): {e}. Retrying in {delay:?}");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

Apply to steps 1-3. Step 4 (market scan) retains hard-fail behavior.

#### Verification

1. Mock a 503 on the first balance fetch (or use network disruption in demo).
2. Bot retries and reaches `BotState::Running` without manual intervention.

---

### Task 7: Live reload of max_markets from shared config in rescan

**Bug fixed:** BUG-8  
**File:** `src/main.rs` (rescan_tick branch ~line 682)

#### Changes

In the `rescan_tick` branch, replace the captured `max_markets` variable with a live
read from the shared config:
```rust
_ = rescan_tick.tick() => {
    let cfg = config.read().await;
    let max_markets = cfg.trading.max_markets_active;  // live value
    // ... rest of rescan using this max_markets
}
```

Remove the `let max_markets = cfg.trading.max_markets_active;` capture from the
`run_trading_loop` preamble (or keep it only as the initial value, replaced in the
rescan branch).

#### Verification

1. Start bot with `max_markets_active: 10`. Confirm 10 active markets.
2. Change to 5 via the config API endpoint. Wait one rescan cycle.
3. Confirm `active_markets` drops to 5.

---

## Phase 4 — Strategy Correctness

### Task 8: Remove double inventory skew from fair value engine

**Bug fixed:** BUG-11  
**Files:** `src/fair_value/mod.rs`, `src/strategy/mod.rs`

#### Context

Currently inventory adjustment appears twice:
1. In `FairValueEngine.compute()`: `inv_adj = -k1*inv - k3*inv^3` shifts the fair value
2. In `MarketMakerStrategy.generate_quotes()`: `skew = -inventory_skew_coeff * inventory * inv_skew_scale` shifts both bid and ask

The fair value is supposed to be a pure market estimate. Inventory management belongs
only in the quoting layer (strategy).

#### Changes

**`src/fair_value/mod.rs`**

Remove `inventory_penalty_k1` and `inventory_penalty_k3` from `FairValueEngine`.
Remove their usage in `compute()`:
```rust
// DELETE:
let inv_adj = -self.inventory_penalty_k1 * inventory
    - self.inventory_penalty_k3 * inventory * inventory * inventory;

// And remove from raw_fair:
let raw_fair = microprice + imbalance_adj + trade_sign_adj;  // no inv_adj
```

Remove `inventory_penalty_k1` and `inventory_penalty_k3` from `FairValueEngine` struct
and its `new()` constructor.

**`src/strategy/mod.rs`** — keep the existing skew logic as-is. It now has sole
responsibility for inventory management.

**`src/config.rs`** — keep the config fields (`inventory_penalty_k1`,
`inventory_penalty_k3`) for now; mark them as deprecated in comments. Removing them
from the YAML would require a config migration.

#### Verification

1. With zero inventory: `fv.price` == `microprice + imbalance_adj + trade_sign_adj`.
2. With +10 contracts long: `fv.price` is unchanged from the zero-inventory case
   (microprice hasn't changed). Only the bid/ask spread in the strategy is skewed.
3. Run the `fair_value_tests.rs` suite and update tests that expected the old behavior.

---

### Task 9: Audit price unit convention

**Bug fixed:** BUG-12  
**Files:** `src/exchange/rest.rs`, `src/exchange/models.rs`, `tests/`

#### Action

This is an investigation + documentation task, not a guaranteed code change.

1. Read `KalshiRestClient::create_order` completely. Find where `yes_price_dollars` is
   serialized into the HTTP request body.
2. Check the Kalshi API docs or demo responses: does the API accept `0.47` or `47`?
3. Verify by placing one resting order in demo at a known price (e.g. bid at 0.30 for
   a ~50¢ market). Call `get_order` and confirm the resting price field matches.
4. Write the result as a comment above `CreateOrderRequest.yes_price_dollars` so it
   is unambiguous.
5. Add an integration test in `tests/` (gated by `#[ignore]` or a feature flag so it
   only runs against demo).

#### Verification

A placed order at price P rests at P on the exchange orderbook (confirmed via REST
round-trip in demo environment).

---

### Task 10: Log missing metadata at WARN level

**Bug fixed:** BUG-13  
**File:** `src/main.rs` (~line 1023)

#### Changes

One-line change:
```rust
// Change from:
tracing::debug!(market = %ticker, "Skipping market: no metadata loaded");
// To:
tracing::warn!(market = %ticker, "Skipping market with book but no metadata — book data wasted");
```

#### Verification

After startup, if any market has a book but no metadata, the warning is visible in
default log output (`info` level filter).

---

## Final Checkpoint

After all tasks, run a 4-hour demo session:

- [ ] Bot stays in `running` state for 4 hours without kill-switch
- [ ] `active_markets` matches `max_markets_active` throughout
- [ ] Dashboard PnL is monotonically reasonable (no jumps, resets, or sign flips)
- [ ] Session realized PnL matches: sum(sell_price - buy_price - fees) per closed round-trip
- [ ] Stop → start within the same session restores correct market count
- [ ] No market permanently stuck in skip_markets due to a single transient error

---

## Files Changed Summary

| Task | Files |
|------|-------|
| 1 | `src/state/mod.rs`, `src/main.rs`, `src/risk/mod.rs` |
| 2 | `src/main.rs` |
| 3 | `src/state/mod.rs` |
| 4 | `src/main.rs`, `src/state/mod.rs` |
| 5 | `src/main.rs`, `src/execution/mod.rs` |
| 6 | `src/main.rs` |
| 7 | `src/main.rs` |
| 8 | `src/fair_value/mod.rs`, `src/config.rs`, `tests/fair_value_tests.rs` |
| 9 | `src/exchange/rest.rs`, `src/exchange/models.rs`, `tests/` |
| 10 | `src/main.rs` |

---

## What This Plan Does NOT Do

- Add new features
- Change the strategy formula beyond removing the double-skew
- Modify the dashboard UI
- Add Prometheus metrics
- Change the database schema
