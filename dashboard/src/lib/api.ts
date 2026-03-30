import type {
  BotStatus,
  BalanceInfo,
  PnlData,
  MarketSummary,
  MarketDetail,
  OrderInfo,
  PositionInfo,
  FillInfo,
  RiskEvent,
  StrategyDecision,
  BotConfig,
  EnvironmentInfo,
  StrategyConfig,
  RiskConfig,
  TradingConfig,
} from "./types";

const BOT_API_URL =
  process.env.NEXT_PUBLIC_BOT_API_URL ||
  process.env.BOT_API_URL ||
  "http://localhost:8080";

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BOT_API_URL}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`API ${path}: ${res.status} ${body}`);
  }
  return res.json() as Promise<T>;
}

export const api = {
  getStatus: () => apiFetch<BotStatus>("/api/status"),
  getBalance: () => apiFetch<BalanceInfo>("/api/balance"),
  getPnl: () => apiFetch<PnlData>("/api/pnl"),
  getMarkets: () => apiFetch<MarketSummary[]>("/api/markets"),
  getMarketDetail: (ticker: string) =>
    apiFetch<MarketDetail>(`/api/markets/${ticker}`),
  getOrders: () => apiFetch<OrderInfo[]>("/api/orders"),
  getPositions: () => apiFetch<PositionInfo[]>("/api/positions"),
  getFills: (limit = 100) =>
    apiFetch<FillInfo[]>(`/api/fills?limit=${limit}`),
  getRiskEvents: (limit = 100) =>
    apiFetch<RiskEvent[]>(`/api/risk-events?limit=${limit}`),
  getStrategyDecisions: (limit = 100) =>
    apiFetch<StrategyDecision[]>(`/api/strategy-decisions?limit=${limit}`),
  getConfig: () => apiFetch<BotConfig>("/api/config"),
  getEnvironment: () => apiFetch<EnvironmentInfo>("/api/environment"),

  botStart: () =>
    apiFetch<{ status: string }>("/api/bot/start", { method: "POST" }),
  botStop: () =>
    apiFetch<{ status: string }>("/api/bot/stop", { method: "POST" }),
  botKill: () =>
    apiFetch<{ status: string }>("/api/bot/kill", { method: "POST" }),

  setEnvironment: (environment: string, confirm?: string) =>
    apiFetch<{ status: string }>("/api/environment", {
      method: "POST",
      body: JSON.stringify({ environment, confirm }),
    }),

  updateStrategy: (config: StrategyConfig) =>
    apiFetch<{ status: string }>("/api/config/strategy", {
      method: "PUT",
      body: JSON.stringify(config),
    }),
  updateRisk: (config: RiskConfig) =>
    apiFetch<{ status: string }>("/api/config/risk", {
      method: "PUT",
      body: JSON.stringify(config),
    }),
  updateTrading: (config: TradingConfig) =>
    apiFetch<{ status: string }>("/api/config/trading", {
      method: "PUT",
      body: JSON.stringify(config),
    }),
};
