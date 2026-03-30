Below is a paste-ready spec doc for Cursor.

````md
# Kalshi Market-Making / Mid-Frequency Trading System Spec

## 1. Objective

Build a production-style trading system for Kalshi that targets **mid-frequency liquidity provision and selective event-driven trading**, not ultra-low-latency equity-style HFT.

The system should:
- stream Kalshi market data in real time
- maintain live order books and fair values
- place and manage passive limit orders
- control inventory and market-specific risk
- record all orders, fills, PnL, and system events
- support both **paper/demo trading** and live deployment

This design is optimized for:
- **spread capture**
- **inventory-aware market making**
- **cross-market sanity checks**
- **selective event repricing**

It is **not** designed for:
- pure microsecond latency racing
- uninformed directional gambling
- aggressive taker-heavy trading

Kalshi provides official REST, WebSocket, FIX, Python SDK, and TypeScript SDK support, plus demo and production environments. :contentReference[oaicite:0]{index=0}

---

## 2. Reality and Constraints

### 2.1 Exchange structure
Kalshi is a binary market venue. The API and docs treat markets as YES/NO contracts, and selling YES is equivalent to buying NO. The order book exposes **yes bids** and **no bids**, with asks being implied by binary complementarity. :contentReference[oaicite:1]{index=1}

### 2.2 Execution model
The primary strategy should rely on **resting limit orders**, because:
- Kalshi supports explicit order creation and order management through authenticated endpoints. :contentReference[oaicite:2]{index=2}
- maker fees can apply to resting orders when they execute, but canceling resting orders itself does not incur fees. :contentReference[oaicite:3]{index=3}
- Kalshi has a formal market maker program and a separate liquidity incentive program, which strongly suggests passive quoting is the intended systematic style for serious API users. :contentReference[oaicite:4]{index=4}

### 2.3 Rate limits
Kalshi rate limits vary by API tier. Their docs list tiers from Basic through Prime, with per-second read/write limits that can become a real architecture constraint. The system must therefore be built with **event-driven streaming + local state**, not poll-heavy logic. :contentReference[oaicite:5]{index=5}

### 2.4 Market limitations
This system assumes:
- liquidity is thinner than equities or major crypto venues
- spreads may be wide
- queue position matters
- fees materially affect edge
- the best opportunities come from **market microstructure inefficiencies**, not raw speed alone

Those last points are engineering assumptions, not official Kalshi rules.

---

## 3. Product Scope

### Phase 1
Single-process research bot:
- live market ingest
- local order book
- fair value engine
- passive quoting
- logging
- kill switch

### Phase 2
Production trading service:
- split services by responsibility
- persistent state store
- robust recovery
- strategy configuration
- monitoring dashboard

### Phase 3
Multi-strategy engine:
- market making
- event-driven repricing
- pair / cross-market consistency checks
- market-specific models

---

## 4. High-Level Architecture

