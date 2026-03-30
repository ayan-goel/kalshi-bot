"use client";

import { Button } from "@/components/ui/button";
import { useStatus } from "@/lib/hooks";
import { api } from "@/lib/api";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Play, Square, Zap } from "lucide-react";

export function BotControls() {
  const { data: status } = useStatus();
  const queryClient = useQueryClient();

  const state = status?.state ?? "stopped";

  const handleStart = async () => {
    try {
      await api.botStart();
      toast.success("Bot starting...");
      queryClient.invalidateQueries({ queryKey: ["status"] });
    } catch (e) {
      toast.error(`Failed to start: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  const handleStop = async () => {
    try {
      await api.botStop();
      toast.success("Bot stopping...");
      queryClient.invalidateQueries({ queryKey: ["status"] });
    } catch (e) {
      toast.error(`Failed to stop: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  const handleKill = async () => {
    try {
      await api.botKill();
      toast.warning("Kill switch activated");
      queryClient.invalidateQueries({ queryKey: ["status"] });
    } catch (e) {
      toast.error(`Kill failed: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  return (
    <div className="flex gap-2">
      <Button
        onClick={handleStart}
        disabled={state !== "stopped"}
        size="sm"
        className="bg-emerald-600 hover:bg-emerald-500 text-white border-0 text-xs font-medium h-8 px-3"
      >
        <Play className="mr-1.5 h-3.5 w-3.5" />
        Start
      </Button>
      <Button
        onClick={handleStop}
        disabled={state !== "running" && state !== "error"}
        size="sm"
        className="bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border border-zinc-700 text-xs font-medium h-8 px-3"
      >
        <Square className="mr-1.5 h-3.5 w-3.5" />
        Stop
      </Button>
      <Button
        onClick={handleKill}
        size="sm"
        className="bg-red-600/20 hover:bg-red-600/30 text-red-400 border border-red-500/30 text-xs font-medium h-8 px-3"
      >
        <Zap className="mr-1.5 h-3.5 w-3.5" />
        Kill
      </Button>
    </div>
  );
}
