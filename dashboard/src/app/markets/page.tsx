"use client";

import { useMarkets } from "@/lib/hooks";
import { MarketCard } from "@/components/market-card";

export default function MarketsPage() {
  const { data: markets, isLoading } = useMarkets();

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Markets</h2>

      {isLoading && <p className="text-muted-foreground">Loading markets...</p>}

      {markets && markets.length === 0 && (
        <p className="text-muted-foreground">
          No active markets. Start the bot to begin monitoring.
        </p>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {markets?.map((m) => (
          <MarketCard key={m.ticker} market={m} />
        ))}
      </div>
    </div>
  );
}
