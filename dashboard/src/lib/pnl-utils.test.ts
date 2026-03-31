import { describe, expect, it } from "vitest";
import {
  extractPnlSummary,
  latestPnlValue,
  mapSnapshotsToChartPoints,
  pointPnlValue,
} from "./pnl-utils";
import type { PnlData, PnlSnapshot } from "./types";

const snapshotsDesc: PnlSnapshot[] = [
  {
    ts: "2026-03-30T20:10:00Z",
    realized_pnl: "1.25",
    unrealized_pnl: "0.50",
    balance: "99.00",
    portfolio_value: "2.00",
    equity: "101.00",
    session_pnl: "1.75",
    session_realized_pnl: "1.25",
    session_unrealized_pnl: "0.50",
    daily_pnl: "2.10",
    daily_realized_pnl: "1.60",
    daily_unrealized_pnl: "0.50",
    open_order_count: 3,
    active_market_count: 2,
  },
  {
    ts: "2026-03-30T20:09:00Z",
    realized_pnl: "1.00",
    unrealized_pnl: "0.40",
    balance: "98.50",
    portfolio_value: "1.90",
    equity: "100.40",
    session_pnl: "1.40",
    session_realized_pnl: "1.00",
    session_unrealized_pnl: "0.40",
    daily_pnl: "1.80",
    daily_realized_pnl: "1.40",
    daily_unrealized_pnl: "0.40",
    open_order_count: 2,
    active_market_count: 2,
  },
];

const nestedPnlData: PnlData = {
  window: "all",
  session_started_at: "2026-03-30T20:00:00Z",
  session: {
    pnl: "1.75",
    realized_pnl: "1.25",
    unrealized_pnl: "0.50",
  },
  daily: {
    pnl: "2.10",
    realized_pnl: "1.60",
    unrealized_pnl: "0.50",
  },
  components: {
    cash: "99.00",
    position_value: "2.00",
    equity: "101.00",
  },
  snapshots: snapshotsDesc,
};

describe("pnl-utils", () => {
  it("extracts summary from nested session/daily fields", () => {
    const summary = extractPnlSummary(nestedPnlData);
    expect(summary).toEqual({
      sessionPnl: 1.75,
      sessionRealized: 1.25,
      sessionUnrealized: 0.5,
      dailyPnl: 2.1,
      dailyRealized: 1.6,
      dailyUnrealized: 0.5,
    });
  });

  it("returns zeros for missing nested fields", () => {
    const incompletePayload = {
      window: "all",
      session_started_at: null,
      components: {
        cash: "99.00",
        position_value: "2.00",
        equity: "101.00",
      },
      snapshots: [],
    } as unknown as PnlData;

    const summary = extractPnlSummary(incompletePayload);
    expect(summary).toEqual({
      sessionPnl: 0,
      sessionRealized: 0,
      sessionUnrealized: 0,
      dailyPnl: 0,
      dailyRealized: 0,
      dailyUnrealized: 0,
    });
  });

  it("returns zeros for undefined payload", () => {
    expect(extractPnlSummary(undefined)).toEqual({
      sessionPnl: 0,
      sessionRealized: 0,
      sessionUnrealized: 0,
      dailyPnl: 0,
      dailyRealized: 0,
      dailyUnrealized: 0,
    });
  });

  it("maps snapshots in chronological order for charts", () => {
    const points = mapSnapshotsToChartPoints(snapshotsDesc);
    expect(points).toHaveLength(2);
    expect(points[0].timestamp).toBeLessThan(points[1].timestamp);
    expect(points[0].cash).toBe(98.5);
    expect(points[1].equity).toBe(101);
  });

  it("selects session or daily pnl per point", () => {
    const points = mapSnapshotsToChartPoints(snapshotsDesc);
    expect(pointPnlValue(points[1], "session")).toBe(1.75);
    expect(pointPnlValue(points[1], "daily")).toBe(2.1);
  });

  it("returns latest pnl safely for empty and non-empty series", () => {
    const points = mapSnapshotsToChartPoints(snapshotsDesc);
    expect(latestPnlValue([], "session")).toBe(0);
    expect(latestPnlValue(points, "session")).toBe(1.75);
    expect(latestPnlValue(points, "daily")).toBe(2.1);
  });
});
