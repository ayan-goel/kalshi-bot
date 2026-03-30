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
      <div className="flex items-center gap-2">
        <span className="text-sm text-muted-foreground">Environment:</span>
        {isDemo ? (
          <>
            <span className="font-medium text-blue-600">DEMO</span>
            <Button size="sm" variant="destructive" onClick={() => setShowConfirm(true)}>
              Switch to Production
            </Button>
          </>
        ) : (
          <>
            <span className="font-bold text-red-600">PRODUCTION</span>
            <Button size="sm" variant="outline" onClick={switchToDemo}>
              Switch to Demo
            </Button>
          </>
        )}
      </div>

      <Dialog open={showConfirm} onOpenChange={setShowConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Switch to Production</DialogTitle>
            <DialogDescription>
              You are about to switch to PRODUCTION mode. This will use real
              money. Type &quot;CONFIRM&quot; to proceed.
            </DialogDescription>
          </DialogHeader>
          <Input
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            placeholder='Type "CONFIRM"'
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowConfirm(false)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              disabled={confirmText !== "CONFIRM"}
              onClick={switchToProduction}
            >
              Switch to Production
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
