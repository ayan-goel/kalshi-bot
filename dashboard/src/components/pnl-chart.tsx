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
      <div className="flex h-[300px] items-center justify-center text-muted-foreground">
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
    <ResponsiveContainer width="100%" height={300}>
      <LineChart data={chartData}>
        <CartesianGrid strokeDasharray="3 3" className="opacity-30" />
        <XAxis
          dataKey="time"
          tick={{ fontSize: 11 }}
          interval="preserveStartEnd"
        />
        <YAxis tick={{ fontSize: 11 }} />
        <Tooltip />
        <Legend />
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
          stroke="#3b82f6"
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
          strokeWidth={2}
          strokeDasharray="5 5"
        />
      </LineChart>
    </ResponsiveContainer>
  );
}
