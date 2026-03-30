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
        className="bg-green-600 hover:bg-green-700"
      >
        <Play className="mr-1 h-4 w-4" />
        Start
      </Button>
      <Button
        onClick={handleStop}
        disabled={state !== "running" && state !== "error"}
        size="sm"
        variant="secondary"
      >
        <Square className="mr-1 h-4 w-4" />
        Stop
      </Button>
      <Button
        onClick={handleKill}
        size="sm"
        variant="destructive"
      >
        <Zap className="mr-1 h-4 w-4" />
        Kill
      </Button>
    </div>
  );
}
