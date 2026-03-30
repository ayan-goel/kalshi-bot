"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { RiskEventsTable } from "@/components/risk-events";
import { useRiskEvents, useStrategyDecisions } from "@/lib/hooks";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export default function RiskPage() {
  const { data: riskEvents } = useRiskEvents(200);
  const { data: decisions } = useStrategyDecisions(200);

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Risk & Logs</h2>

      <Tabs defaultValue="risk">
        <TabsList>
          <TabsTrigger value="risk">
            Risk Events ({riskEvents?.length ?? 0})
          </TabsTrigger>
          <TabsTrigger value="decisions">
            Strategy Decisions ({decisions?.length ?? 0})
          </TabsTrigger>
        </TabsList>

        <TabsContent value="risk">
          <Card>
            <CardHeader>
              <CardTitle>Risk Events</CardTitle>
            </CardHeader>
            <CardContent>
              <RiskEventsTable events={riskEvents ?? []} />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="decisions">
          <Card>
            <CardHeader>
              <CardTitle>Strategy Decisions</CardTitle>
            </CardHeader>
            <CardContent>
              {decisions && decisions.length > 0 ? (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Time</TableHead>
                      <TableHead>Market</TableHead>
                      <TableHead className="text-right">Fair Value</TableHead>
                      <TableHead className="text-right">Inventory</TableHead>
                      <TableHead>Reason</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {decisions.map((d, i) => (
                      <TableRow key={`${d.ts}-${i}`}>
                        <TableCell className="text-xs">
                          {new Date(d.ts).toLocaleString()}
                        </TableCell>
                        <TableCell className="font-mono text-xs">
                          {d.market_ticker}
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          ${d.fair_value}
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {d.inventory}
                        </TableCell>
                        <TableCell className="max-w-[300px] truncate text-xs">
                          {d.reason}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              ) : (
                <p className="text-sm text-muted-foreground">
                  No strategy decisions yet
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
