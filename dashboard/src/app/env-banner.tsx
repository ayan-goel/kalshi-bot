"use client";

import { useStatus } from "@/lib/hooks";
import { AlertTriangle } from "lucide-react";

export function EnvBannerClient() {
  const { data: status } = useStatus();

  if (!status || status.environment === "demo") return null;

  return (
    <div className="bg-red-500/10 border-b border-red-500/20 text-red-400 text-center py-1.5 text-xs font-semibold tracking-widest uppercase flex items-center justify-center gap-2">
      <AlertTriangle className="h-3.5 w-3.5" />
      Production Mode — Real Money
      <AlertTriangle className="h-3.5 w-3.5" />
    </div>
  );
}
