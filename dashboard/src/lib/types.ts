export interface BotStatus {
  state: "stopped" | "starting" | "running" | "stopping" | "error" | "switching";
  environment: "demo" | "production";
  uptime_secs: number | null;
  connectivity: string;
  active_markets: number;
  open_orders: number;
  trading_enabled: boolean;
  error_message: string | null;
}

export interface BalanceInfo {
  available: string;
  portfolio_value: string;
  total_reserved: string;
}

export interface PnlData {
  daily_realized_pnl: string;
  snapshots: PnlSnapshot[];
}

export interface PnlSnapshot {
  ts: string;
  realized_pnl: string;
  unrealized_pnl: string;
  balance: string;
  portfolio_value: string;
  open_order_count: number;
  active_market_count: number;
}

export interface MarketSummary {
  ticker: string;
  mid: string | null;
  spread: string | null;
  best_bid: string | null;
  best_ask: string | null;
  position: PositionSummary | null;
}

export interface MarketDetail {
  ticker: string;
  mid: string | null;
  spread: string | null;
  best_bid: string | null;
  best_ask: string | null;
  microprice: string | null;
  position: PositionSummary | null;
  open_orders: OrderInfo[];
}

export interface PositionSummary {
  yes_contracts: string;
  no_contracts: string;
  net_inventory: string;
  realized_pnl: string;
}

export interface OrderInfo {
  order_id: string;
  market_ticker: string;
  side: string;
  action: string;
  price: string;
  remaining_count: string;
  fill_count?: string;
  status?: string;
}

export interface PositionInfo {
  market_ticker: string;
  yes_contracts: string;
  no_contracts: string;
  net_inventory: string;
  realized_pnl: string;
  unrealized_pnl: string;
}

export interface FillInfo {
  fill_id: string;
  order_id: string;
  market_ticker: string;
  side: string;
  action: string;
  price: string;
  quantity: string;
  fee: string;
  is_taker: boolean;
  fill_ts: string;
}

export interface RiskEvent {
  ts: string;
  severity: string;
  component: string;
  market_ticker: string | null;
  message: string;
  payload: unknown;
}

export interface StrategyDecision {
  ts: string;
  market_ticker: string;
  fair_value: string;
  inventory: string;
  reason: string;
}

export interface StrategyConfig {
  base_half_spread: string;
  min_edge_after_fees: string;
  default_order_size: number;
  max_order_size: number;
  min_rest_ms: number;
  repricing_threshold: string;
  inventory_skew_coeff: string;
  volatility_widen_coeff: string;
  tick_interval_ms: number;
  order_imbalance_alpha: string;
  trade_sign_alpha: string;
  inventory_penalty_k1: string;
  inventory_penalty_k3: string;
}

export interface RiskConfig {
  max_loss_daily: string;
  max_market_notional: string;
  max_market_inventory_contracts: number;
  max_total_reserved: string;
  max_open_orders: number;
  cancel_all_on_disconnect: boolean;
  disconnect_timeout_secs: number;
  seq_gap_timeout_secs: number;
}

export interface TradingConfig {
  enabled: boolean;
  markets_allowlist: string[];
  categories_allowlist: string[];
  max_open_orders: number;
  max_markets_active: number;
}

export interface BotConfig {
  strategy: StrategyConfig;
  risk: RiskConfig;
  trading: TradingConfig;
}

export interface EnvironmentInfo {
  environment: string;
  is_demo: boolean;
}

export type WsEventType =
  | "state_change"
  | "pnl_tick"
  | "fill"
  | "order_update"
  | "risk_event"
  | "config_change";

export interface WsMessage {
  type: WsEventType;
  data: unknown;
}
