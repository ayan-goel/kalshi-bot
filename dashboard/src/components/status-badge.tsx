"use client";

import type { BotStatus } from "@/lib/types";
import { cn } from "@/lib/utils";

const stateConfig: Record<string, { bg: string; text: string; dot: string }> = {
  running: { bg: "bg-emerald-500/15", text: "text-emerald-400", dot: "bg-emerald-400" },
  stopped: { bg: "bg-zinc-500/15", text: "text-zinc-400", dot: "bg-zinc-500" },
  starting: { bg: "bg-amber-500/15", text: "text-amber-400", dot: "bg-amber-400" },
  stopping: { bg: "bg-amber-500/15", text: "text-amber-400", dot: "bg-amber-400" },
  error: { bg: "bg-red-500/15", text: "text-red-400", dot: "bg-red-400" },
  switching: { bg: "bg-blue-500/15", text: "text-blue-400", dot: "bg-blue-400" },
};

export function StatusBadge({ status }: { status: BotStatus | undefined }) {
  if (!status) {
    return (
      <div className="inline-flex items-center gap-1.5 rounded-md bg-zinc-800/50 px-2.5 py-1">
        <span className="h-1.5 w-1.5 rounded-full bg-zinc-600 animate-pulse" />
        <span className="text-[11px] font-medium text-zinc-500">
          Connecting...
        </span>
      </div>
    );
  }

  const config = stateConfig[status.state] ?? stateConfig.stopped;

  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md px-2.5 py-1",
        config.bg
      )}
    >
      <span
        className={cn(
          "h-1.5 w-1.5 rounded-full",
          config.dot,
          (status.state === "running" || status.state === "starting") && "animate-pulse"
        )}
      />
      <span className={cn("text-[11px] font-semibold uppercase tracking-wider", config.text)}>
        {status.state}
      </span>
    </div>
  );
}
