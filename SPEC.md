# Kalshi Bot — Stabilization Spec

## Objective

Fix the existing bot so it runs correctly and reliably before attempting to improve
profitability. The system has working infrastructure but contains concrete bugs in
market management, PnL accounting, and the trading loop that make it unsafe to run
live or draw conclusions from. This spec catalogs every identified bug with its
source location and defines acceptance criteria for each fix.

**Do not add new features until everything in this spec is resolved.**

---

## Reality Check

The bot is a single Rust process. Key components:

- `main.rs` — top-level wiring: data-sync task, PnL snapshot loop, bot-control loop, trading loop
- `state/mod.rs` — `StateEngine`: in-memory books, orders, positions, balance, PnL counters
- `strategy/mod.rs` — `MarketMakerStrategy`: multi-level quote generation
- `fair_value/mod.rs` — `FairValueEngine`: microprice + order imbalance + trade sign
- `execution/mod.rs` — `ExecutionEngine`: diffs desired vs live orders, calls REST
- `risk/mod.rs` — `RiskEngine`: per-order approval + kill-switch checks
- `market_scanner/mod.rs` — selects top-N markets by score
- `bot_state.rs` — `BotStateMachine`: state transitions (Stopped/Starting/Running/Error)
- `api/routes.rs` — REST + WebSocket API for the dashboard

---

## Bug Catalog

### P0 — Correctness / Safety

---

#### BUG-1: Active-market count grows unboundedly; max_markets is never enforced

**File:** `src/main.rs:682–733` (rescan branch of `rescan_tick`)

**What happens:** The periodic rescan calls `scanner.select_markets(…, max_markets)` which
returns up to `max_markets` tickers. The code only adds NEW markets (those without an
existing book entry). It never removes markets that dropped out of the top N. Over
multiple rescan cycles the active set accumulates indefinitely — hence 50 active markets
when the config says 30.

**Fix:** After the scanner returns the new desired set, compute the symmetric difference:
remove any market currently tracked in `market_meta` that is NOT in `new_tickers`. Call
`engine.remove_market()` for each dropped ticker. Also unsubscribe those tickers from the
WebSocket.

**Acceptance criteria:**
- After one full rescan cycle, `engine.active_market_count()` equals the configured
  `max_markets_active` (or fewer if fewer markets pass filters).
- Removed markets are unsubscribed from the WS feed.

---

#### BUG-2: Wrong API call when fetching market metadata during rescan

**File:** `src/main.rs:700`

