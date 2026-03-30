"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  LayoutDashboard,
  Settings,
  BarChart3,
  FileText,
  ShieldAlert,
} from "lucide-react";
import Image from "next/image";
import { StatusBadge } from "@/components/status-badge";
import { useStatus, useBotWebSocket } from "@/lib/hooks";
import { cn } from "@/lib/utils";

const navItems = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard },
  { href: "/markets", label: "Markets", icon: BarChart3 },
  { href: "/orders", label: "Orders & Fills", icon: FileText },
  { href: "/risk", label: "Risk & Logs", icon: ShieldAlert },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function SidebarNav() {
  const pathname = usePathname();
  const { data: status } = useStatus();
  useBotWebSocket();

  return (
    <aside className="w-60 border-r border-[#1e1e2e] bg-[#0d0d14] flex flex-col min-h-screen">
      <div className="px-5 py-5 border-b border-[#1e1e2e]">
        <div className="flex items-center gap-2.5">
          <Image
            src="/kalshi-mark.svg"
            alt="Kalshi"
            width={32}
            height={32}
            className="rounded-lg"
          />
          <div>
            <h1 className="font-semibold text-sm text-zinc-100 tracking-tight">
              Kalshi Bot
            </h1>
            <p className="text-[11px] text-zinc-500 font-medium">
              Trading Terminal
            </p>
          </div>
        </div>
        <div className="mt-3.5">
          <StatusBadge status={status} />
        </div>
      </div>

      <nav className="flex-1 px-3 py-3 space-y-0.5">
        {navItems.map((item) => {
          const isActive =
            item.href === "/"
              ? pathname === "/"
              : pathname.startsWith(item.href);
          return (
            <Link
              key={item.href}
              href={item.href}
              className={cn(
                "flex items-center gap-2.5 rounded-lg px-3 py-2 text-[13px] font-medium transition-all",
                isActive
                  ? "bg-indigo-500/15 text-indigo-400"
                  : "text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.03]"
              )}
            >
              <item.icon className={cn("h-4 w-4", isActive ? "text-indigo-400" : "text-zinc-600")} />
              {item.label}
            </Link>
          );
        })}
      </nav>

      <div className="px-5 py-4 border-t border-[#1e1e2e]">
        <div className="flex items-center justify-between">
          {status?.environment === "production" ? (
            <div className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-red-500 animate-pulse" />
              <span className="text-xs font-semibold text-red-400 tracking-wide uppercase">
                Production
              </span>
            </div>
          ) : (
            <div className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-blue-500" />
              <span className="text-xs font-medium text-blue-400 tracking-wide uppercase">
                Demo
              </span>
            </div>
          )}
          {status?.uptime_secs != null && (
            <span className="text-[11px] text-zinc-600 font-mono">
              {Math.floor(status.uptime_secs / 3600)}h{" "}
              {Math.floor((status.uptime_secs % 3600) / 60)}m
            </span>
          )}
        </div>
      </div>
    </aside>
  );
}
