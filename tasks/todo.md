# Kalshi Bot — Stabilization Task List

Legend: [ ] todo | [~] in progress | [x] done

---

## Phase 1 — Stop the Bleeding

- [x] **Task 1** — Eliminate redundant balance sync; stabilize kill switch
  - Remove `balance.available += cash_delta` from fill handler (`state/mod.rs:568`)
  - Remove `realized_pnl += cash_delta` from fill handler (`state/mod.rs:566–567`)
  - Remove duplicate balance+positions sync from `order_sync_tick` (`main.rs:633–654`)
  - Add `daily_start_equity == 0` guard in kill switch (`risk/mod.rs:56`)
  - ✓ Dashboard PnL does not jump at 2-minute mark
  - ✓ Kill switch does not fire on first startup

- [x] **Task 2** — Fix market count enforcement and metadata fetch in rescan
  - Compute dropped markets after rescan and call `engine.remove_market()` for each
  - Unsubscribe dropped markets from WS
  - Replace broken `get_markets(None, Some(1))` with market_data_map lookup
  - ✓ `active_market_count()` == `max_markets_active` after first rescan cycle
  - ✓ Newly-added markets have correct tick_size, close_time, category

**→ Checkpoint A: 30-min demo run, bot stays running, market count stable**

---

## Phase 2 — State Lifecycle

- [x] **Task 3** — Fix realized PnL accounting (round-trip, not cash flows)
  - Add `avg_yes_price` / `avg_no_price` update on buys in fill handler
  - Compute realized PnL only when reducing a position
  - ✓ Buy 5 + sell 5 → realized PnL ≈ (exit − entry) × 5 − fees

- [x] **Task 4** — Clear stale state on restart; fix active_market_count
  - Add `clear_books_and_meta()` to StateEngine, call at top of `run_trading_loop`
  - Fix `active_market_count()` to count intersection of books ∩ market_meta
  - ✓ Stop → start → market count == max_markets_active (not accumulated)

**→ Checkpoint B: realized PnL correct, clean restart verified**

---

## Phase 3 — Reliability

- [x] **Task 5** — Fix permanent market blacklisting
  - Reset `market_failures[market]` on successful order create
  - Remove from `skip_markets` when counter resets to 0
  - ✓ Market recovers from blacklist after successful order

- [x] **Task 6** — Add startup retry for transient exchange errors (steps 1–3)
  - Wrap balance / orders / positions fetches in retry (3 attempts, exp backoff)
  - ✓ Single 503 on startup does not prevent reaching `BotState::Running`

- [x] **Task 7** — Live reload max_markets from shared config in rescan
  - Read `max_markets` from `config.read().await` inside `rescan_tick` branch
  - ✓ Changing max_markets via API takes effect after next rescan, no restart needed

---

## Phase 4 — Strategy Correctness

- [x] **Task 8** — Remove double inventory skew from fair value engine
  - Delete `inv_adj` from `FairValueEngine.compute()` (`fair_value/mod.rs`)
  - Remove `inventory_penalty_k1 / k3` from FairValueEngine struct and `new()`
  - Update `fair_value_tests.rs` to match new behavior
  - ✓ With zero inventory, fv.price == microprice + signal adjustments only

- [x] **Task 9** — Audit price unit convention (investigation)
  - Trace `yes_price_dollars` through `create_order` to HTTP body
  - Place one order in demo, confirm resting price via `get_order`
  - Document convention in code comment
  - Add integration test (gated by `#[ignore]`)
  - ✓ Placed order at P rests at P on exchange

- [x] **Task 10** — Log missing metadata at WARN
  - Change `debug!` to `warn!` for "Skipping market: no metadata loaded" (`main.rs:~1026`)
  - ✓ Warning visible in default log output when market has book but no metadata

**→ Final Checkpoint: 4-hour demo run, all criteria pass**