```text
                    +----------------------+
                    |   Kalshi REST API    |
                    +----------+-----------+
                               |
                               |
                    +----------v-----------+
                    |   Kalshi WebSocket   |
                    +----------+-----------+
                               |
        +----------------------+----------------------+
        |                                             |
+-------v--------+                            +-------v--------+
| Market Data    |                            | User Data      |
| Ingest Service |                            | Ingest Service |
| orderbooks     |                            | fills/orders   |
| trades         |                            | positions      |
+-------+--------+                            +-------+--------+
        |                                             |
        +----------------------+----------------------+
                               |
                    +----------v-----------+
                    |   State Engine       |
                    | books, positions,    |
                    | signals, health      |
                    +----------+-----------+
                               |
          +--------------------+--------------------+
          |                    |                    |
+---------v--------+  +--------v---------+  +------v--------+
| Fair Value       |  | Strategy Engine  |  | Risk Engine   |
| Engine           |  | quote decisions  |  | limits/skew   |
+---------+--------+  +--------+---------+  +------+--------+
          |                    |                    |
          +--------------------+--------------------+
                               |
                    +----------v-----------+
                    | Execution Engine     |
                    | create/cancel/replace|
                    +----------+-----------+
                               |
                    +----------v-----------+
                    | Persistence / Audit  |
                    | Postgres + logs      |
                    +----------+-----------+
                               |
                    +----------v-----------+
                    | Monitoring Dashboard |
                    +----------------------+
````

---

## 5. Recommended Tech Stack

## 5.1 Core stack

Use a **Rust + Python hybrid**.

### Rust responsibilities

* WebSocket connectivity
* order book maintenance
* quote generation
* execution path
* risk gate checks
* high-throughput event bus
* deterministic state machine

### Python responsibilities

* research notebooks
* backtests
* offline feature engineering
* fair value model prototyping
* analytics and reporting

### Postgres

Use Postgres for:

* fills
* orders
* position snapshots
* strategy decisions
* PnL records
* replay and audit

### Redis

Optional, for:

* low-latency shared state
* pub/sub between services
* cache for hot market metadata

### Frontend

Use Next.js or a simple React dashboard for:

* live quotes
* active orders
* per-market inventory
* PnL
* system health
* error events

---

## 6. Exchange/API Integration Requirements

### 6.1 Environments

Support both:

* **demo environment**
* **production environment**

Kalshi explicitly documents a demo environment and separate authenticated/public quick starts. ([Kalshi API Documentation][1])

### 6.2 Authentication

Authenticated requests require API keys and signed headers. Kalshi documents authenticated REST requests and also requires authentication during the WebSocket handshake. ([Kalshi API Documentation][2])

### 6.3 Required API capabilities

The system must integrate with:

* market discovery
* order book fetch
* candlestick/history fetch
* order create
* order query
* fills query
* balance query
* user fill stream

Kalshi documents all of these. ([Kalshi API Documentation][3])

### 6.4 Important exchange quirks

* `GET /markets/{ticker}/orderbook` returns yes bids and no bids only; asks are implied. ([Kalshi API Documentation][4])
* order creation is authenticated and Kalshi states each user is limited to **200,000 open orders at a time**. ([Kalshi API Documentation][5])
* historical data older than cutoff timestamps must be pulled from historical endpoints, not live ones. ([Kalshi API Documentation][6])

---

## 7. Service Design

## 7.1 Market Data Ingest Service

### Responsibilities

* connect to Kalshi WebSocket
* subscribe to order book / market data channels
* normalize messages into internal event types
* maintain top-of-book and full local ladders
* recover from disconnects cleanly

### Inputs

* public market data
* order book snapshots
* incremental updates
* candlestick/history fallback via REST

### Outputs

* normalized market events
* local book state
* top of book
* spread
* mid price
* microprice
* recent trade stats

### Internal event schema

```ts
type MarketEvent =
  | { type: "BOOK_SNAPSHOT"; market: string; ts: number; yesBids: [number, number][]; noBids: [number, number][] }
  | { type: "BOOK_DELTA"; market: string; ts: number; side: "YES" | "NO"; price: number; qty: number }
  | { type: "TRADE"; market: string; ts: number; price: number; qty: number }
  | { type: "MARKET_STATUS"; market: string; ts: number; status: string }
```

### Notes

Do not poll order books at high frequency unless reconnecting or repairing state. Use WebSockets as the primary path because Kalshi provides authenticated WebSocket streaming and rate-limited REST. ([Kalshi API Documentation][7])

---

## 7.2 User Data Ingest Service

### Responsibilities

* subscribe to user fill notifications
* reconcile local open orders with exchange state
* fetch orders/fills on startup and periodically as a repair mechanism
* update live positions and cash

Kalshi documents a dedicated user fills WebSocket feed and separate REST endpoints for orders, fills, and balances. ([Kalshi API Documentation][8])

### Internal event schema

```ts
type UserEvent =
  | { type: "ORDER_ACK"; orderId: string; market: string; ts: number; side: "YES" | "NO"; price: number; qty: number }
  | { type: "ORDER_CANCEL"; orderId: string; market: string; ts: number }
  | { type: "FILL"; fillId: string; orderId: string; market: string; ts: number; price: number; qty: number; feeCents: number }
  | { type: "BALANCE"; ts: number; availableCents: number; portfolioValueCents: number }
