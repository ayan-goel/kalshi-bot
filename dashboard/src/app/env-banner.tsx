"use client";

import { useStatus } from "@/lib/hooks";

export function EnvBannerClient() {
  const { data: status } = useStatus();

  if (!status || status.environment === "demo") return null;

  return (
    <div className="bg-red-600 text-white text-center py-1 text-sm font-bold tracking-wide">
      PRODUCTION MODE — REAL MONEY
    </div>
  );
}
