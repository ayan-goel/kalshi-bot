"use client";

import { RiskEventsTable } from "@/components/risk-events";
import { useRawLogs, useRiskEvents, useStrategyDecisions } from "@/lib/hooks";
import type { RawLogEntry } from "@/lib/types";
import { useState } from "react";
import { cn } from "@/lib/utils";

export default function RiskPage() {
  const [tab, setTab] = useState<"risk" | "decisions" | "raw">("risk");
  const [pageSize, setPageSize] = useState(100);
  const [riskPage, setRiskPage] = useState(0);
  const [decisionsPage, setDecisionsPage] = useState(0);
  const [rawBeforeId, setRawBeforeId] = useState<number | undefined>(undefined);

  const riskOffset = riskPage * pageSize;
  const decisionsOffset = decisionsPage * pageSize;

  const { data: riskEventsPage, isFetching: riskLoading } = useRiskEvents(
    pageSize,
    riskOffset
  );
  const { data: decisionsPageData, isFetching: decisionsLoading } =
    useStrategyDecisions(pageSize, decisionsOffset);
  const { data: rawLogsPage, isFetching: rawLogsLoading } = useRawLogs(
    pageSize,
    rawBeforeId
  );

  const riskEvents = riskEventsPage?.items ?? [];
  const decisions = decisionsPageData?.items ?? [];
  const rawLogs = rawLogsPage?.items ?? [];

  const onPageSizeChange = (value: number) => {
    const next = Number.isFinite(value) ? Math.min(Math.max(value, 10), 1000) : 100;
    setPageSize(next);
    setRiskPage(0);
    setDecisionsPage(0);
    setRawBeforeId(undefined);
  };

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

      <div className="flex flex-wrap items-center gap-3">
        <div className="flex gap-1 bg-[#111118] rounded-lg border border-[#1e1e2e] p-1 w-fit">
          <TabButton
            active={tab === "risk"}
            onClick={() => setTab("risk")}
            count={riskEvents.length}
          >
            Risk Events
          </TabButton>
          <TabButton
            active={tab === "decisions"}
            onClick={() => setTab("decisions")}
            count={decisions.length}
          >
            Strategy Decisions
          </TabButton>
          <TabButton
            active={tab === "raw"}
            onClick={() => setTab("raw")}
            count={rawLogs.length}
          >
            Raw Logs
          </TabButton>
        </div>

        <label className="text-xs text-zinc-500 flex items-center gap-2">
          Page Size
          <input
            type="number"
            min={10}
            max={1000}
            value={pageSize}
            onChange={(e) => onPageSizeChange(Number(e.target.value))}
            className="w-20 h-8 rounded-md border border-[#2b2b3f] bg-[#0d0d14] px-2 font-mono text-xs text-zinc-200 outline-none focus:ring-2 focus:ring-indigo-500/50"
          />
        </label>
      </div>

      <div className="rounded-xl border border-[#1e1e2e] bg-[#111118] p-5">
        {tab === "risk" && (
          <>
            <RiskEventsTable events={riskEvents} />
            <OffsetPagination
              page={riskPage}
              hasPrev={riskPage > 0}
              hasNext={Boolean(riskEventsPage?.next_offset)}
              loading={riskLoading}
              onPrev={() => setRiskPage((p) => Math.max(0, p - 1))}
              onNext={() => setRiskPage((p) => p + 1)}
            />
          </>
        )}

        {tab === "decisions" && (
          <>
            <DecisionsTable decisions={decisions} />
            <OffsetPagination
              page={decisionsPage}
              hasPrev={decisionsPage > 0}
              hasNext={Boolean(decisionsPageData?.next_offset)}
              loading={decisionsLoading}
              onPrev={() => setDecisionsPage((p) => Math.max(0, p - 1))}
              onNext={() => setDecisionsPage((p) => p + 1)}
            />
          </>
        )}

        {tab === "raw" && (
          <>
            <RawLogsTable logs={rawLogs} />
            <CursorPagination
              viewingLatest={!rawBeforeId}
              hasOlder={Boolean(rawLogsPage?.next_before_id)}
              loading={rawLogsLoading}
              onNewest={() => setRawBeforeId(undefined)}
              onOlder={() => setRawBeforeId(rawLogsPage?.next_before_id ?? undefined)}
            />
          </>
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

function OffsetPagination({
  page,
  hasPrev,
  hasNext,
  loading,
  onPrev,
  onNext,
}: {
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  loading: boolean;
  onPrev: () => void;
  onNext: () => void;
}) {
  return (
    <div className="mt-4 pt-4 border-t border-[#1e1e2e] flex items-center justify-between">
      <span className="text-xs text-zinc-500">
        Page {page + 1}
        {loading ? " • refreshing..." : ""}
      </span>
      <div className="flex gap-2">
        <button
          onClick={onPrev}
          disabled={!hasPrev}
          className="h-8 px-3 rounded-md border border-[#2b2b3f] bg-[#0d0d14] text-xs text-zinc-300 disabled:opacity-40 disabled:cursor-not-allowed hover:border-indigo-500/50 transition-colors"
        >
          Previous
        </button>
        <button
          onClick={onNext}
          disabled={!hasNext}
          className="h-8 px-3 rounded-md border border-[#2b2b3f] bg-[#0d0d14] text-xs text-zinc-300 disabled:opacity-40 disabled:cursor-not-allowed hover:border-indigo-500/50 transition-colors"
        >
          Next
        </button>
      </div>
    </div>
  );
}

function CursorPagination({
  viewingLatest,
  hasOlder,
  loading,
  onNewest,
  onOlder,
}: {
  viewingLatest: boolean;
  hasOlder: boolean;
  loading: boolean;
  onNewest: () => void;
  onOlder: () => void;
}) {
  return (
    <div className="mt-4 pt-4 border-t border-[#1e1e2e] flex items-center justify-between">
      <span className="text-xs text-zinc-500">
        {viewingLatest ? "Viewing latest logs" : "Viewing older logs"}
        {loading ? " • refreshing..." : ""}
      </span>
      <div className="flex gap-2">
        <button
          onClick={onNewest}
          disabled={viewingLatest}
          className="h-8 px-3 rounded-md border border-[#2b2b3f] bg-[#0d0d14] text-xs text-zinc-300 disabled:opacity-40 disabled:cursor-not-allowed hover:border-indigo-500/50 transition-colors"
        >
          Newest
        </button>
        <button
          onClick={onOlder}
          disabled={!hasOlder}
          className="h-8 px-3 rounded-md border border-[#2b2b3f] bg-[#0d0d14] text-xs text-zinc-300 disabled:opacity-40 disabled:cursor-not-allowed hover:border-indigo-500/50 transition-colors"
        >
          Older
        </button>
      </div>
    </div>
  );
}

function RawLogsTable({ logs }: { logs: RawLogEntry[] }) {
  if (!logs.length) {
    return <p className="text-sm text-zinc-600">No logs yet</p>;
  }

  return (
    <div className="space-y-2">
      {logs.map((log) => (
        <div
          key={log.id}
          className="rounded-lg border border-[#1e1e2e] bg-[#0d0d14] p-3"
        >
          <div className="flex flex-wrap items-center gap-2 text-[11px]">
            <span className="font-mono text-zinc-500">
              {new Date(log.ts).toLocaleString()}
            </span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded font-semibold uppercase tracking-wide",
                log.level === "ERROR" && "bg-red-500/15 text-red-400",
                log.level === "WARN" && "bg-amber-500/15 text-amber-400",
                log.level === "INFO" && "bg-blue-500/15 text-blue-400",
                log.level === "DEBUG" && "bg-zinc-700/50 text-zinc-300",
                !["ERROR", "WARN", "INFO", "DEBUG"].includes(log.level) &&
                  "bg-zinc-700/50 text-zinc-300"
              )}
            >
              {log.level}
            </span>
            <span className="font-mono text-indigo-400">{log.target}</span>
          </div>
          <div className="mt-2 text-xs font-mono text-zinc-200 break-all">
            {log.message}
          </div>
          {Object.keys(log.fields ?? {}).length > 0 && (
            <pre className="mt-2 text-[11px] text-zinc-500 overflow-x-auto font-mono whitespace-pre-wrap break-all">
              {JSON.stringify(log.fields, null, 2)}
            </pre>
          )}
        </div>
      ))}
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
