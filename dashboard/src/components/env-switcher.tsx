"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useStatus } from "@/lib/hooks";
import { api } from "@/lib/api";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ArrowRightLeft } from "lucide-react";

export function EnvSwitcher() {
  const { data: status } = useStatus();
  const queryClient = useQueryClient();
  const [showConfirm, setShowConfirm] = useState(false);
  const [confirmText, setConfirmText] = useState("");

  const isDemo = status?.environment === "demo";

  const switchToDemo = async () => {
    try {
      await api.botStop().catch(() => {});
      await api.setEnvironment("demo");
      toast.success("Switched to demo environment");
      queryClient.invalidateQueries();
    } catch (e) {
      toast.error(`Switch failed: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  const switchToProduction = async () => {
    if (confirmText !== "CONFIRM") return;
    try {
      await api.botStop().catch(() => {});
      await api.setEnvironment("production", "CONFIRM");
      toast.success("Switched to production environment");
      queryClient.invalidateQueries();
      setShowConfirm(false);
      setConfirmText("");
    } catch (e) {
      toast.error(`Switch failed: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  return (
    <>
      <div className="flex items-center gap-3">
        <span className="text-xs font-medium text-zinc-500 uppercase tracking-wider">
          Environment
        </span>
        {isDemo ? (
          <>
            <span className="text-xs font-semibold text-blue-400 bg-blue-400/10 px-2 py-0.5 rounded">
              DEMO
            </span>
            <Button
              size="sm"
              onClick={() => setShowConfirm(true)}
              className="bg-red-600/15 hover:bg-red-600/25 text-red-400 border border-red-500/20 text-xs font-medium h-7 px-2.5"
            >
              <ArrowRightLeft className="mr-1.5 h-3 w-3" />
              Switch to Production
            </Button>
          </>
        ) : (
          <>
            <span className="text-xs font-semibold text-red-400 bg-red-400/10 px-2 py-0.5 rounded">
              PRODUCTION
            </span>
            <Button
              size="sm"
              onClick={switchToDemo}
              className="bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border border-zinc-700 text-xs font-medium h-7 px-2.5"
            >
              <ArrowRightLeft className="mr-1.5 h-3 w-3" />
              Switch to Demo
            </Button>
          </>
        )}
      </div>

      <Dialog open={showConfirm} onOpenChange={setShowConfirm}>
        <DialogContent className="bg-[#111118] border-[#1e1e2e]">
          <DialogHeader>
            <DialogTitle className="text-zinc-100">
              Switch to Production
            </DialogTitle>
            <DialogDescription className="text-zinc-500">
              You are about to switch to PRODUCTION mode. This will use real
              money. Type &quot;CONFIRM&quot; to proceed.
            </DialogDescription>
          </DialogHeader>
          <Input
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            placeholder='Type "CONFIRM"'
            className="bg-[#0a0a0f] border-[#1e1e2e] text-zinc-100 font-mono"
          />
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowConfirm(false)}
              className="bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border-zinc-700"
            >
              Cancel
            </Button>
            <Button
              disabled={confirmText !== "CONFIRM"}
              onClick={switchToProduction}
              className="bg-red-600 hover:bg-red-500 text-white"
            >
              Switch to Production
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
