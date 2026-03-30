"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useConfig } from "@/lib/hooks";
import { api } from "@/lib/api";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import type { StrategyConfig, RiskConfig, TradingConfig } from "@/lib/types";

function StrategyForm({ initial }: { initial: StrategyConfig }) {
  const [values, setValues] = useState(initial);
  const queryClient = useQueryClient();

  useEffect(() => setValues(initial), [initial]);

  const update = (key: keyof StrategyConfig, val: string | number) => {
    setValues((prev) => ({ ...prev, [key]: val }));
  };

  const save = async () => {
    try {
      await api.updateStrategy(values);
      toast.success("Strategy config updated");
      queryClient.invalidateQueries({ queryKey: ["config"] });
    } catch (e) {
      toast.error(`Save failed: ${e instanceof Error ? e.message : "Unknown"}`);
    }
  };

  const fields: { key: keyof StrategyConfig; label: string; type: "text" | "number" }[] = [
    { key: "base_half_spread", label: "Base Half Spread", type: "text" },
    { key: "min_edge_after_fees", label: "Min Edge After Fees", type: "text" },
    { key: "default_order_size", label: "Default Order Size", type: "number" },
    { key: "max_order_size", label: "Max Order Size", type: "number" },
    { key: "min_rest_ms", label: "Min Rest (ms)", type: "number" },
    { key: "repricing_threshold", label: "Repricing Threshold", type: "text" },
    { key: "inventory_skew_coeff", label: "Inventory Skew Coeff", type: "text" },
    { key: "volatility_widen_coeff", label: "Volatility Widen Coeff", type: "text" },
    { key: "tick_interval_ms", label: "Tick Interval (ms)", type: "number" },
    { key: "order_imbalance_alpha", label: "Order Imbalance Alpha", type: "text" },
    { key: "trade_sign_alpha", label: "Trade Sign Alpha", type: "text" },
    { key: "inventory_penalty_k1", label: "Inventory Penalty k1", type: "text" },
    { key: "inventory_penalty_k3", label: "Inventory Penalty k3", type: "text" },
  ];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Strategy</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          {fields.map((f) => (
            <div key={f.key}>
              <Label className="text-xs">{f.label}</Label>
              <Input
                type={f.type}
                value={values[f.key]}
                onChange={(e) =>
                  update(
                    f.key,
                    f.type === "number" ? Number(e.target.value) : e.target.value
                  )
                }
              />
            </div>
          ))}
        </div>
        <Button onClick={save} className="w-full">
          Save Strategy
        </Button>
      </CardContent>
    </Card>
  );
}

function RiskForm({ initial }: { initial: RiskConfig }) {
  const [values, setValues] = useState(initial);
  const queryClient = useQueryClient();

  useEffect(() => setValues(initial), [initial]);

  const update = (key: keyof RiskConfig, val: string | number | boolean) => {
    setValues((prev) => ({ ...prev, [key]: val }));
  };

  const save = async () => {
    try {
      await api.updateRisk(values);
      toast.success("Risk config updated");
      queryClient.invalidateQueries({ queryKey: ["config"] });
    } catch (e) {
      toast.error(`Save failed: ${e instanceof Error ? e.message : "Unknown"}`);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Risk</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label className="text-xs">Max Daily Loss</Label>
            <Input value={values.max_loss_daily} onChange={(e) => update("max_loss_daily", e.target.value)} />
          </div>
          <div>
            <Label className="text-xs">Max Market Notional</Label>
            <Input value={values.max_market_notional} onChange={(e) => update("max_market_notional", e.target.value)} />
          </div>
          <div>
            <Label className="text-xs">Max Inventory Contracts</Label>
            <Input type="number" value={values.max_market_inventory_contracts} onChange={(e) => update("max_market_inventory_contracts", Number(e.target.value))} />
          </div>
          <div>
            <Label className="text-xs">Max Total Reserved</Label>
            <Input value={values.max_total_reserved} onChange={(e) => update("max_total_reserved", e.target.value)} />
          </div>
          <div>
            <Label className="text-xs">Max Open Orders</Label>
            <Input type="number" value={values.max_open_orders} onChange={(e) => update("max_open_orders", Number(e.target.value))} />
          </div>
          <div>
            <Label className="text-xs">Disconnect Timeout (s)</Label>
            <Input type="number" value={values.disconnect_timeout_secs} onChange={(e) => update("disconnect_timeout_secs", Number(e.target.value))} />
          </div>
          <div>
            <Label className="text-xs">Seq Gap Timeout (s)</Label>
            <Input type="number" value={values.seq_gap_timeout_secs} onChange={(e) => update("seq_gap_timeout_secs", Number(e.target.value))} />
          </div>
          <div className="flex items-center gap-2 pt-5">
            <Switch
              checked={values.cancel_all_on_disconnect}
              onCheckedChange={(checked) => update("cancel_all_on_disconnect", checked)}
            />
            <Label className="text-xs">Cancel All on Disconnect</Label>
          </div>
        </div>
        <Button onClick={save} className="w-full">
          Save Risk
        </Button>
      </CardContent>
    </Card>
  );
}

function TradingForm({ initial }: { initial: TradingConfig }) {
  const [values, setValues] = useState(initial);
  const queryClient = useQueryClient();

  useEffect(() => setValues(initial), [initial]);

  const update = (key: keyof TradingConfig, val: string | number | boolean | string[]) => {
    setValues((prev) => ({ ...prev, [key]: val }));
  };

  const save = async () => {
    try {
      await api.updateTrading(values);
      toast.success("Trading config updated");
      queryClient.invalidateQueries({ queryKey: ["config"] });
    } catch (e) {
      toast.error(`Save failed: ${e instanceof Error ? e.message : "Unknown"}`);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Trading</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center gap-2">
          <Switch
            checked={values.enabled}
            onCheckedChange={(checked) => update("enabled", checked)}
          />
          <Label>Trading Enabled</Label>
        </div>
        <div>
          <Label className="text-xs">Markets Allowlist (comma-separated)</Label>
          <Input
            value={values.markets_allowlist.join(", ")}
            onChange={(e) =>
              update(
                "markets_allowlist",
                e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean)
              )
            }
          />
        </div>
        <div>
          <Label className="text-xs">Categories Allowlist (comma-separated)</Label>
          <Input
            value={values.categories_allowlist.join(", ")}
            onChange={(e) =>
              update(
                "categories_allowlist",
                e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean)
              )
            }
          />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label className="text-xs">Max Open Orders</Label>
            <Input type="number" value={values.max_open_orders} onChange={(e) => update("max_open_orders", Number(e.target.value))} />
          </div>
          <div>
            <Label className="text-xs">Max Markets Active</Label>
            <Input type="number" value={values.max_markets_active} onChange={(e) => update("max_markets_active", Number(e.target.value))} />
          </div>
        </div>
        <Button onClick={save} className="w-full">
          Save Trading
        </Button>
      </CardContent>
    </Card>
  );
}

export function SettingsForm() {
  const { data } = useConfig();

  if (!data) {
    return <div className="text-muted-foreground">Loading config...</div>;
  }

  return (
    <div className="space-y-6">
      <StrategyForm initial={data.strategy} />
      <RiskForm initial={data.risk} />
      <TradingForm initial={data.trading} />
    </div>
  );
}
