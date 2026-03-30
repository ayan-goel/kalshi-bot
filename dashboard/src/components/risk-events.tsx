"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import type { RiskEvent } from "@/lib/types";

const severityColors: Record<string, string> = {
  critical: "bg-red-600 text-white",
  warning: "bg-yellow-500 text-white",
  info: "bg-blue-500 text-white",
};

export function RiskEventsTable({ events }: { events: RiskEvent[] }) {
  if (!events.length) {
    return <p className="text-sm text-muted-foreground">No risk events</p>;
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Time</TableHead>
          <TableHead>Severity</TableHead>
          <TableHead>Component</TableHead>
          <TableHead>Market</TableHead>
          <TableHead>Message</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {events.map((e, i) => (
          <TableRow key={`${e.ts}-${i}`}>
            <TableCell className="text-xs">
              {new Date(e.ts).toLocaleString()}
            </TableCell>
            <TableCell>
              <Badge className={severityColors[e.severity] ?? "bg-zinc-400"}>
                {e.severity}
              </Badge>
            </TableCell>
            <TableCell>{e.component}</TableCell>
            <TableCell className="font-mono text-xs">
              {e.market_ticker ?? "—"}
            </TableCell>
            <TableCell className="max-w-[300px] truncate">{e.message}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
