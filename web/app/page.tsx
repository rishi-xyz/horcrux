"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { Activity, BadgeCheck, Bot, QrCode, ShieldCheck, Signature, Split, Wallet, RadioTower, ScrollText } from "lucide-react";
import { api, hasToken } from "@/lib/api";
import type { StatusResponse, LogEntry, AiEventRef, WalletProfile } from "@/lib/types";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export default function DashboardPage() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [events, setEvents] = useState<AiEventRef[]>([]);
  const [wallets, setWallets] = useState<WalletProfile[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hasToken()) return;
    (async () => {
      try {
        const [s, l, a, w] = await Promise.all([
          api.get<StatusResponse>("/status"),
          api.get<{ entries: LogEntry[] }>("/log?tail=12").catch(() => ({ entries: [] })),
          api.get<{ events: AiEventRef[] }>("/ai/events?tail=10").catch(() => ({ events: [] })),
          api.get<{ wallets: WalletProfile[] }>("/wallets").catch(() => ({ wallets: [] })),
        ]);
        setStatus(s); setEntries(l.entries || []); setEvents(a.events || []); setWallets(w.wallets || []);
      } catch (e: any) { setError(e?.message || "Failed to load dashboard"); }
    })();
  }, []);

  if (!hasToken()) {
    return (
      <Alert variant="danger">
        <AlertTitle>Missing access token</AlertTitle>
        <AlertDescription>
          Open this console via the URL printed by <code>horcrux web</code> (it includes <code>?token=...</code>). The backend requires <code>X-Horcrux-Token</code> for all <code>/api/*</code> actions.
        </AlertDescription>
      </Alert>
    );
  }

  const quick = [
    { href: "/init", label: "Create SSS Shards", icon: Split, desc: "Split a key into threshold shards (Mode A)" },
    { href: "/sign", label: "Sign Transaction", icon: Signature, desc: "Reconstruct & sign Solana/Bitcoin/Cosmos" },
    { href: "/mpc", label: "MPC (FROST)", icon: ShieldCheck, desc: "Split and threshold-sign without reconstruction" },
    { href: "/verify", label: "Verify Files", icon: BadgeCheck, desc: "Check shard/share integrity and consistency" },
    { href: "/wallets", label: "Wallets", icon: Wallet, desc: "Shard-backed, browser-connected, watch-only" },
    { href: "/onchain", label: "On-Chain Views", icon: RadioTower, desc: "Balances & history (Solana/Cosmos)" },
    { href: "/qr", label: "QR Air-Gap", icon: QrCode, desc: "Shard QR import/export and 5-step MPC flow" },
    { href: "/ai", label: "AI & Security", icon: Bot, desc: "Advisory checks, events, and configuration" },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Horcrux Console</h1>
        <p className="text-muted mt-1">Local, loopback-only key management with threshold security, deterministic audit, and advisory AI checks.</p>
      </div>

      {error && <Alert variant="danger"><AlertTitle>Dashboard error</AlertTitle><AlertDescription>{error}</AlertDescription></Alert>}

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardDescription>Backend</CardDescription><CardTitle className="text-lg">{status ? `v${status.version}` : "—"}</CardTitle></CardHeader>
          <CardContent className="text-sm text-muted">Bound to {status?.bind || "127.0.0.1"} · token-gated API</CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardDescription>Advisory AI</CardDescription><CardTitle className="text-lg flex items-center gap-2"><Activity className="h-4 w-4 text-accent" />{status?.ai.configured ? "Configured" : "Not configured"}</CardTitle></CardHeader>
          <CardContent className="text-sm text-muted">{status?.ai.model || "—"} {status?.ai.source && status.ai.source !== "none" ? `(${status.ai.source})` : ""}<div className="mt-1 text-faint">AI is advisory only; deterministic audit remains blocking.</div></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardDescription>Wallet Profiles</CardDescription><CardTitle className="text-lg">{wallets.length}</CardTitle></CardHeader>
          <CardContent className="text-sm text-muted">{wallets.filter(w=>w.protected).length} Horcrux-protected · {wallets.filter(w=>!w.protected).length} read-only/watch-only</CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardDescription>Recent Security Events</CardDescription><CardTitle className="text-lg">{events.length}</CardTitle></CardHeader>
          <CardContent className="text-sm text-muted">Last audit entries: {entries.length} · <Link href="/access-log" className="text-accent hover:underline">view log</Link></CardContent>
        </Card>
      </div>

      <div>
        <h2 className="text-sm font-medium text-muted mb-3">Quick actions</h2>
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {quick.map((q) => {
            const Icon = q.icon;
            return (
              <Link key={q.href} href={q.href} className="group rounded-xl border border-border bg-surface/60 p-4 hover:bg-surface-2 hover:border-border-strong transition-colors">
                <div className="flex items-center gap-2 mb-2 text-accent"><Icon className="h-4 w-4" /><span className="font-medium text-text group-hover:text-accent">{q.label}</span></div>
                <p className="text-sm text-muted leading-relaxed">{q.desc}</p>
              </Link>
            );
          })}
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Recent Access Log</CardTitle><CardDescription>Deterministic audit trail (allow/warn/block context)</CardDescription></div><Link href="/access-log" className={cn(buttonVariants({ variant: "ghost", size: "sm" }))}>Open</Link></CardHeader>
          <CardContent className="space-y-2">
            {entries.length === 0 && <div className="text-sm text-faint">No entries yet.</div>}
            {entries.slice().reverse().slice(0, 8).map((e, i) => (
              <div key={i} className="flex items-center justify-between text-sm border border-border rounded-md px-3 py-2 bg-surface-2/40">
                <div className="flex items-center gap-2 min-w-0">
                  <Badge variant={e.kind === "blocked" ? "danger" : e.kind === "decrypt_fail" ? "warn" : e.kind === "signed" ? "default" : "ok"}>{e.kind}</Badge>
                  <span className="text-muted truncate">shard {e.shard_id}</span>
                </div>
                <span className="text-faint font-mono text-xs">{e.ts_utc || new Date(e.ts).toLocaleString()}</span>
              </div>
            ))}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Advisory AI Events</CardTitle><CardDescription>Derived from existing actions (non-blocking)</CardDescription></div><Link href="/ai" className={cn(buttonVariants({ variant: "ghost", size: "sm" }))}>Open</Link></CardHeader>
          <CardContent className="space-y-2">
            {events.length === 0 && <div className="text-sm text-faint">No AI/security events recorded yet.</div>}
            {events.slice().reverse().slice(0, 8).map((e) => (
              <div key={e.id} className="flex items-start justify-between gap-3 text-sm border border-border rounded-md px-3 py-2 bg-surface-2/40">
                <div className="min-w-0">
                  <div className="flex items-center gap-2"><Badge variant={e.status === "anomaly" ? "warn" : e.status === "unavailable" || e.status === "skipped" ? "secondary" : e.status === "pending" ? "info" : "ok"}>{e.status}</Badge><span className="font-medium text-text">{e.action}</span></div>
                  {e.summary && <p className="text-muted mt-1 line-clamp-2">{e.summary}</p>}
                </div>
                <span className="text-faint font-mono text-xs shrink-0">{e.ts_utc || ""}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Alert variant="info">
        <AlertTitle>Security model</AlertTitle>
        <AlertDescription>
          This console is loopback-only and token-gated. No reconstructed private keys or seeds are returned to the UI/logs/AI (a freshly generated disposable test key may be shown once on init/MPC split/wallet creation). AI checks are <strong>advisory only</strong>; deterministic audit <code>Block</code>/<code>Warn</code> behavior is unchanged.
        </AlertDescription>
      </Alert>
    </div>
  );
}
