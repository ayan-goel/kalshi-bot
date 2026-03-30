"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { OrdersTable } from "@/components/orders-table";
import { FillsTable } from "@/components/fills-table";
import { useOrders, useFills } from "@/lib/hooks";

export default function OrdersPage() {
  const { data: orders } = useOrders();
  const { data: fills } = useFills(200);

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Orders & Fills</h2>

      <Tabs defaultValue="orders">
        <TabsList>
          <TabsTrigger value="orders">
            Open Orders ({orders?.length ?? 0})
          </TabsTrigger>
          <TabsTrigger value="fills">
            Recent Fills ({fills?.length ?? 0})
          </TabsTrigger>
        </TabsList>

        <TabsContent value="orders">
          <Card>
            <CardHeader>
              <CardTitle>Open Orders</CardTitle>
            </CardHeader>
            <CardContent>
              <OrdersTable orders={orders ?? []} />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="fills">
          <Card>
            <CardHeader>
              <CardTitle>Recent Fills</CardTitle>
            </CardHeader>
            <CardContent>
              <FillsTable fills={fills ?? []} />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
