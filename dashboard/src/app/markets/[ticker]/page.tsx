"use client";

import { useParams } from "next/navigation";
import { useMarketDetail } from "@/lib/hooks";
import { OrdersTable } from "@/components/orders-table";
import { ArrowLeft } from "lucide-react";
import Link from "next/link";
import { cn } from "@/lib/utils";

export default function MarketDetailPage() {
  const params = useParams();
  const ticker = params.ticker as string;
  const { data, isLoading, error } = useMarketDetail(ticker);

  if (isLoading)
    return <p className="text-zinc-600 text-sm animate-pulse">Loading...</p>;
  if (error)
    return (
      <p className="text-red-400 text-sm">
        Error: {error instanceof Error ? error.message : "Unknown"}
      </p>
    );
  if (!data) return <p className="text-zinc-600 text-sm">Market not found</p>;

  return (
    <div className="space-y-6 max-w-[1400px]">
      <div className="flex items-center gap-3">
        <Link
          href="/markets"
          className="h-8 w-8 rounded-lg bg-[#111118] border border-[#1e1e2e] flex items-center justify-center hover:border-zinc-600 transition-colors"
        >
          <ArrowLeft className="h-4 w-4 text-zinc-400" />
        </Link>
        <div>
          <h2 className="text-xl font-semibold font-mono text-indigo-400 tracking-tight">
            {ticker}
          </h2>
          <p className="text-xs text-zinc-500">Market detail</p>
        </div>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <InfoCard label="Mid" value={data.mid ? `$${data.mid}` : "—"} />
        <InfoCard label="Spread" value={data.spread ? `$${data.spread}` : "—"} />
        <InfoCard
          label="Best Bid"
          value={data.best_bid ? `$${data.best_bid}` : "—"}
          valueColor="text-emerald-400"
        />
        <InfoCard
          label="Best Ask"
          value={data.best_ask ? `$${data.best_ask}` : "—"}
          valueColor="text-red-400"
        />
        <InfoCard
          label="Microprice"
          value={data.microprice ? `$${data.microprice}` : "—"}
        />
        {data.position && (
          <>
            <InfoCard label="Net Inventory" value={data.position.net_inventory} />
            <InfoCard label="Yes Contracts" value={data.position.yes_contracts} />
            <InfoCard
              label="Realized PnL"
              value={`$${data.position.realized_pnl}`}
              valueColor={
                parseFloat(data.position.realized_pnl) >= 0
                  ? "text-emerald-400"
                  : "text-red-400"
              }
            />
          </>
        )}
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        <h3 className="text-sm font-medium text-zinc-400 mb-4">Open Orders</h3>
        <OrdersTable orders={data.open_orders} />
      </div>
    </div>
  );
}

function InfoCard({
  label,
  value,
  valueColor,
}: {
  label: string;
  value: string;
  valueColor?: string;
}) {
  return (
    <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-4">
      <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-wider">
        {label}
      </p>
      <p
        className={cn(
          "text-lg font-semibold font-mono mt-1",
          valueColor ?? "text-zinc-100"
        )}
      >
        {value}
      </p>
    </div>
  );
}
