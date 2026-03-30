"use client";

import type { RiskEvent } from "@/lib/types";
import { cn } from "@/lib/utils";

const severityConfig: Record<string, { bg: string; text: string }> = {
  critical: { bg: "bg-red-500/15", text: "text-red-400" },
  warning: { bg: "bg-amber-500/15", text: "text-amber-400" },
  info: { bg: "bg-blue-500/15", text: "text-blue-400" },
};

export function RiskEventsTable({ events }: { events: RiskEvent[] }) {
  if (!events.length) {
    return <p className="text-sm text-zinc-600">No risk events</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-[#1e1e2e]">
            <Th>Time</Th>
            <Th>Severity</Th>
            <Th>Component</Th>
            <Th>Market</Th>
            <Th>Message</Th>
          </tr>
        </thead>
        <tbody>
          {events.map((e, i) => {
            const sev = severityConfig[e.severity] ?? severityConfig.info;
            return (
              <tr
                key={`${e.ts}-${i}`}
                className="border-b border-[#1e1e2e]/50 hover:bg-white/[0.02] transition-colors"
              >
                <Td className="text-zinc-500">
                  {new Date(e.ts).toLocaleString()}
                </Td>
                <Td>
                  <span
                    className={cn(
                      "text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded",
                      sev.bg,
                      sev.text
                    )}
                  >
                    {e.severity}
                  </span>
                </Td>
                <Td className="text-zinc-400">{e.component}</Td>
                <Td className="font-mono text-indigo-400">
                  {e.market_ticker ?? "—"}
                </Td>
                <Td className="text-zinc-400 max-w-[300px] truncate">
                  {e.message}
                </Td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th className="py-2.5 px-3 text-[11px] font-semibold text-zinc-500 uppercase tracking-wider text-left">
      {children}
    </th>
  );
}

function Td({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <td className={cn("py-2 px-3 text-xs", className)}>{children}</td>
  );
}
