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
    <aside className="w-56 border-r bg-muted/30 flex flex-col min-h-screen">
      <div className="p-4 border-b">
        <h1 className="font-bold text-lg">Kalshi Bot</h1>
        <div className="mt-2">
          <StatusBadge status={status} />
        </div>
      </div>
      <nav className="flex-1 p-2 space-y-1">
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
                "flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground"
              )}
            >
              <item.icon className="h-4 w-4" />
              {item.label}
            </Link>
          );
        })}
      </nav>
      <div className="p-4 border-t text-xs text-muted-foreground">
        {status?.environment === "production" ? (
          <span className="text-red-600 font-bold">PRODUCTION</span>
        ) : (
          <span className="text-blue-600">DEMO</span>
        )}
        {status?.uptime_secs != null && (
          <span className="ml-2">
            Up {Math.floor(status.uptime_secs / 60)}m
          </span>
        )}
      </div>
    </aside>
  );
}
