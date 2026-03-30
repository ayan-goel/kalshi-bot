"use client";

import { useState } from "react";
import { usePnl } from "@/lib/hooks";
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

type Timeframe = "30m" | "1h" | "4h" | "1d" | "all";

const TIMEFRAME_OPTIONS: { label: string; value: Timeframe; ms: number | null }[] = [
  { label: "30m", value: "30m", ms: 30 * 60 * 1000 },
  { label: "1h",  value: "1h",  ms: 60 * 60 * 1000 },
  { label: "4h",  value: "4h",  ms: 4 * 60 * 60 * 1000 },
  { label: "1d",  value: "1d",  ms: 24 * 60 * 60 * 1000 },
  { label: "All", value: "all", ms: null },
];

export function PnlChart() {
  const { data } = usePnl();
  const [timeframe, setTimeframe] = useState<Timeframe>("1h");

  if (!data?.snapshots?.length) {
    return (
      <div className="flex h-[280px] items-center justify-center text-zinc-600 text-sm">
        No PnL data yet
      </div>
    );
  }

  // Reverse so oldest is left, newest is right
  const ordered = [...data.snapshots].reverse();

  // Filter to selected window
  const windowMs = TIMEFRAME_OPTIONS.find((o) => o.value === timeframe)?.ms ?? null;
  const cutoff = windowMs ? new Date(Date.now() - windowMs) : null;
  const filtered = cutoff
    ? ordered.filter((s) => new Date(s.ts) >= cutoff)
    : ordered;

  const chartData = filtered.map((s) => {
    const equity = parseFloat(s.balance) + parseFloat(s.portfolio_value);
    // PnL = realized fill cash flow + unrealized position mark-to-market.
    // Both fields are correctly populated per-snapshot by the backend (post Bug 8/18 fix).
    const pnl = parseFloat(s.realized_pnl) + parseFloat(s.unrealized_pnl);
    return {
      time: new Date(s.ts).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
      }),
      equity: parseFloat(equity.toFixed(4)),
      pnl: parseFloat(pnl.toFixed(4)),
      cash: parseFloat(parseFloat(s.balance).toFixed(4)),
    };
  });

  // Compute reasonable Y-axis tick count based on data density
  const xTickInterval =
    chartData.length <= 10 ? 0 :
    chartData.length <= 30 ? Math.floor(chartData.length / 6) :
    Math.floor(chartData.length / 8);

  return (
    <div>
      {/* Timeframe selector */}
      <div className="flex items-center gap-1 mb-4">
        {TIMEFRAME_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setTimeframe(opt.value)}
            className={cn(
              "px-2.5 py-1 rounded text-xs font-medium transition-colors",
              timeframe === opt.value
                ? "bg-zinc-700 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800"
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {chartData.length === 0 ? (
        <div className="flex h-[280px] items-center justify-center text-zinc-600 text-sm">
          No data in the selected window
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={280}>
          <LineChart data={chartData}>
            <CartesianGrid stroke="#1e1e2e" strokeDasharray="none" />
            <XAxis
              dataKey="time"
              tick={{ fontSize: 11, fill: "#52525b" }}
              axisLine={{ stroke: "#1e1e2e" }}
              tickLine={{ stroke: "#1e1e2e" }}
              interval={xTickInterval}
            />
            <YAxis
              tick={{ fontSize: 11, fill: "#52525b" }}
              axisLine={{ stroke: "#1e1e2e" }}
              tickLine={{ stroke: "#1e1e2e" }}
              tickFormatter={(v) => `$${(v as number).toFixed(2)}`}
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
              formatter={(value, name) => [
                typeof value === "number" ? `$${value.toFixed(4)}` : String(value),
                name,
              ]}
            />
            <Legend wrapperStyle={{ fontSize: "12px", color: "#71717a" }} />
            <Line
              type="monotone"
              dataKey="equity"
              stroke="#f59e0b"
              name="Total Equity"
              dot={false}
              strokeWidth={2}
            />
            <Line
              type="monotone"
              dataKey="pnl"
              stroke="#22c55e"
              name="Realized + Unrealized PnL"
              dot={false}
              strokeWidth={1.5}
              strokeDasharray="4 4"
            />
            <Line
              type="monotone"
              dataKey="cash"
              stroke="#6366f1"
              name="Cash"
              dot={false}
              strokeWidth={1}
              opacity={0.6}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}
