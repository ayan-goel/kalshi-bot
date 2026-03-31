"use client";

import { useMemo, useState } from "react";
import { usePnl } from "@/lib/hooks";
import type { PnlWindow } from "@/lib/types";
import {
  latestPnlValue,
  mapSnapshotsToChartPoints,
  type PnlMode,
} from "@/lib/pnl-utils";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from "recharts";
import { cn } from "@/lib/utils";
type SeriesKey = "pnl" | "cash" | "equity";

const WINDOW_OPTIONS: { label: string; value: PnlWindow }[] = [
  { label: "30m", value: "30m" },
  { label: "1h", value: "1h" },
  { label: "4h", value: "4h" },
  { label: "1d", value: "1d" },
  { label: "All", value: "all" },
];

const MODE_OPTIONS: { label: string; value: PnlMode }[] = [
  { label: "Session", value: "session" },
  { label: "Daily", value: "daily" },
];

const SERIES_OPTIONS: { label: string; key: SeriesKey }[] = [
  { label: "PnL", key: "pnl" },
  { label: "Cash", key: "cash" },
  { label: "Equity", key: "equity" },
];

function fmtUsd(value: number): string {
  return `$${value.toFixed(2)}`;
}

function fmtXAxis(ts: number, window: PnlWindow): string {
  const d = new Date(ts);
  if (window === "1d" || window === "all") {
    return d.toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function PnlChart() {
  const [mode, setMode] = useState<PnlMode>("session");
  const [window, setWindow] = useState<PnlWindow>("1h");
  const [visible, setVisible] = useState<Record<SeriesKey, boolean>>({
    pnl: true,
    cash: true,
    equity: true,
  });
  const { data } = usePnl(window);

  const chartData = useMemo(() => {
    if (!data?.snapshots?.length) return [];
    return mapSnapshotsToChartPoints(data.snapshots);
  }, [data]);

  const pnlLabel = mode === "session" ? "Session PnL" : "Daily PnL";
  const latestPnl = latestPnlValue(chartData, mode);

  const toggleSeries = (series: SeriesKey) => {
    setVisible((prev) => ({ ...prev, [series]: !prev[series] }));
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {MODE_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setMode(opt.value)}
            className={cn(
              "px-2.5 py-1 rounded text-xs font-medium transition-colors",
              mode === opt.value
                ? "bg-zinc-700 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800"
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-1">
        {WINDOW_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setWindow(opt.value)}
            className={cn(
              "px-2.5 py-1 rounded text-xs font-medium transition-colors",
              window === opt.value
                ? "bg-zinc-700 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800"
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {SERIES_OPTIONS.map((series) => (
          <button
            key={series.key}
            onClick={() => toggleSeries(series.key)}
            className={cn(
              "px-2 py-1 rounded border text-[11px] font-medium transition-colors",
              visible[series.key]
                ? "border-zinc-600 bg-zinc-800 text-zinc-200"
                : "border-zinc-800 text-zinc-500 hover:text-zinc-300 hover:border-zinc-700"
            )}
          >
            {series.label}
          </button>
        ))}
      </div>

      {chartData.length === 0 ? (
        <div className="flex h-[280px] items-center justify-center text-zinc-600 text-sm">
          No data in the selected window
        </div>
      ) : (
        <>
          <p
            className={cn(
              "text-xs font-mono",
              latestPnl >= 0 ? "text-emerald-400" : "text-red-400"
            )}
          >
            {pnlLabel}: {latestPnl >= 0 ? "+" : ""}
            {fmtUsd(latestPnl)}
          </p>

          <ResponsiveContainer width="100%" height={300}>
            <LineChart data={chartData}>
              <CartesianGrid stroke="#1e1e2e" strokeDasharray="none" />
              <XAxis
                dataKey="timestamp"
                type="number"
                domain={["dataMin", "dataMax"]}
                tick={{ fontSize: 11, fill: "#52525b" }}
                axisLine={{ stroke: "#1e1e2e" }}
                tickLine={{ stroke: "#1e1e2e" }}
                tickFormatter={(value) => fmtXAxis(Number(value), window)}
                interval="preserveStartEnd"
                minTickGap={26}
              />
              <YAxis
                tick={{ fontSize: 11, fill: "#52525b" }}
                axisLine={{ stroke: "#1e1e2e" }}
                tickLine={{ stroke: "#1e1e2e" }}
                tickFormatter={(v) => fmtUsd(Number(v))}
                domain={["auto", "auto"]}
                allowDataOverflow={false}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: "#111118",
                  border: "1px solid #1e1e2e",
                  borderRadius: "8px",
                  color: "#e4e4e7",
                  fontSize: "12px",
                }}
                labelFormatter={(value) => fmtXAxis(Number(value), window)}
                formatter={(value, name) => [
                  typeof value === "number" ? fmtUsd(value) : String(value),
                  name,
                ]}
              />
              <Legend wrapperStyle={{ fontSize: "12px", color: "#71717a" }} />
              {visible.equity && (
                <Line
                  type="monotone"
                  dataKey="equity"
                  stroke="#f59e0b"
                  name="Total Equity"
                  dot={false}
                  strokeWidth={2}
                />
              )}
              {visible.pnl && (
                <Line
                  type="monotone"
                  dataKey={mode === "session" ? "sessionPnl" : "dailyPnl"}
                  stroke="#22c55e"
                  name={pnlLabel}
                  dot={false}
                  strokeWidth={1.8}
                  strokeDasharray="4 4"
                />
              )}
              {visible.cash && (
                <Line
                  type="monotone"
                  dataKey="cash"
                  stroke="#6366f1"
                  name="Cash"
                  dot={false}
                  strokeWidth={1}
                  opacity={0.65}
                />
              )}
            </LineChart>
          </ResponsiveContainer>
        </>
      )}
    </div>
  );
}
