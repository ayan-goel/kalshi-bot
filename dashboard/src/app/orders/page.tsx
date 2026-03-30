"use client";

import { OrdersTable } from "@/components/orders-table";
import { FillsTable } from "@/components/fills-table";
import { useOrders, useFills } from "@/lib/hooks";
import { useState } from "react";
import { cn } from "@/lib/utils";

export default function OrdersPage() {
  const { data: orders } = useOrders();
  const { data: fills } = useFills(200);
  const [tab, setTab] = useState<"orders" | "fills">("orders");

  return (
    <div className="space-y-6 max-w-[1400px]">
      <div>
        <h2 className="text-xl font-semibold text-zinc-100 tracking-tight">
          Orders & Fills
        </h2>
        <p className="text-sm text-zinc-500 mt-0.5">
          Live order book and fill history
        </p>
      </div>

      <div className="flex gap-1 bg-[#111118] rounded-lg border border-[#1e1e2e] p-1 w-fit">
        <TabButton
          active={tab === "orders"}
          onClick={() => setTab("orders")}
          count={orders?.length ?? 0}
        >
          Open Orders
        </TabButton>
        <TabButton
          active={tab === "fills"}
          onClick={() => setTab("fills")}
          count={fills?.length ?? 0}
        >
          Recent Fills
        </TabButton>
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        {tab === "orders" ? (
          <OrdersTable orders={orders ?? []} />
        ) : (
          <FillsTable fills={fills ?? []} />
        )}
      </div>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  count,
  children,
}: {
  active: boolean;
  onClick: () => void;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "px-3 py-1.5 rounded-md text-xs font-medium transition-colors flex items-center gap-2",
        active
          ? "bg-indigo-500/15 text-indigo-400"
          : "text-zinc-500 hover:text-zinc-300"
      )}
    >
      {children}
      <span
        className={cn(
          "text-[10px] font-mono px-1.5 py-0.5 rounded",
          active ? "bg-indigo-500/20 text-indigo-400" : "bg-zinc-800 text-zinc-500"
        )}
      >
        {count}
      </span>
    </button>
  );
}
