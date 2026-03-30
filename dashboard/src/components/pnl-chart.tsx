"use client";

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

export function PnlChart() {
  const { data } = usePnl();

  if (!data?.snapshots?.length) {
    return (
      <div className="flex h-[280px] items-center justify-center text-zinc-600 text-sm">
        No PnL data yet
      </div>
    );
  }

  // Reverse so oldest is left, newest is right
  const ordered = [...data.snapshots].reverse();

  // Baseline = first snapshot's total equity
  const baseline =
    parseFloat(ordered[0]?.balance ?? "0") +
    parseFloat(ordered[0]?.portfolio_value ?? "0");

  const chartData = ordered.map((s) => {
    const equity = parseFloat(s.balance) + parseFloat(s.portfolio_value);
    return {
      time: new Date(s.ts).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
      }),
      equity: parseFloat(equity.toFixed(4)),
      pnl: parseFloat((equity - baseline).toFixed(4)),
      cash: parseFloat(parseFloat(s.balance).toFixed(4)),
      positions: parseFloat(parseFloat(s.portfolio_value).toFixed(4)),
    };
  });

  return (
    <ResponsiveContainer width="100%" height={280}>
      <LineChart data={chartData}>
        <CartesianGrid stroke="#1e1e2e" strokeDasharray="none" />
        <XAxis
          dataKey="time"
          tick={{ fontSize: 11, fill: "#52525b" }}
          axisLine={{ stroke: "#1e1e2e" }}
          tickLine={{ stroke: "#1e1e2e" }}
          interval="preserveStartEnd"
        />
        <YAxis
          tick={{ fontSize: 11, fill: "#52525b" }}
          axisLine={{ stroke: "#1e1e2e" }}
          tickLine={{ stroke: "#1e1e2e" }}
          tickFormatter={(v) => `$${v.toFixed(2)}`}
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
          name="Session PnL"
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
  );
}
