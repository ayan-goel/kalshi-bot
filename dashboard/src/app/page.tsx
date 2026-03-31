"use client";

import { BotControls } from "@/components/bot-controls";
import { EnvSwitcher } from "@/components/env-switcher";
import { PnlChart } from "@/components/pnl-chart";
import { FillsTable } from "@/components/fills-table";
import { useStatus, useBalance, usePnl, useFills } from "@/lib/hooks";
import {
  DollarSign,
  Layers,
  TrendingUp,
  CalendarDays,
  ShoppingCart,
  BarChart3,
  Lock,
  Wifi,
  Power,
} from "lucide-react";
import { cn } from "@/lib/utils";

export default function DashboardPage() {
  const { data: status } = useStatus();
  const { data: balance } = useBalance();
  const { data: pnl } = usePnl();
  const { data: fills } = useFills(10);

  const available = balance ? parseFloat(balance.available) : 0;
  const portfolioValue = balance ? parseFloat(balance.portfolio_value) : 0;

  const sessionPnl = pnl ? parseFloat(pnl.session.pnl) : 0;
  const sessionRealized = pnl ? parseFloat(pnl.session.realized_pnl) : 0;
  const sessionUnrealized = pnl ? parseFloat(pnl.session.unrealized_pnl) : 0;

  const dailyPnl = pnl ? parseFloat(pnl.daily.pnl) : 0;
  const dailyRealized = pnl ? parseFloat(pnl.daily.realized_pnl) : 0;
  const dailyUnrealized = pnl ? parseFloat(pnl.daily.unrealized_pnl) : 0;

  const fmt = (v: number) => `$${v.toFixed(2)}`;
  const sign = (v: number) => (v >= 0 ? "+" : "");

  return (
    <div className="space-y-6 max-w-[1400px]">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-zinc-100 tracking-tight">
            Dashboard
          </h2>
          <p className="text-sm text-zinc-500 mt-0.5">
            Real-time overview of your trading bot
          </p>
        </div>
        <BotControls />
      </div>

      <EnvSwitcher />

      {/* Row 1: Cash | Position Value | Session PnL | Daily PnL | Open Orders */}
      <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
        <StatCard
          label="Cash"
          value={balance ? fmt(available) : "—"}
          subValue="Available balance"
          icon={DollarSign}
          iconColor="text-emerald-400"
          iconBg="bg-emerald-400/10"
        />
        <StatCard
          label="Position Value"
          value={balance ? fmt(portfolioValue) : "—"}
          subValue="Mark-to-market"
          icon={Layers}
          iconColor="text-blue-400"
          iconBg="bg-blue-400/10"
        />
        <StatCard
          label="Session PnL"
          value={pnl ? `${sign(sessionPnl)}${fmt(sessionPnl)}` : "—"}
          subValue={
            pnl
              ? `${sign(sessionRealized)}${fmt(sessionRealized)} realized · ${sign(sessionUnrealized)}${fmt(sessionUnrealized)} unrealized`
              : undefined
          }
          icon={TrendingUp}
          iconColor={sessionPnl >= 0 ? "text-emerald-400" : "text-red-400"}
          iconBg={sessionPnl >= 0 ? "bg-emerald-400/10" : "bg-red-400/10"}
          valueColor={sessionPnl >= 0 ? "text-emerald-400" : "text-red-400"}
        />
        <StatCard
          label="Daily PnL"
          value={pnl ? `${sign(dailyPnl)}${fmt(dailyPnl)}` : "—"}
          subValue={
            pnl
              ? `${sign(dailyRealized)}${fmt(dailyRealized)} realized · ${sign(dailyUnrealized)}${fmt(dailyUnrealized)} unrealized`
              : undefined
          }
          icon={CalendarDays}
          iconColor={dailyPnl >= 0 ? "text-emerald-400" : "text-red-400"}
          iconBg={dailyPnl >= 0 ? "bg-emerald-400/10" : "bg-red-400/10"}
          valueColor={dailyPnl >= 0 ? "text-emerald-400" : "text-red-400"}
        />
        <StatCard
          label="Open Orders"
          value={status?.open_orders?.toString() ?? "—"}
          icon={ShoppingCart}
          iconColor="text-amber-400"
          iconBg="bg-amber-400/10"
        />
      </div>

      {/* Row 2: Active Markets | Reserved | Connectivity | Trading */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <StatCard
          label="Active Markets"
          value={status?.active_markets?.toString() ?? "—"}
          icon={BarChart3}
          iconColor="text-violet-400"
          iconBg="bg-violet-400/10"
        />
        <StatCard
          label="Reserved"
          value={balance ? `$${parseFloat(balance.total_reserved).toFixed(2)}` : "—"}
          icon={Lock}
          iconColor="text-zinc-400"
          iconBg="bg-zinc-400/10"
        />
        <StatCard
          label="Connectivity"
          value={status?.connectivity ?? "—"}
          icon={Wifi}
          iconColor="text-cyan-400"
          iconBg="bg-cyan-400/10"
        />
        <StatCard
          label="Trading"
          value={status?.trading_enabled ? "ENABLED" : "DISABLED"}
          icon={Power}
          iconColor={status?.trading_enabled ? "text-emerald-400" : "text-zinc-500"}
          iconBg={status?.trading_enabled ? "bg-emerald-400/10" : "bg-zinc-500/10"}
          valueColor={status?.trading_enabled ? "text-emerald-400" : "text-zinc-500"}
        />
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        <h3 className="text-sm font-medium text-zinc-400 mb-4">PnL Over Time</h3>
        <PnlChart />
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        <h3 className="text-sm font-medium text-zinc-400 mb-4">Recent Fills</h3>
        <FillsTable fills={fills ?? []} compact />
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  subValue,
  icon: Icon,
  iconColor,
  iconBg,
  valueColor,
}: {
  label: string;
  value: string;
  subValue?: string;
  icon: React.ComponentType<{ className?: string }>;
  iconColor: string;
  iconBg: string;
  valueColor?: string;
}) {
  return (
    <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-4 flex items-start gap-3">
      <div className={cn("rounded-lg p-2 shrink-0", iconBg)}>
        <Icon className={cn("h-4 w-4", iconColor)} />
      </div>
      <div className="min-w-0">
        <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-wider">
          {label}
        </p>
        <p
          className={cn(
            "text-lg font-semibold font-mono mt-0.5 truncate",
            valueColor ?? "text-zinc-100"
          )}
        >
          {value}
        </p>
        {subValue && (
          <p className="text-[10px] text-zinc-600 font-mono mt-0.5 truncate">
            {subValue}
          </p>
        )}
      </div>
    </div>
  );
}
