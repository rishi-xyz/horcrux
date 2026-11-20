"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  BadgeCheck,
  Bot,
  FileKey2,
  LayoutDashboard,
  QrCode,
  RefreshCw,
  ScrollText,
  Settings,
  ShieldCheck,
  Signature,
  Split,
  Wallet,
  RadioTower,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { api, getToken, hasToken } from "@/lib/api";
import type { StatusResponse } from "@/lib/types";
import { Button } from "@/components/ui/button";

const NAV = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard },
  { href: "/access-log", label: "Access Log", icon: ScrollText },
  { href: "/verify", label: "Verify", icon: BadgeCheck },
  { href: "/init", label: "Init (SSS)", icon: Split },
  { href: "/sign", label: "Sign (Mode A)", icon: Signature },
  { href: "/mpc", label: "MPC (FROST)", icon: ShieldCheck },
  { href: "/reconstruct", label: "Reconstruct", icon: RefreshCw },
  { href: "/qr", label: "QR Air-Gap", icon: QrCode },
  { href: "/wallets", label: "Wallets", icon: Wallet },
  { href: "/onchain", label: "On-Chain", icon: RadioTower },
  { href: "/ai", label: "AI & Security", icon: Bot },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [statusErr, setStatusErr] = useState<string | null>(null);
  const tokenPresent = useMemo(() => hasToken(), []);

  const load = async () => {
    try {
      setStatusErr(null);
      const s = await api.get<StatusResponse>("/status");
      setStatus(s);
    } catch (e: any) {
      setStatusErr(e?.message || "Failed to reach backend");
    }
  };

  useEffect(() => {
    if (tokenPresent) load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tokenPresent]);

  return (
    <div className="min-h-screen bg-bg bg-noise text-text">
      <div className="flex min-h-screen">
        <aside className="w-64 shrink-0 border-r border-border bg-surface/40 backdrop-blur-md flex flex-col sticky top-0 h-screen">
          <div className="p-4 border-b border-border">
            <div className="flex items-center gap-2">
              <div className="h-8 w-8 rounded-lg bg-accent/15 border border-accent/30 flex items-center justify-center">
                <FileKey2 className="h-4 w-4 text-accent" />
              </div>
              <div>
                <div className="font-semibold tracking-tight leading-none">Horcrux</div>
                <div className="text-[11px] text-muted mt-1">Local Console · 127.0.0.1</div>
              </div>
            </div>
          </div>
          <nav className="flex-1 overflow-y-auto p-2 space-y-0.5">
            {NAV.map((item) => {
              const Icon = item.icon;
              const active = item.href === "/" ? pathname === "/" : pathname?.startsWith(item.href);
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors border border-transparent",
                    active
                      ? "bg-accent/12 text-accent border-accent/20"
                      : "text-muted hover:text-text hover:bg-surface-2"
                  )}
                >
                  <Icon className="h-4 w-4" />
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
          <div className="p-3 border-t border-border space-y-2 text-xs">
            <div className={cn("rounded-md px-2.5 py-2 border", tokenPresent ? "border-ok/30 bg-ok/10 text-ok" : "border-danger/30 bg-danger/10 text-danger")}>
              {tokenPresent ? "Token loaded from URL" : "Missing token — actions will 401"}
            </div>
            <div className="text-muted leading-relaxed">
              Paths are read from this machine by the backend. No private keys or seeds are returned to the UI.
            </div>
            {status && (
              <div className="text-faint font-mono">v{status.version}</div>
            )}
          </div>
        </aside>

        <div className="flex-1 min-w-0 flex flex-col">
          <header className="h-14 border-b border-border bg-bg/70 backdrop-blur-md sticky top-0 z-20 px-5 flex items-center justify-between gap-4">
            <div className="flex items-center gap-3 min-w-0">
              <div className="text-sm text-muted truncate">
                {NAV.find((n) => (n.href === "/" ? pathname === "/" : pathname?.startsWith(n.href)))?.label || "Console"}
              </div>
              {status?.ai && (
                <div className={cn("hidden sm:inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs", status.ai.configured ? "border-accent/30 bg-accent/10 text-accent" : "border-border text-muted")}>
                  <Activity className="h-3.5 w-3.5" />
                  AI {status.ai.configured ? "advisory on" : "not configured"}
                </div>
              )}
              {statusErr && <div className="text-xs text-danger truncate">{statusErr}</div>}
            </div>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" onClick={load} disabled={!tokenPresent}>Refresh</Button>
            </div>
          </header>
          <main className="flex-1 p-5 md:p-7 max-w-7xl w-full mx-auto">{children}</main>
        </div>
      </div>
    </div>
  );
}
