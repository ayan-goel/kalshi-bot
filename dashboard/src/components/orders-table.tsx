"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { OrderInfo } from "@/lib/types";

export function OrdersTable({ orders }: { orders: OrderInfo[] }) {
  if (!orders.length) {
    return <p className="text-sm text-muted-foreground">No open orders</p>;
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Market</TableHead>
          <TableHead>Side</TableHead>
          <TableHead>Action</TableHead>
          <TableHead className="text-right">Price</TableHead>
          <TableHead className="text-right">Remaining</TableHead>
          <TableHead>Status</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {orders.map((o) => (
          <TableRow key={o.order_id}>
            <TableCell className="font-mono text-xs">{o.market_ticker}</TableCell>
            <TableCell>{o.side}</TableCell>
            <TableCell>{o.action}</TableCell>
            <TableCell className="text-right font-mono">${o.price}</TableCell>
            <TableCell className="text-right">{o.remaining_count}</TableCell>
            <TableCell>{o.status ?? "resting"}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