**What happens:**
```rust
match rest_client.get_markets(None, Some(1)).await {
```
This fetches up to 1 market with NO ticker filter — it returns the first market in
Kalshi's pagination, not the target ticker. Newly-added markets during rescan get the
wrong metadata (or none at all if the returned market doesn't happen to be the target).

**Fix:** Fetch the specific market by ticker using the appropriate REST endpoint
(`get_market(&ticker)` or filtering by ticker in `get_markets`). The initial startup
reconcile (`reconcile_startup`) already has the correct pattern using `market_data_map`.
The rescan should build the same map from `rest_client.get_all_markets(…)` rather than
calling `get_markets(None, Some(1))`.

**Acceptance criteria:**
- Every market added during a rescan has correct metadata (tick_size, expiry, category).
- Metadata for market X is loaded from X's response, not an arbitrary market's response.

---

#### BUG-3: PnL accounting is incorrect — cash flows, not PnL

**File:** `src/state/mod.rs:562–567`

**What happens:** The fill handler computes:
```rust
let cash_delta = match action {
    Action::Buy  => -(price * count) - fee,   // negative for every buy
    Action::Sell => (price * count) - fee,    // positive for every sell
};
self.session_realized_pnl += cash_delta;
```
This tracks the cumulative net cash flow, not realized PnL. Buying YES at 0.40 registers
as a loss of 0.40 per contract even though nothing is realized yet. The number is only
meaningful (equals realized PnL) when ALL positions are flat, which is never the case
for an active market maker.

**Fix:** `session_realized_pnl` should track ROUND-TRIP PnL only:
- On a buy, no realized PnL — update the position cost basis.
- On a sell, compute: `realized += (sell_price - avg_buy_price) * count - fees`.
- Symmetrically for NO-side and short positions.

Alternatively, remove session_realized_pnl entirely from the fill handler and derive
it from the balance delta between REST syncs, which is what Kalshi actually reports via
`balance.available` and `balance.portfolio_value`.

**Acceptance criteria:**
- After opening and closing a single position with a 2¢ spread, `session_realized_pnl`
  equals approximately 2¢ × contracts − fees.
- Holding an open position with no closes shows `session_realized_pnl ≈ 0`.

---

#### BUG-4: Balance sync clobbers the in-memory balance modified by fill events

**Files:** `src/state/mod.rs:568`, `src/main.rs:211–215`, `src/main.rs:633–642`

**What happens:** When a fill arrives, the handler immediately adjusts
`self.balance.available += cash_delta`. But the data-sync task and the trading-loop
order_sync_tick both call `engine.set_balance(rest_balance)` every ~120 seconds, fully
overwriting `balance.available`. This causes the dashboard PnL to jump/reset every 2
minutes.

Additionally, there are **two concurrent balance syncs** — one in the data-sync task
(line 211) and one in the trading-loop order_sync_tick (line 633) — both running
every 120 seconds. Both hold a write lock on `state_engine`.

**Fix:**
1. Remove the in-memory `balance.available += cash_delta` from the fill handler. Let
   the periodic REST sync be the single source of truth for the balance. The fill event
   only needs to update position state and PnL tracking (once BUG-3 is fixed).
2. Remove one of the two redundant balance syncs. Pick one location (prefer the
   data-sync task); remove the duplicate from `order_sync_tick`.

**Acceptance criteria:**
- Dashboard balance does not jump after a REST sync.
- There is exactly one code path writing balance to `state_engine`.

---

#### BUG-5: Kill switch fires on spurious PnL values

**File:** `src/risk/mod.rs:56`

**What happens:** `kill_switch_check` evaluates `state.daily_total_pnl()` which is
computed as `current_equity() - daily_start_equity`. `current_equity()` is
`balance.available + balance.portfolio_value`. Both of those values fluctuate constantly
due to BUG-4. A temporary balance overwrite can show a large apparent loss, triggering
the kill switch and terminating the trading loop.

This is the primary reason the bot terminates unexpectedly mid-session.

**Fix:** Resolve BUG-4 first. Then ensure `daily_start_equity` is set correctly at the
start of each day from the DB snapshot. Add a guard: if `daily_start_equity == 0`, skip
the daily-loss kill-switch check (the baseline hasn't been established yet).

**Acceptance criteria:**
- Kill switch does not fire within the first minute of startup before balance is established.
- Kill switch does not fire due to balance oscillation caused by REST sync timing.

---

### P1 — Reliability

---

#### BUG-6: Market blacklisting is permanent within a session and not reset on success

**File:** `src/main.rs:821–832`

**What happens:** A market is added to `skip_markets` after 3 `invalid_order` responses.
The counter in `market_failures` is never reset on a successful order. Once blacklisted,
a market is skipped for the entire session even if the underlying issue was transient
(e.g., price moved outside valid range during volatility, then stabilized).

**Fix:** Reset `market_failures[market]` to 0 on any successful order create for that
market. Consider clearing the skip_markets entry after N successful ticks without error.

**Acceptance criteria:**
- A market that had a transient invalid_order error but then produces 3+ successful
  orders is no longer in skip_markets.

---

#### BUG-7: State not cleared on bot stop — stale books persist across sessions

**Files:** `src/main.rs:388–405`, `src/bot_state.rs`

**What happens:** When the bot stops (via `BotCommand::Stop`), the bot-control loop
only signals the trading task to stop. It does NOT call `engine.clear_all()`. Books,
market metadata, and open_orders from the previous session persist in `state_engine`.

When the bot is started again, `reconcile_startup` adds new orders/positions on top of
the stale state. Stale book data from a previous session is used for quote generation
until new WS snapshots arrive, which could be minutes.

**Fix:** Call `engine.clear_all()` at the start of `run_trading_loop` (before
`reconcile_startup`), or in the bot-control loop immediately after the trading task
exits. Do NOT clear during Stop command, but DO clear before the next Start.

**Acceptance criteria:**
- After stop + start, `engine.books().len()` equals the number of markets selected in
  the new startup reconcile, not the previous session's count.

---

#### BUG-8: Config changes (max_markets, tick_interval) not respected during a session

**Files:** `src/main.rs:597`, `src/main.rs:586`

**What happens:** `max_markets` is captured once from `cfg` at the start of
`run_trading_loop` and never re-read. `tick_interval` is also fixed at startup. If the
user updates these via the API (which persists overrides to DB), the changes don't take
effect until the bot is restarted.

**Fix:** In the `rescan_tick` branch, re-read `max_markets` from the shared config:
```rust
let cfg = config.read().await;
let max_markets = cfg.trading.max_markets_active;
```
For `tick_interval`, accept that it requires a restart — document this in the API
response when the config is updated.

**Acceptance criteria:**
- Changing `max_markets_active` via the API and waiting for the next rescan cycle
  adjusts the active market count without restarting the bot.

---

#### BUG-9: `reconcile_startup` fails on transient exchange errors, hard-fails the bot

**File:** `src/main.rs:493–509`

**What happens:** If any of the 4 startup steps fail (balance, orders, positions,
market scan), `reconcile_startup` returns an `Err` and the bot transitions to
`BotState::Error`. Steps 1-3 can fail on transient 5xx errors. The bot cannot recover
without a manual restart.

**Fix:** Add retry logic (3 attempts, exponential backoff) for steps 1-3. The market
scan (step 4) failing is more serious — it's acceptable to hard-fail there.

**Acceptance criteria:**
- A single transient 503 from Kalshi during startup does not prevent the bot from
  reaching `BotState::Running`.

---

#### BUG-10: `active_market_count()` returns `market_meta.len()`, not actual quoted markets

**File:** `src/state/mod.rs:322–325`

**What happens:** The dashboard status endpoint shows `active_markets` which calls
`engine.active_market_count()`. This counts `market_meta.len()`, but:
- Books can exist without metadata (created by WS events for tickers outside the active set).
- After a stop without `clear_all()`, stale metadata inflates the count.

The `books` map and `market_meta` map are not kept in sync: you can have a book with no
metadata (WS-injected), or metadata with no book (edge case after remove_market race).

**Fix:** `active_market_count()` should return the count of tickers present in BOTH
`books` AND `market_meta`. Or better: maintain a single `active_markets: HashSet<MarketTicker>`
that is the authoritative set.

**Acceptance criteria:**
- `active_markets` in the dashboard status matches the number of markets being quoted
  in the current tick.

---

### P2 — Strategy / Profitability

---

#### BUG-11: Fair value uses inventory-adjusted microprice as the quoting center

**File:** `src/fair_value/mod.rs:59–62`

**What happens:** The inventory penalty is applied to the fair value:
```rust
let inv_adj = -k1 * inventory - k3 * inventory^3;
let raw_fair = microprice + imbalance_adj + trade_sign_adj + inv_adj;
```
Then in `strategy/mod.rs`, the bid/ask are placed symmetrically around this
inventory-skewed fair value. This double-counts the inventory adjustment because the
strategy also applies an explicit `inventory_skew_coeff`.

The result: heavy inventory causes quotes to drift far from the book mid, which either
gets the bot no fills (if it's quoting into the spread) or gets it crossed (if the
skewed fair value is wrong).

**Fix:** Keep the fair value as a pure microstructure estimate (microprice + flow signals,
NO inventory adjustment). Apply inventory skew ONLY in the strategy layer, as an
asymmetric shift: widen on one side, narrow on the other.

**Acceptance criteria:**
- With zero inventory, `fv.price` equals microprice + imbalance + trade-sign adjustments.
- With long inventory, bids are lower than fair and asks are unchanged (or vice versa),
  not both shifted.

---

#### BUG-12: Quote prices are in [0,1] dollars but Kalshi expects cents

**Files:** `src/strategy/mod.rs`, `src/exchange/rest.rs` (CreateOrderRequest)

**What happens:** Strategy computes prices as `Decimal` in the [0, 1] range (e.g., 0.47
for 47¢). The `CreateOrderRequest` struct has `yes_price_dollars: Option<String>`.
If Kalshi's API expects integer cents (47) rather than fractional dollars (0.47), every
order is priced incorrectly. Confirm which unit the REST client sends and whether the
API returns `invalid_order` because of this.

**Action:** Audit `KalshiRestClient::create_order` to verify the unit convention. Add
an integration test that creates a limit order at a known price in demo and verifies
the resting price via `get_order`.

**Acceptance criteria:**
- An order placed at 0.47 rests at 47¢ on Kalshi's book, confirmed via API round-trip
  in demo environment.

---

#### BUG-13: Strategy skips markets with no position, missing the full book context

**File:** `src/main.rs:1020–1029`

**What happens:** `compute_target_quotes` skips any market without metadata loaded.
During startup before metadata is fetched (or if metadata fetch failed per BUG-2),
the market is silently skipped. No log at WARN level — only a debug log — so the
dashboard appears to be running but many markets generate zero quotes.

**Fix:** Log at INFO or WARN when a market with a book has no metadata. After BUG-2
is fixed, this should not happen but the log acts as a sentinel.

---

## What Success Looks Like

The bot is stable when all of the following hold for a 4-hour demo session:

1. `active_markets` in the dashboard matches `max_markets_active` from config (±1
   for filter mismatches).
2. The bot does not terminate with kill-switch or Error state due to PnL accounting
   errors.
3. Dashboard PnL does not jump/reset every 2 minutes.
4. After stop → start, the bot resumes at the correct market count without stale state.
5. No market is permanently blacklisted due to a single transient error.
6. Session PnL shown in the dashboard matches the account equity change on Kalshi.

Profitability is a separate concern — fix correctness first.

---

## Build Order

Fix bugs in this order:

1. **BUG-4** (balance sync) — prerequisite for BUG-3 and BUG-5.
2. **BUG-5** (kill switch) — stops the unexpected termination.
3. **BUG-1** (market count) — stops the market accumulation.
4. **BUG-2** (metadata fetch) — unblocks correct quoting on newly-scanned markets.
5. **BUG-3** (PnL accounting) — now that balance is stable, fix realized PnL tracking.
6. **BUG-7** (stale state on restart) — clean start/stop cycle.
7. **BUG-10** (active_market_count) — dashboard correctness.
8. **BUG-6** (blacklist reset) — reduce unnecessary market skips.
9. **BUG-8** (live config reload) — live parameter tuning.
10. **BUG-9** (startup retry) — resilience to transient errors.
11. **BUG-11** (double inventory skew) — strategy correctness.
12. **BUG-12** (price units) — verify and document, add integration test.
13. **BUG-13** (log missing metadata) — observability.

---

## Out of Scope (for this spec)

- Adding new strategy types.
- Backtesting infrastructure.
- Dashboard UI changes beyond what's needed to surface correct data.
- Prometheus metrics.
- ML-based fair value models.
