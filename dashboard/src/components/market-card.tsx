"use client";

import type { MarketSummary } from "@/lib/types";
import Link from "next/link";
import { cn } from "@/lib/utils";

export function MarketCard({ market }: { market: MarketSummary }) {
  const makerFee = market.mid
    ? (0.0175 * parseFloat(market.mid) * (1 - parseFloat(market.mid))).toFixed(4)
    : null;
  const netInventory = market.position ? parseFloat(market.position.net_inventory) : 0;
  const inventoryValue = market.position ? market.position.net_inventory.toString() : "—";
  const pnlValue = market.position ? `$${market.position.realized_pnl}` : "—";
  const pnlValueClass =
    market.position == null
      ? undefined
      : parseFloat(market.position.realized_pnl) >= 0
        ? "text-emerald-400"
        : "text-red-400";

  return (
    <Link href={`/markets/${market.ticker}`} className="block h-full">
      <div className="h-full rounded-xl border border-[#1e1e2e] bg-[#111118] p-4 hover:border-indigo-500/30 hover:bg-[#13131d] transition-all group">
        <div className="flex items-center justify-between gap-2 mb-1">
          <span className="min-w-0 truncate font-mono text-sm font-semibold text-indigo-400 group-hover:text-indigo-300 transition-colors">
            {market.ticker}
          </span>
          <div className="flex items-center gap-1.5">
            {market.score != null && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-indigo-500/15 text-indigo-400">
                {market.score.toFixed(3)}
              </span>
            )}
            {market.position && (
              <span
                className={cn(
                  "text-[10px] font-bold uppercase px-1.5 py-0.5 rounded",
                  netInventory > 0
                    ? "bg-emerald-500/15 text-emerald-400"
                    : netInventory < 0
                      ? "bg-red-500/15 text-red-400"
                      : "bg-zinc-500/15 text-zinc-400"
                )}
              >
                {netInventory > 0 ? "LONG" : netInventory < 0 ? "SHORT" : "FLAT"}
              </span>
            )}
          </div>
        </div>

        <div className="mb-3 min-h-[14px]">
          {(market.category || market.event_ticker) && (
            <div className="flex items-center gap-2">
              {market.category && (
                <span className="min-w-0 truncate text-[10px] text-zinc-600">{market.category}</span>
              )}
              {market.event_ticker && (
                <span className="min-w-0 truncate text-[10px] text-zinc-700 font-mono">{market.event_ticker}</span>
              )}
            </div>
          )}
        </div>

        <div className="grid grid-cols-2 gap-x-4 gap-y-2">
          <DataRow label="Mid" value={market.mid ? `$${market.mid}` : "—"} />
          <DataRow label="Spread" value={market.spread ? `$${market.spread}` : "—"} />
          <DataRow
            label="Bid"
            value={market.best_bid ? `$${market.best_bid}` : "—"}
            valueClass="text-emerald-400"
          />
          <DataRow
            label="Ask"
            value={market.best_ask ? `$${market.best_ask}` : "—"}
            valueClass="text-red-400"
          />
          <DataRow
            label="Volume 24h"
            value={market.volume_24h != null ? market.volume_24h.toFixed(0) : "—"}
          />
          <DataRow
            label="Expiry"
            value={
              market.hours_to_expiry != null
                ? market.hours_to_expiry < 24
                  ? `${market.hours_to_expiry.toFixed(1)}h`
                  : `${(market.hours_to_expiry / 24).toFixed(1)}d`
                : "—"
            }
          />
          <DataRow
            label="Maker Fee"
            value={makerFee ? `$${makerFee}` : "—"}
            valueClass={makerFee ? "text-amber-400" : undefined}
          />
          <DataRow
            label="Inventory"
            value={inventoryValue}
          />
          <DataRow
            label="PnL"
            value={pnlValue}
            valueClass={pnlValueClass}
          />
        </div>
      </div>
    </Link>
  );
}

function DataRow({
  label,
  value,
  valueClass,
}: {
  label: string;
  value: string;
  valueClass?: string;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-[11px] text-zinc-600">{label}</span>
      <span className={cn("text-xs font-mono font-medium", valueClass ?? "text-zinc-300")}>
        {value}
      </span>
    </div>
  );
}
