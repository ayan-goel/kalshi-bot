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
  OffsetPage,
  RawLogEntry,
  CursorPage,
  BotConfig,
  EnvironmentInfo,
  StrategyConfig,
  RiskConfig,
  TradingConfig,
  PnlWindow,
} from "./types";

const BOT_API_URL =
  process.env.NEXT_PUBLIC_BOT_API_URL ||
  process.env.BOT_API_URL ||
  "http://localhost:8080";

const API_SECRET = process.env.NEXT_PUBLIC_BOT_API_SECRET || "";

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (API_SECRET) {
    headers["Authorization"] = `Bearer ${API_SECRET}`;
  }
  return headers;
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BOT_API_URL}${path}`, {
    ...init,
    headers: {
      ...authHeaders(),
      ...init?.headers,
    },
  });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`API ${path}: ${res.status} ${body}`);
  }
  return res.json() as Promise<T>;
}

export function getWsUrl(): string {
  const base =
    process.env.NEXT_PUBLIC_BOT_WS_URL ||
    (typeof window !== "undefined"
      ? `ws://${window.location.hostname}:8080/api/ws`
      : "");
  if (!base) return "";
  if (API_SECRET) {
    const sep = base.includes("?") ? "&" : "?";
    return `${base}${sep}token=${API_SECRET}`;
  }
  return base;
}

export const api = {
  getStatus: () => apiFetch<BotStatus>("/api/status"),
  getBalance: () => apiFetch<BalanceInfo>("/api/balance"),
  getPnl: (window: PnlWindow = "all") =>
    apiFetch<PnlData>(`/api/pnl?window=${window}`),
  getMarkets: () => apiFetch<MarketSummary[]>("/api/markets"),
  getMarketDetail: (ticker: string) =>
    apiFetch<MarketDetail>(`/api/markets/${ticker}`),
  getOrders: () => apiFetch<OrderInfo[]>("/api/orders"),
  getPositions: () => apiFetch<PositionInfo[]>("/api/positions"),
  getFills: (limit = 100) =>
    apiFetch<FillInfo[]>(`/api/fills?limit=${limit}`),
  getRiskEvents: (limit = 100, offset = 0) =>
    apiFetch<OffsetPage<RiskEvent>>(
      `/api/risk-events?limit=${limit}&offset=${offset}`
    ),
  getStrategyDecisions: (limit = 100, offset = 0) =>
    apiFetch<OffsetPage<StrategyDecision>>(
      `/api/strategy-decisions?limit=${limit}&offset=${offset}`
    ),
  getRawLogs: (limit = 100, beforeId?: number) =>
    apiFetch<CursorPage<RawLogEntry>>(
      beforeId
        ? `/api/raw-logs?limit=${limit}&before_id=${beforeId}`
        : `/api/raw-logs?limit=${limit}`
    ),
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