```

---

## 7.3 State Engine

### Responsibilities

Single source of truth for:

* live order books
* open orders
* recent fills
* inventory by market
* cash and reserved capital
* exchange connectivity state
* strategy state

### Design requirement

State updates must be **event sourced** and replayable. Every external event and internal action should be appended to durable storage so the system can reconstruct state after restart.

---

## 7.4 Fair Value Engine

### Goal

Estimate a fair probability for each market.

### Version 1 fair value

For a first production version, use simple microstructure-driven fair values:

* book mid
* weighted mid / microprice
* recent trade imbalance
* spread regime
* time-to-expiry adjustment
* inventory penalty adjustment

### Formula

```text
raw_fair = weighted_mid + alpha1 * order_imbalance + alpha2 * recent_trade_sign + alpha3 * short_term_reversion
fair = clamp(raw_fair + inventory_adjustment + event_adjustment, 1, 99)
```

All prices are handled in **cents / integer ticks**.

### Version 2 fair value

Add event-specific priors:

* weather markets: external weather APIs
* macro markets: econ calendar + consensus forecasts
* sports markets: live game state feed
* approval/election markets: polling aggregations, if permitted and worthwhile

Those external integrations are strategy recommendations, not from Kalshi docs.

---

## 7.5 Strategy Engine

The strategy engine consumes fair values and state, then emits desired quotes.

### Strategy 1: Inventory-Aware Market Maker

For each market:

1. compute fair value
2. decide quoting width
3. choose bid and ask around fair
4. skew quotes based on inventory
5. avoid quoting when market is stale, locked, or too wide

#### Inputs

* fair value
* top of book
* local queue assumptions
* current inventory
* market risk score
* fees
* time to expiration

#### Outputs

Desired target quotes:

```ts
type TargetQuote = {
  market: string
  yesBid?: { price: number; qty: number }
  yesAsk?: { price: number; qty: number }
  noBid?: { price: number; qty: number }
  noAsk?: { price: number; qty: number }
  reason: string
}
```

### Strategy 2: Event Repricing Overlay

When an external event materially changes fair value:

* widen or pull stale quotes
* recalculate fair
* reenter around new fair only after state stabilizes

### Strategy 3: Cross-Market Sanity Checks

Use rule-based consistency checks across related markets:

* mutually exclusive market baskets
* event collections
* contract complements
* multivariate structures where applicable

Kalshi exposes multivariate event collection endpoints, which is useful if you later build cross-market consistency logic. ([Kalshi API Documentation][9])

---

## 8. Execution Engine

## 8.1 Responsibilities

* compare current live orders vs desired quotes
* decide create/cancel/replace actions
* avoid needless churn
* throttle requests to stay under rate limits
* enforce risk approval before every outbound action

## 8.2 Order policy

Default to:

* passive limit orders
* small clip sizes
* no marketable sweeping unless explicitly enabled
* no crossing spread unless an event-driven signal exceeds threshold

## 8.3 Cancel/replace logic

```text
if desired quote missing and live order exists:
    cancel
if desired quote exists and no live order exists:
    create
if desired quote exists and live order differs by >= price_threshold or size_threshold:
    cancel and recreate
else:
    hold
