"use client";

import { SettingsForm } from "@/components/settings-form";

export default function SettingsPage() {
  return (
    <div className="space-y-6 max-w-[1000px]">
      <div>
        <h2 className="text-xl font-semibold text-zinc-100 tracking-tight">
          Settings
        </h2>
        <p className="text-sm text-zinc-500 mt-0.5">
          Configure strategy parameters, risk limits, and trading behavior
        </p>
      </div>
      <SettingsForm />
    </div>
  );
}
