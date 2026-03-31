import type { PnlSnapshot } from "./types";

export type PnlMode = "session" | "daily";

export interface PnlChartPoint {
  timestamp: number;
  cash: number;
  equity: number;
  sessionPnl: number;
  dailyPnl: number;
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
