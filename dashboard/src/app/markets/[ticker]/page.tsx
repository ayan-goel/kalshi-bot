"use client";

import { useParams } from "next/navigation";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useMarketDetail } from "@/lib/hooks";
import { OrdersTable } from "@/components/orders-table";

export default function MarketDetailPage() {
  const params = useParams();
  const ticker = params.ticker as string;
  const { data, isLoading, error } = useMarketDetail(ticker);

  if (isLoading) return <p className="text-muted-foreground">Loading...</p>;
  if (error)
    return (
      <p className="text-red-600">
        Error: {error instanceof Error ? error.message : "Unknown"}
      </p>
    );
  if (!data) return <p className="text-muted-foreground">Market not found</p>;

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold font-mono">{ticker}</h2>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <InfoCard title="Mid" value={data.mid ? `$${data.mid}` : "—"} />
        <InfoCard title="Spread" value={data.spread ? `$${data.spread}` : "—"} />
        <InfoCard
          title="Best Bid"
          value={data.best_bid ? `$${data.best_bid}` : "—"}
          color="text-green-600"
        />
        <InfoCard
          title="Best Ask"
          value={data.best_ask ? `$${data.best_ask}` : "—"}
          color="text-red-500"
        />
        <InfoCard
          title="Microprice"
          value={data.microprice ? `$${data.microprice}` : "—"}
        />
        {data.position && (
          <>
            <InfoCard title="Net Inventory" value={data.position.net_inventory} />
            <InfoCard
              title="Yes Contracts"
              value={data.position.yes_contracts}
            />
            <InfoCard
              title="Realized PnL"
              value={`$${data.position.realized_pnl}`}
            />
          </>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Open Orders</CardTitle>
        </CardHeader>
        <CardContent>
          <OrdersTable orders={data.open_orders} />
        </CardContent>
      </Card>
    </div>
  );
}

function InfoCard({
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
        <p className={`text-lg font-bold font-mono mt-1 ${color ?? ""}`}>
          {value}
        </p>
      </CardContent>
    </Card>
  );
}
