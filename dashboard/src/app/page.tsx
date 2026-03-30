"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { BotControls } from "@/components/bot-controls";
import { EnvSwitcher } from "@/components/env-switcher";
import { PnlChart } from "@/components/pnl-chart";
import { FillsTable } from "@/components/fills-table";
import { useStatus, useBalance, usePnl, useFills } from "@/lib/hooks";

export default function DashboardPage() {
  const { data: status } = useStatus();
  const { data: balance } = useBalance();
  const { data: pnl } = usePnl();
  const { data: fills } = useFills(10);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">Dashboard</h2>
        <BotControls />
      </div>

      <EnvSwitcher />

      {/* Stats row */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard
          title="Balance"
          value={balance ? `$${balance.available}` : "—"}
        />
        <StatCard
          title="Portfolio Value"
          value={balance ? `$${balance.portfolio_value}` : "—"}
        />
        <StatCard
          title="Daily PnL"
          value={pnl ? `$${pnl.daily_realized_pnl}` : "—"}
          color={
            pnl && parseFloat(pnl.daily_realized_pnl) >= 0
              ? "text-green-600"
              : "text-red-600"
          }
        />
        <StatCard
          title="Open Orders"
          value={status?.open_orders?.toString() ?? "—"}
        />
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard
          title="Active Markets"
          value={status?.active_markets?.toString() ?? "—"}
        />
        <StatCard
          title="Reserved"
          value={balance ? `$${balance.total_reserved}` : "—"}
        />
        <StatCard
          title="Connectivity"
          value={status?.connectivity ?? "—"}
        />
        <StatCard
          title="Trading"
          value={status?.trading_enabled ? "ENABLED" : "DISABLED"}
          color={status?.trading_enabled ? "text-green-600" : "text-zinc-500"}
        />
      </div>

      {/* PnL Chart */}
      <Card>
        <CardHeader>
          <CardTitle>PnL Over Time</CardTitle>
        </CardHeader>
        <CardContent>
          <PnlChart />
        </CardContent>
      </Card>

      {/* Recent fills */}
      <Card>
        <CardHeader>
          <CardTitle>Recent Fills</CardTitle>
        </CardHeader>
        <CardContent>
          <FillsTable fills={fills ?? []} compact />
        </CardContent>
      </Card>
    </div>
  );
}

function StatCard({
  title,
  value,
  color,
}: {
  title: string;
  value: string;
  color?: string;
}) {
  return (
    <Card>
      <CardContent className="pt-4">
        <p className="text-xs text-muted-foreground uppercase tracking-wide">
          {title}
        </p>
        <p className={`text-xl font-bold font-mono mt-1 ${color ?? ""}`}>
          {value}
        </p>
      </CardContent>
    </Card>
  );
}
