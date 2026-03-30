"use client";

import { RiskEventsTable } from "@/components/risk-events";
import { useRiskEvents, useStrategyDecisions } from "@/lib/hooks";
import { useState } from "react";
import { cn } from "@/lib/utils";

export default function RiskPage() {
  const { data: riskEvents } = useRiskEvents(200);
  const { data: decisions } = useStrategyDecisions(200);
  const [tab, setTab] = useState<"risk" | "decisions">("risk");

  return (
    <div className="space-y-6 max-w-[1400px]">
      <div>
        <h2 className="text-xl font-semibold text-zinc-100 tracking-tight">
          Risk & Logs
        </h2>
        <p className="text-sm text-zinc-500 mt-0.5">
          Risk events and strategy decision history
        </p>
      </div>

      <div className="flex gap-1 bg-[#111118] rounded-lg border border-[#1e1e2e] p-1 w-fit">
        <TabButton
          active={tab === "risk"}
          onClick={() => setTab("risk")}
          count={riskEvents?.length ?? 0}
        >
          Risk Events
        </TabButton>
        <TabButton
          active={tab === "decisions"}
          onClick={() => setTab("decisions")}
          count={decisions?.length ?? 0}
        >
          Strategy Decisions
        </TabButton>
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        {tab === "risk" ? (
          <RiskEventsTable events={riskEvents ?? []} />
        ) : (
          <DecisionsTable decisions={decisions ?? []} />
        )}
      </div>
    </div>
  );
}

function DecisionsTable({
  decisions,
}: {
  decisions: Array<{
    ts: string;
    market_ticker: string;
    fair_value: string;
    inventory: string;
    reason: string;
  }>;
}) {
  if (!decisions.length) {
    return <p className="text-sm text-zinc-600">No strategy decisions yet</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-[#1e1e2e]">
            <Th>Time</Th>
            <Th>Market</Th>
            <Th align="right">Fair Value</Th>
            <Th align="right">Inventory</Th>
            <Th>Reason</Th>
          </tr>
        </thead>
        <tbody>
          {decisions.map((d, i) => (
            <tr
              key={`${d.ts}-${i}`}
              className="border-b border-[#1e1e2e]/50 hover:bg-white/[0.02] transition-colors"
            >
              <td className="py-2 px-3 text-xs text-zinc-500">
                {new Date(d.ts).toLocaleString()}
              </td>
              <td className="py-2 px-3 text-xs font-mono text-indigo-400">
                {d.market_ticker}
              </td>
              <td className="py-2 px-3 text-xs text-right font-mono text-zinc-200">
                ${d.fair_value}
              </td>
              <td className="py-2 px-3 text-xs text-right font-mono text-zinc-300">
                {d.inventory}
              </td>
              <td className="py-2 px-3 text-xs text-zinc-500 max-w-[300px] truncate">
                {d.reason}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
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
