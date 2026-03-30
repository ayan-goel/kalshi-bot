"use client";

import { Badge } from "@/components/ui/badge";
import type { BotStatus } from "@/lib/types";

const stateColors: Record<string, string> = {
  running: "bg-green-500 text-white",
  stopped: "bg-zinc-400 text-white",
  starting: "bg-yellow-500 text-white",
  stopping: "bg-yellow-500 text-white",
  error: "bg-red-600 text-white",
  switching: "bg-blue-500 text-white",
};

export function StatusBadge({ status }: { status: BotStatus | undefined }) {
  if (!status) {
    return <Badge variant="outline">Loading...</Badge>;
  }

  return (
    <Badge className={stateColors[status.state] ?? "bg-zinc-400 text-white"}>
      {status.state.toUpperCase()}
    </Badge>
  );
}
