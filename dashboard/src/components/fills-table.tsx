"use client";

import type { FillInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

export function FillsTable({ fills, compact }: { fills: FillInfo[]; compact?: boolean }) {
  if (!fills.length) {
    return <p className="text-sm text-zinc-600">No fills yet</p>;
  }

  const displayed = compact ? fills.slice(0, 10) : fills;

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-[#1e1e2e]">
            <Th>Time</Th>
            <Th>Market</Th>
            <Th>Side</Th>
            <Th>Action</Th>
            <Th align="right">Price</Th>
            <Th align="right">Qty</Th>
            <Th align="right">Fee</Th>
            {!compact && <Th>Taker</Th>}
          </tr>
        </thead>
        <tbody>
          {displayed.map((f) => (
            <tr
              key={f.fill_id}
              className="border-b border-[#1e1e2e]/50 hover:bg-white/[0.02] transition-colors"
            >
              <Td className="text-zinc-500">
                {new Date(f.fill_ts).toLocaleTimeString()}
              </Td>
              <Td className="font-mono text-indigo-400">{f.market_ticker}</Td>
              <Td>
                <span
                  className={cn(
                    "text-xs font-semibold uppercase",
                    f.side === "yes" ? "text-emerald-400" : "text-red-400"
                  )}
                >
                  {f.side}
                </span>
              </Td>
              <Td className="text-zinc-400">{f.action}</Td>
              <Td align="right" className="font-mono text-zinc-200">
                ${f.price}
              </Td>
              <Td align="right" className="font-mono text-zinc-300">
                {f.quantity}
              </Td>
              <Td align="right" className="font-mono text-zinc-500">
                ${f.fee}
              </Td>
              {!compact && (
                <Td className="text-zinc-500">{f.is_taker ? "Yes" : "No"}</Td>
              )}
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
