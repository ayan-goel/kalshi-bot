import type { PnlData, PnlSnapshot } from "./types";

export type PnlMode = "session" | "daily";

export interface PnlChartPoint {
  timestamp: number;
  cash: number;
  equity: number;
  sessionPnl: number;
  dailyPnl: number;
}

export interface PnlSummary {
  sessionPnl: number;
  sessionRealized: number;
  sessionUnrealized: number;
  dailyPnl: number;
  dailyRealized: number;
  dailyUnrealized: number;
}

function toNumber(value: unknown): number {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : 0;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

export function extractPnlSummary(pnl: PnlData | null | undefined): PnlSummary {
  return {
    sessionPnl: toNumber(pnl?.session?.pnl),
    sessionRealized: toNumber(pnl?.session?.realized_pnl),
    sessionUnrealized: toNumber(pnl?.session?.unrealized_pnl),
    dailyPnl: toNumber(pnl?.daily?.pnl),
    dailyRealized: toNumber(pnl?.daily?.realized_pnl),
    dailyUnrealized: toNumber(pnl?.daily?.unrealized_pnl),
  };
}

export function mapSnapshotsToChartPoints(
  snapshots: PnlSnapshot[]
): PnlChartPoint[] {
  return [...snapshots].reverse().map((s) => ({
    timestamp: new Date(s.ts).getTime(),
    cash: parseFloat(s.balance),
    equity: parseFloat(s.equity),
    sessionPnl: parseFloat(s.session_pnl),
    dailyPnl: parseFloat(s.daily_pnl),
  }));
}

export function pointPnlValue(point: PnlChartPoint, mode: PnlMode): number {
  return mode === "session" ? point.sessionPnl : point.dailyPnl;
}

export function latestPnlValue(
  points: PnlChartPoint[],
  mode: PnlMode
): number {
  if (points.length === 0) return 0;
  return pointPnlValue(points[points.length - 1], mode);
}
