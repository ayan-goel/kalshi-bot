"use client";

import type { OrderInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

export function OrdersTable({ orders }: { orders: OrderInfo[] }) {
  if (!orders.length) {
    return <p className="text-sm text-zinc-600">No open orders</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-[#1e1e2e]">
            <Th>Market</Th>
            <Th>Side</Th>
            <Th>Action</Th>
            <Th align="right">Price</Th>
            <Th align="right">Remaining</Th>
            <Th>Status</Th>
          </tr>
        </thead>
        <tbody>
          {orders.map((o) => (
            <tr
              key={o.order_id}
              className="border-b border-[#1e1e2e]/50 hover:bg-white/[0.02] transition-colors"
            >
              <Td className="font-mono text-indigo-400">{o.market_ticker}</Td>
              <Td>
                <span
                  className={cn(
                    "text-xs font-semibold uppercase",
                    o.side === "yes" ? "text-emerald-400" : "text-red-400"
                  )}
                >
                  {o.side}
                </span>
              </Td>
              <Td className="text-zinc-400">{o.action}</Td>
              <Td align="right" className="font-mono text-zinc-200">
                ${o.price}
              </Td>
              <Td align="right" className="font-mono text-zinc-300">
                {o.remaining_count}
              </Td>
              <Td>
                <span className="text-xs font-medium text-zinc-500 bg-zinc-800 px-1.5 py-0.5 rounded">
                  {o.status ?? "resting"}
                </span>
              </Td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Th({
  children,
  align,
}: {
  children: React.ReactNode;
  align?: "left" | "right";
}) {
  return (
    <th
      className={cn(
        "py-2.5 px-3 text-[11px] font-semibold text-zinc-500 uppercase tracking-wider",
        align === "right" ? "text-right" : "text-left"
      )}
    >
      {children}
    </th>
  );
}

function Td({
  children,
  align,
  className,
}: {
  children: React.ReactNode;
  align?: "left" | "right";
  className?: string;
}) {
  return (
    <td
      className={cn(
        "py-2 px-3 text-xs",
        align === "right" ? "text-right" : "text-left",
        className
      )}
    >
      {children}
    </td>
  );
}
