"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useConfig } from "@/lib/hooks";
import { api } from "@/lib/api";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import type { StrategyConfig, RiskConfig, TradingConfig } from "@/lib/types";
import { Save } from "lucide-react";
import { cn } from "@/lib/utils";

function SectionCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-[#1e1e2e] bg-[#111118]">
      <div className="px-5 py-3.5 border-b border-[#1e1e2e]">
        <h3 className="text-sm font-semibold text-zinc-200">{title}</h3>
      </div>
      <div className="p-5">{children}</div>
    </div>
  );
}

function FormField({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-[11px] font-medium text-zinc-500 uppercase tracking-wider">
        {label}
      </Label>
      {children}
    </div>
  );
}

const inputClass =
  "bg-[#0a0a0f] border-[#1e1e2e] text-zinc-200 font-mono text-sm focus:border-indigo-500/50 focus:ring-indigo-500/20 h-9";

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
    { key: "inv_spread_scale", label: "Inv Spread Scale", type: "text" },
    { key: "inv_skew_scale", label: "Inv Skew Scale", type: "text" },
    { key: "vol_baseline_spread", label: "Vol Baseline Spread", type: "text" },
    { key: "expiry_widen_coeff", label: "Expiry Widen Coeff", type: "text" },
    { key: "expiry_widen_threshold_hours", label: "Expiry Widen Threshold (h)", type: "number" },
    { key: "event_half_spread_multiplier", label: "Event Spread Multiplier", type: "text" },
    { key: "event_threshold", label: "Event Threshold", type: "text" },
    { key: "event_decay_seconds", label: "Event Decay (s)", type: "number" },
  ];

  return (
    <SectionCard title="Strategy Parameters">
      <div className="grid grid-cols-2 gap-4">
        {fields.map((f) => (
          <FormField key={f.key} label={f.label}>
            <Input
              type={f.type}
              value={values[f.key]}
              onChange={(e) =>
                update(f.key, f.type === "number" ? Number(e.target.value) : e.target.value)
              }
              className={inputClass}
            />
          </FormField>
        ))}
      </div>
      <Button
        onClick={save}
        className="w-full mt-5 bg-indigo-600 hover:bg-indigo-500 text-white font-medium h-9"
      >
        <Save className="mr-2 h-3.5 w-3.5" />
        Save Strategy
      </Button>
    </SectionCard>
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
    <SectionCard title="Risk Limits">
      <div className="grid grid-cols-2 gap-4">
        <FormField label="Max Daily Loss">
          <Input value={values.max_loss_daily} onChange={(e) => update("max_loss_daily", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Max Market Notional">
          <Input value={values.max_market_notional} onChange={(e) => update("max_market_notional", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Max Inventory Contracts">
          <Input type="number" value={values.max_market_inventory_contracts} onChange={(e) => update("max_market_inventory_contracts", Number(e.target.value))} className={inputClass} />
        </FormField>
        <FormField label="Max Total Reserved">
          <Input value={values.max_total_reserved} onChange={(e) => update("max_total_reserved", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Max Open Orders">
          <Input type="number" value={values.max_open_orders} onChange={(e) => update("max_open_orders", Number(e.target.value))} className={inputClass} />
        </FormField>
        <FormField label="Max Capital Per Market">
          <Input value={values.max_capital_per_market} onChange={(e) => update("max_capital_per_market", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Max Portfolio Utilization">
          <Input value={values.max_portfolio_utilization} onChange={(e) => update("max_portfolio_utilization", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Max Fair Deviation">
          <Input value={values.max_fair_deviation} onChange={(e) => update("max_fair_deviation", e.target.value)} className={inputClass} />
        </FormField>
        <FormField label="Disconnect Timeout (s)">
          <Input type="number" value={values.disconnect_timeout_secs} onChange={(e) => update("disconnect_timeout_secs", Number(e.target.value))} className={inputClass} />
        </FormField>
        <FormField label="Seq Gap Timeout (s)">
          <Input type="number" value={values.seq_gap_timeout_secs} onChange={(e) => update("seq_gap_timeout_secs", Number(e.target.value))} className={inputClass} />
        </FormField>
        <div className="flex items-center gap-3 pt-6">
          <Switch
            checked={values.cancel_all_on_disconnect}
            onCheckedChange={(checked) => update("cancel_all_on_disconnect", checked)}
          />
          <Label className="text-xs text-zinc-400">Cancel All on Disconnect</Label>
        </div>
      </div>
      <Button
        onClick={save}
        className="w-full mt-5 bg-indigo-600 hover:bg-indigo-500 text-white font-medium h-9"
      >
        <Save className="mr-2 h-3.5 w-3.5" />
        Save Risk
      </Button>
    </SectionCard>
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
    <SectionCard title="Trading Configuration">
      <div className="space-y-4">
        <div className="flex items-center justify-between rounded-lg bg-[#0a0a0f] border border-[#1e1e2e] p-3">
          <div>
            <p className="text-sm font-medium text-zinc-200">Trading Enabled</p>
            <p className="text-[11px] text-zinc-500">
              {values.enabled ? "Bot will place real orders" : "Bot will not place orders"}
            </p>
          </div>
          <Switch
            checked={values.enabled}
            onCheckedChange={(checked) => update("enabled", checked)}
          />
        </div>
        <FormField label="Markets Allowlist (comma-separated)">
          <Input
            value={values.markets_allowlist.join(", ")}
            onChange={(e) =>
              update(
                "markets_allowlist",
                e.target.value.split(",").map((s) => s.trim()).filter(Boolean)
              )
            }
            className={inputClass}
          />
        </FormField>
        <FormField label="Categories Allowlist (comma-separated)">
          <Input
            value={values.categories_allowlist.join(", ")}
            onChange={(e) =>
              update(
                "categories_allowlist",
                e.target.value.split(",").map((s) => s.trim()).filter(Boolean)
              )
            }
            className={inputClass}
          />
        </FormField>
        <div className="grid grid-cols-2 gap-4">
          <FormField label="Max Open Orders">
            <Input type="number" value={values.max_open_orders} onChange={(e) => update("max_open_orders", Number(e.target.value))} className={inputClass} />
          </FormField>
          <FormField label="Max Markets Active">
            <Input type="number" value={values.max_markets_active} onChange={(e) => update("max_markets_active", Number(e.target.value))} className={inputClass} />
          </FormField>
          <FormField label="Rescan Interval (mins)">
            <Input type="number" value={values.market_rescan_interval_mins} onChange={(e) => update("market_rescan_interval_mins", Number(e.target.value))} className={inputClass} />
          </FormField>
          <FormField label="Min Expiry (hours)">
            <Input type="number" value={values.min_time_to_expiry_hours} onChange={(e) => update("min_time_to_expiry_hours", Number(e.target.value))} className={inputClass} />
          </FormField>
          <FormField label="Max Expiry (hours)">
            <Input type="number" value={values.max_time_to_expiry_hours} onChange={(e) => update("max_time_to_expiry_hours", Number(e.target.value))} className={inputClass} />
          </FormField>
          <FormField label="Min Volume 24h">
            <Input type="number" value={values.min_volume_24h} onChange={(e) => update("min_volume_24h", Number(e.target.value))} className={inputClass} />
          </FormField>
        </div>
      </div>
      <Button
        onClick={save}
        className="w-full mt-5 bg-indigo-600 hover:bg-indigo-500 text-white font-medium h-9"
      >
        <Save className="mr-2 h-3.5 w-3.5" />
        Save Trading
      </Button>
    </SectionCard>
  );
}

export function SettingsForm() {
  const { data } = useConfig();

  if (!data) {
    return (
      <div className="text-zinc-600 text-sm">Loading config...</div>
    );
  }

  return (
    <div className="space-y-6">
      <StrategyForm initial={data.strategy} />
      <RiskForm initial={data.risk} />
      <TradingForm initial={data.trading} />
    </div>
  );
}
