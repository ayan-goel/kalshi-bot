"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { MarketSummary } from "@/lib/types";
import Link from "next/link";

export function MarketCard({ market }: { market: MarketSummary }) {
  return (
    <Link href={`/markets/${market.ticker}`}>
      <Card className="transition-colors hover:bg-muted/50">
        <CardHeader className="pb-2">
          <CardTitle className="font-mono text-sm">{market.ticker}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-1 text-sm">
            <div>
              <span className="text-muted-foreground">Mid: </span>
              <span className="font-mono">
                {market.mid ? `$${market.mid}` : "—"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Spread: </span>
              <span className="font-mono">
                {market.spread ? `$${market.spread}` : "—"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Bid: </span>
              <span className="font-mono text-green-600">
                {market.best_bid ? `$${market.best_bid}` : "—"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Ask: </span>
              <span className="font-mono text-red-500">
                {market.best_ask ? `$${market.best_ask}` : "—"}
              </span>
            </div>
            {market.position && (
              <>
                <div>
                  <span className="text-muted-foreground">Inventory: </span>
                  <span className="font-mono">{market.position.net_inventory}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">PnL: </span>
                  <span className="font-mono">${market.position.realized_pnl}</span>
                </div>
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}
