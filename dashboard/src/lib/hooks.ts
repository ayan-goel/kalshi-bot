"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useCallback } from "react";
import { api } from "./api";
import type { WsMessage } from "./types";

export function useStatus() {
  return useQuery({
    queryKey: ["status"],
    queryFn: api.getStatus,
    refetchInterval: 5000,
  });
}

export function useBalance() {
  return useQuery({
    queryKey: ["balance"],
    queryFn: api.getBalance,
    refetchInterval: 5000,
  });
}

export function usePnl() {
  return useQuery({
    queryKey: ["pnl"],
    queryFn: api.getPnl,
    refetchInterval: 10000,
  });
}

export function useMarkets() {
  return useQuery({
    queryKey: ["markets"],
    queryFn: api.getMarkets,
    refetchInterval: 5000,
  });
}

export function useMarketDetail(ticker: string) {
  return useQuery({
    queryKey: ["market", ticker],
    queryFn: () => api.getMarketDetail(ticker),
    refetchInterval: 3000,
  });
}

export function useOrders() {
  return useQuery({
    queryKey: ["orders"],
    queryFn: api.getOrders,
    refetchInterval: 5000,
  });
}

export function usePositions() {
  return useQuery({
    queryKey: ["positions"],
    queryFn: api.getPositions,
    refetchInterval: 5000,
  });
}

export function useFills(limit = 100) {
  return useQuery({
    queryKey: ["fills", limit],
    queryFn: () => api.getFills(limit),
    refetchInterval: 10000,
  });
}

export function useRiskEvents(limit = 100) {
  return useQuery({
    queryKey: ["riskEvents", limit],
    queryFn: () => api.getRiskEvents(limit),
    refetchInterval: 10000,
  });
}

export function useStrategyDecisions(limit = 100) {
  return useQuery({
    queryKey: ["strategyDecisions", limit],
    queryFn: () => api.getStrategyDecisions(limit),
    refetchInterval: 10000,
  });
}

export function useConfig() {
  return useQuery({
    queryKey: ["config"],
    queryFn: api.getConfig,
  });
}

export function useEnvironment() {
  return useQuery({
    queryKey: ["environment"],
    queryFn: api.getEnvironment,
    refetchInterval: 10000,
  });
}

const WS_URL =
  typeof window !== "undefined"
    ? (process.env.NEXT_PUBLIC_BOT_WS_URL ||
        `ws://${window.location.hostname}:8080/api/ws`)
    : "";

export function useBotWebSocket() {
  const queryClient = useQueryClient();
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<NodeJS.Timeout | null>(null);

  const connect = useCallback(() => {
    if (!WS_URL) return;
    try {
      const ws = new WebSocket(WS_URL);
      wsRef.current = ws;

      ws.onmessage = (event) => {
        try {
          const msg: WsMessage = JSON.parse(event.data);
          switch (msg.type) {
            case "state_change":
              queryClient.invalidateQueries({ queryKey: ["status"] });
              break;
            case "pnl_tick":
              queryClient.invalidateQueries({ queryKey: ["balance"] });
              queryClient.invalidateQueries({ queryKey: ["pnl"] });
              break;
            case "fill":
              queryClient.invalidateQueries({ queryKey: ["fills"] });
              queryClient.invalidateQueries({ queryKey: ["positions"] });
              queryClient.invalidateQueries({ queryKey: ["balance"] });
              break;
            case "order_update":
              queryClient.invalidateQueries({ queryKey: ["orders"] });
              break;
            case "risk_event":
              queryClient.invalidateQueries({ queryKey: ["riskEvents"] });
              break;
            case "config_change":
              queryClient.invalidateQueries({ queryKey: ["config"] });
              break;
          }
        } catch {
          // ignore parse errors
        }
      };

      ws.onclose = () => {
        reconnectTimer.current = setTimeout(connect, 3000);
      };

      ws.onerror = () => {
        ws.close();
      };
    } catch {
      reconnectTimer.current = setTimeout(connect, 3000);
    }
  }, [queryClient]);

  useEffect(() => {
    connect();
    return () => {
      wsRef.current?.close();
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
    };
  }, [connect]);
}
