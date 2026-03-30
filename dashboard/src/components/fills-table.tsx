"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { FillInfo } from "@/lib/types";

export function FillsTable({ fills, compact }: { fills: FillInfo[]; compact?: boolean }) {
  if (!fills.length) {
    return <p className="text-sm text-muted-foreground">No fills yet</p>;
  }

  const displayed = compact ? fills.slice(0, 10) : fills;

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Time</TableHead>
          <TableHead>Market</TableHead>
          <TableHead>Side</TableHead>
          <TableHead>Action</TableHead>
          <TableHead className="text-right">Price</TableHead>
          <TableHead className="text-right">Qty</TableHead>
          <TableHead className="text-right">Fee</TableHead>
          {!compact && <TableHead>Taker</TableHead>}
        </TableRow>
      </TableHeader>
      <TableBody>
        {displayed.map((f) => (
          <TableRow key={f.fill_id}>
            <TableCell className="text-xs">
              {new Date(f.fill_ts).toLocaleTimeString()}
            </TableCell>
            <TableCell className="font-mono text-xs">{f.market_ticker}</TableCell>
            <TableCell>{f.side}</TableCell>
            <TableCell>{f.action}</TableCell>
            <TableCell className="text-right font-mono">${f.price}</TableCell>
            <TableCell className="text-right">{f.quantity}</TableCell>
            <TableCell className="text-right font-mono">${f.fee}</TableCell>
            {!compact && <TableCell>{f.is_taker ? "Yes" : "No"}</TableCell>}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