```

## 8.4 Anti-churn rules

* minimum rest time before cancel unless hard risk event
* no more than X updates per market per Y seconds
* do not reprice on tiny fair-value noise
* batch actions across markets when safe

This is important because API rate limits are finite and open orders are capped. ([Kalshi API Documentation][10])

---

## 9. Risk Engine

This is the most important non-exchange component.

## 9.1 Hard limits

Per market:

* max gross contracts
* max net directional exposure
* max resting order quantity
* max notional at risk
* max quote width
* max drawdown before disable

Portfolio-wide:

* max open order count
* max reserved capital
* max correlated market exposure
* max loss per day
* max loss per event category
* max number of active markets

## 9.2 Soft controls

* inventory skewing
* quote size scaling under stress
* auto-widen during volatility
* disable quoting on connection degradation

## 9.3 Kill switches

Immediate cancel-all and strategy disable if:

* WebSocket desync detected
* order/fill reconciliation mismatch exceeds threshold
* drawdown exceeds daily limit
* time sync or signature errors persist
* exchange status abnormal
* duplicate order bug suspected

---

## 10. Inventory Model

Treat each market as its own bounded inventory process.

### State

```ts
type MarketInventory = {
  market: string
  yesContracts: number
  noContracts: number
  avgYesPrice?: number
  avgNoPrice?: number
  realizedPnlCents: number
  unrealizedPnlCents: number
  reservedCapitalCents: number
}
```

### Inventory skew

If long YES:

* quote less aggressively on YES bid
* quote more aggressively on YES ask / NO bid equivalent
* bias fair downward slightly

If short YES:

* opposite treatment

### Inventory penalty

```text
inventory_adjustment = -k1 * normalized_inventory - k2 * inventory^3
```

This discourages one-sided accumulation.

---

## 11. Data Model / Database Schema

Use Postgres.

## 11.1 Tables

### markets

```sql
CREATE TABLE markets (
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
```

### book_snapshots

```sql
CREATE TABLE book_snapshots (
  id BIGSERIAL PRIMARY KEY,
  market_ticker TEXT NOT NULL,
  ts TIMESTAMPTZ NOT NULL,
  yes_bids JSONB NOT NULL,
  no_bids JSONB NOT NULL
);
CREATE INDEX idx_book_snapshots_market_ts ON book_snapshots(market_ticker, ts DESC);
```

### orders

```sql
CREATE TABLE orders (
  order_id TEXT PRIMARY KEY,
  market_ticker TEXT NOT NULL,
  side TEXT NOT NULL,
  action TEXT NOT NULL,
  price_cents INT NOT NULL,
  quantity INT NOT NULL,
  status TEXT NOT NULL,
  client_order_id TEXT,
  created_ts TIMESTAMPTZ NOT NULL,
  updated_ts TIMESTAMPTZ NOT NULL,
  raw JSONB
);
```

### fills

```sql
CREATE TABLE fills (
  fill_id TEXT PRIMARY KEY,
  order_id TEXT NOT NULL,
  market_ticker TEXT NOT NULL,
  price_cents INT NOT NULL,
  quantity INT NOT NULL,
  fee_cents INT DEFAULT 0,
  fill_ts TIMESTAMPTZ NOT NULL,
  raw JSONB
);
CREATE INDEX idx_fills_market_ts ON fills(market_ticker, fill_ts DESC);
```

### positions

```sql
CREATE TABLE positions (
  market_ticker TEXT PRIMARY KEY,
  yes_contracts INT NOT NULL DEFAULT 0,
  no_contracts INT NOT NULL DEFAULT 0,
  avg_yes_price NUMERIC,
  avg_no_price NUMERIC,
  realized_pnl_cents BIGINT NOT NULL DEFAULT 0,
  unrealized_pnl_cents BIGINT NOT NULL DEFAULT 0,
  updated_ts TIMESTAMPTZ NOT NULL
);
```

### strategy_decisions

```sql
CREATE TABLE strategy_decisions (
  id BIGSERIAL PRIMARY KEY,
  market_ticker TEXT NOT NULL,
  ts TIMESTAMPTZ NOT NULL,
  fair_value_cents NUMERIC NOT NULL,
  inventory INT NOT NULL,
  target_quotes JSONB NOT NULL,
  features JSONB,
  reason TEXT
);
```

### risk_events

```sql
CREATE TABLE risk_events (
  id BIGSERIAL PRIMARY KEY,
  ts TIMESTAMPTZ NOT NULL,
  severity TEXT NOT NULL,
  component TEXT NOT NULL,
  market_ticker TEXT,
  message TEXT NOT NULL,
  payload JSONB
);
```

---

## 12. Strategy Logic

## 12.1 Quoting equations

Let:

* `f` = fair value in cents
* `s_base` = base half-spread
* `s_inv` = inventory penalty
* `s_vol` = volatility penalty
* `s_fee` = fee adjustment
* `s_total = s_base + s_inv + s_vol + s_fee`

Then:

```text
bid = floor(f - s_total)
ask = ceil(f + s_total)
```

Clamp:

```text
bid >= 1
ask <= 99
ask > bid
```

### Size function

```text
qty = base_qty * liquidity_factor * confidence_factor * capital_factor
```

### Inventory skew

```text
bid = bid - skew_coeff * inventory
ask = ask - skew_coeff * inventory
```

Interpretation:

* if inventory is long, both quotes move lower to encourage selling and discourage more buying

## 12.2 Participation filters

Do not quote when:

* spread too tight to overcome fees
* market stale or disconnected
* fair value confidence too low
* capital below reserve threshold
* event too close to resolution and state is uncertain
* market category disabled

---

## 13. Backtesting / Simulation Requirements

## 13.1 Goal

Before live deployment, build a replay system that simulates:

* order book updates
* quote placement
* fills
* fees
* inventory evolution
* realized and unrealized PnL

## 13.2 Historical data

Kalshi exposes historical markets, historical trades, historical fills, historical orders, and historical cutoff timestamps, plus candlesticks for both live and historical markets. ([Kalshi API Documentation][6])

### Important caveat

Historical candlesticks alone are **not enough** for market-making simulation. Candles lose queue and microstructure detail. You need the best market data you can store yourself from live streaming if you want realistic execution research.

## 13.3 Fill model

For the first backtester:

* assume fill if your bid crosses future traded price or if your ask is reached
* optional partial-fill model using displayed size and queue assumptions
* add maker/taker fee model

## 13.4 Metrics

Track:

* gross PnL
* net PnL after fees
* Sharpe-like daily stability
* max drawdown
* fill ratio
* quote-to-fill ratio
* cancel rate
* average holding time
* adverse selection after fills
* per-market performance

---

## 14. Monitoring and Observability

## 14.1 Structured logs

Every service must emit JSON logs with:

* timestamp
* service
* market
* action
* order_id
* severity
* message
* payload

## 14.2 Metrics

Export Prometheus metrics:

* websocket_connected
* inbound_market_events_per_sec
* outbound_order_requests_per_sec
* open_orders_count
* fills_per_min
* gross_exposure
* net_exposure
* pnl_realized_cents
* pnl_unrealized_cents
* strategy_enabled
* market_data_lag_ms
* order_ack_lag_ms
* position_reconcile_failures

## 14.3 Dashboard

Dashboard should show:

* live PnL
* open positions
* top risky markets
* current spreads
* active quotes
* API error rate
* reconnect count
* daily drawdown
* kill switch state

---

## 15. Security and Secrets

* store API secrets in environment variables or secret manager
* never log raw secrets
* separate demo and production credentials
* require manual enable flag for live order placement
* require explicit account/environment banner in dashboard
* use IP allowlisting if possible in deployment environment

---

## 16. Deployment

## 16.1 Development

* docker-compose
* demo environment only
* local Postgres
* local Redis
* simple dashboard

## 16.2 Production

* deploy Rust services in containers
* use managed Postgres
* use persistent logs
* run on low-latency VPS / bare-metal close to Kalshi endpoints if measurable
* maintain NTP time sync
* blue/green deploy for execution service

## 16.3 Recommended repos

```text
kalshi-bot/
  apps/
    market-data-service/
    user-data-service/
    execution-service/
    strategy-service/
    dashboard/
  libs/
    exchange-client-rust/
    exchange-client-python/
    shared-models/
    risk/
    backtester/
  infra/
    docker/
    terraform/
  sql/
  notebooks/
  docs/
```

---

## 17. MVP Build Order

## Week 1

* API auth
* market discovery
* WebSocket market data ingest
* local order book
* demo trading only

## Week 2

* order create/cancel
* open-order tracking
* fill reconciliation
* inventory state
* Postgres persistence

## Week 3

* fair value engine v1
* market-making logic
* risk limits
* kill switch
* dashboard

## Week 4

* backtest/replay tooling
* performance metrics
* anti-churn logic
* config system
* demo forward test

## Week 5+

* live small-size deployment
* event overlays
* market-specific priors
* multi-market optimization
* cross-market consistency logic

---

## 18. Configuration Spec

Use YAML.

```yaml
environment: demo

exchange:
  rest_base_url: "https://demo-api.kalshi.co/trade-api/v2"
  ws_url: "wss://demo-api.kalshi.co/trade-api/ws/v2"
  api_key_env: "KALSHI_API_KEY"
  private_key_env: "KALSHI_PRIVATE_KEY"

trading:
  enabled: false
  markets_allowlist: []
  categories_allowlist: ["Economics", "Weather", "Sports"]
  max_open_orders: 500
  max_markets_active: 30

strategy:
  base_half_spread_cents: 2
  min_edge_after_fees_cents: 1.5
  default_order_size: 5
  max_order_size: 25
  min_rest_ms: 1500
  repricing_threshold_cents: 1
  inventory_skew_coeff: 0.25
  volatility_widen_coeff: 0.40

risk:
  max_loss_daily_cents: 25000
  max_market_notional_cents: 150000
  max_market_inventory_contracts: 100
  max_total_reserved_cents: 400000
  cancel_all_on_disconnect: true

monitoring:
  prometheus_port: 9100
  log_level: "info"

database:
  url_env: "DATABASE_URL"

redis:
  enabled: true
  url_env: "REDIS_URL"
```

---

## 19. Non-Goals

This version will not include:

* deep ML fair value models at launch
* cross-venue arbitration execution at launch
* full FIX integration at launch
* options-style portfolio optimization
* autonomous capital sizing using reinforcement learning

Kalshi does support FIX, but that should wait until the REST/WebSocket system is stable. ([Kalshi API Documentation][11])

---

## 20. Acceptance Criteria

The MVP is successful when it can:

1. connect to demo environment and maintain stable streams for 8+ hours
2. reconstruct and maintain local order books correctly
3. create, cancel, and reconcile orders without manual intervention
4. track fills and positions correctly in real time
5. enforce hard risk limits and kill switch behavior
6. quote passively in at least 10 markets simultaneously
7. persist all decisions and fills for replay
8. produce a daily PnL and risk report
9. survive reconnects and process restarts without broken state
10. be toggled from demo to live with config-only changes

---

## 21. Engineering Notes for Cursor

When implementing this project:

* prioritize correctness over optimization first
* build exchange adapters behind interfaces
* keep all money and prices in integer cents
* make strategy output deterministic for a fixed event stream
* design for replayability from day one
* include exhaustive integration tests around order lifecycle
* treat every external message as unreliable until validated
* never allow execution service to bypass risk service
* add a hard `TRADING_ENABLED=false` default everywhere
* make demo the default environment in all local configs

---

## 22. Suggested First Interfaces

### Rust trait

```rust
pub trait ExchangeClient {
    fn get_balance(&self) -> anyhow::Result<Balance>;
    fn get_open_orders(&self) -> anyhow::Result<Vec<Order>>;
    fn create_order(&self, req: CreateOrderRequest) -> anyhow::Result<OrderAck>;
    fn cancel_order(&self, order_id: &str) -> anyhow::Result<()>;
    fn get_orderbook(&self, market_ticker: &str) -> anyhow::Result<OrderBook>;
}
```

### Strategy trait

```rust
pub trait Strategy {
    fn on_market_event(&mut self, event: &MarketEvent, state: &StateView) -> Vec<DesiredAction>;
    fn on_user_event(&mut self, event: &UserEvent, state: &StateView) -> Vec<DesiredAction>;
}
```

### Risk trait

```rust
pub trait RiskManager {
    fn approve(&self, action: &DesiredAction, state: &StateView) -> RiskDecision;
}
```

---

## 23. Final Product Direction

The first real version should be a:

* **passive, inventory-aware, multi-market quoting engine**
* with **tight risk controls**
* running in **demo first**
* then **tiny live size**
* with **research tooling strong enough to improve fair value models over time**

That gives the best chance of building something that is actually tradable instead of just technically impressive.

```

A clean next step is to turn this into a repo-ready `README.md` plus a folder-by-folder implementation checklist.
```

[1]: https://docs.kalshi.com/welcome?utm_source=chatgpt.com "Introduction - API Documentation"
[2]: https://docs.kalshi.com/getting_started/quick_start_authenticated_requests?utm_source=chatgpt.com "Quick Start: Authenticated Requests (No SDK)"
[3]: https://docs.kalshi.com/typescript-sdk/api/MarketsApi?utm_source=chatgpt.com "Markets - API Documentation"
[4]: https://docs.kalshi.com/api-reference/market/get-market-orderbook?utm_source=chatgpt.com "Get Market Orderbook - API Documentation"
[5]: https://docs.kalshi.com/api-reference/orders/create-order?utm_source=chatgpt.com "Create Order - API Documentation"
[6]: https://docs.kalshi.com/getting_started/historical_data?utm_source=chatgpt.com "Historical Data - API Documentation"
[7]: https://docs.kalshi.com/websockets/websocket-connection?utm_source=chatgpt.com "WebSocket Connection - API Documentation"
[8]: https://docs.kalshi.com/websockets/user-fills?utm_source=chatgpt.com "User Fills - API Documentation"
[9]: https://docs.kalshi.com/llms.txt?utm_source=chatgpt.com "llms.txt"
[10]: https://docs.kalshi.com/getting_started/rate_limits?utm_source=chatgpt.com "Rate Limits and Tiers - API Documentation"
[11]: https://docs.kalshi.com/fix/order-entry?utm_source=chatgpt.com "Order Entry Messages - API Documentation"
