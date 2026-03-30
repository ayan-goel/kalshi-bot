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

  const chartData = [...data.snapshots].reverse().map((s) => ({
    time: new Date(s.ts).toLocaleTimeString(),
    realized: parseFloat(s.realized_pnl),
    unrealized: parseFloat(s.unrealized_pnl),
    total: parseFloat(s.realized_pnl) + parseFloat(s.unrealized_pnl),
    balance: parseFloat(s.balance),
  }));

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
        />
        <Tooltip
          contentStyle={{
            backgroundColor: "#111118",
            border: "1px solid #1e1e2e",
            borderRadius: "8px",
            color: "#e4e4e7",
            fontSize: "12px",
          }}
        />
        <Legend
          wrapperStyle={{ fontSize: "12px", color: "#71717a" }}
        />
        <Line
          type="monotone"
          dataKey="realized"
          stroke="#22c55e"
          name="Realized"
          dot={false}
          strokeWidth={2}
        />
        <Line
          type="monotone"
          dataKey="unrealized"
          stroke="#6366f1"
          name="Unrealized"
          dot={false}
          strokeWidth={2}
        />
        <Line
          type="monotone"
          dataKey="total"
          stroke="#f59e0b"
          name="Total"
          dot={false}
          strokeWidth={1.5}
          strokeDasharray="4 4"
        />
      </LineChart>
    </ResponsiveContainer>
  );
}
