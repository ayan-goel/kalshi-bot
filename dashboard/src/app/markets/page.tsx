"use client";

import { useMarkets } from "@/lib/hooks";
import { MarketCard } from "@/components/market-card";
import { BarChart3 } from "lucide-react";

export default function MarketsPage() {
  const { data: markets, isLoading } = useMarkets();

  return (
    <div className="space-y-6 max-w-[1400px]">
      <div>
        <h2 className="text-xl font-semibold text-zinc-100 tracking-tight">
          Markets
        </h2>
        <p className="text-sm text-zinc-500 mt-0.5">
          Active markets being monitored by the bot
        </p>
      </div>

      {isLoading && (
        <div className="text-sm text-zinc-600 animate-pulse">Loading markets...</div>
      )}

      {markets && markets.length === 0 && (
        <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-12 text-center">
          <BarChart3 className="h-8 w-8 text-zinc-700 mx-auto mb-3" />
          <p className="text-sm text-zinc-500">
            No active markets. Start the bot to begin monitoring.
          </p>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {markets?.map((m) => (
          <MarketCard key={m.ticker} market={m} />
        ))}
      </div>
    </div>
  );
}
