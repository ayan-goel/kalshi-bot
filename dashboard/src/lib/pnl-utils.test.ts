import { describe, expect, it } from "vitest";
import {
  latestPnlValue,
  mapSnapshotsToChartPoints,
  pointPnlValue,
} from "./pnl-utils";
import type { PnlSnapshot } from "./types";

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

describe("pnl-utils", () => {
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
