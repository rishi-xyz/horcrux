"use client";

import { useEffect, useState } from "react";
import { api, getToken, hasToken } from "@/lib/api";
import type { StatusResponse } from "@/lib/types";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export default function SettingsPage() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = async () => {
    try { setErr(null); setStatus(await api.get<StatusResponse>("/status")); }
    catch (e: any) { setErr(e?.message || "Failed to load status"); }
  };
  useEffect(() => { if (hasToken()) load(); }, []);

  return (
    <div className="space-y-5 max-w-4xl">
      <div>
        <h1 className="text-xl font-semibold">Settings & Runtime</h1>
        <p className="text-muted text-sm mt-1">Console/runtime information for this local Horcrux backend session.</p>
      </div>

      {!hasToken() && <Alert variant="danger"><AlertTitle>Missing token</AlertTitle><AlertDescription>Open via the URL printed by <code>horcrux web</code>.</AlertDescription></Alert>}
      {err && <Alert variant="danger"><AlertTitle>Error</AlertTitle><AlertDescription>{err}</AlertDescription></Alert>}

      <Card>
        <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Backend status</CardTitle><CardDescription>From <code>/api/status</code></CardDescription></div><Button variant="outline" size="sm" onClick={load}>Refresh</Button></CardHeader>
        <CardContent className="grid sm:grid-cols-2 gap-3 text-sm">
          <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">Version</div><div className="font-mono">{status?.version || "—"}</div></div>
          <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">Bind</div><div className="font-mono">{status?.bind || "—"}</div></div>
          <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">Auth</div><div>{status ? <Badge variant="ok">X-Horcrux-Token required</Badge> : "—"}</div></div>
          <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">AI</div><div>{status?.ai?.configured ? <Badge variant="default">Configured ({status.ai.source})</Badge> : <Badge variant="secondary">Not configured</Badge>}</div></div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Session token</CardTitle><CardDescription>Read from URL query parameter; sent as <code>X-Horcrux-Token</code> on API calls.</CardDescription></CardHeader>
        <CardContent className="space-y-2 text-sm">
          <div className="flex items-center gap-2"><Badge variant={hasToken() ? "ok" : "danger"}>{hasToken() ? "present" : "missing"}</Badge><span className="text-muted">Token is kept client-side for this page session only; do not share the console URL.</span></div>
          {hasToken() && <div className="font-mono text-xs text-faint break-all">length: {getToken().length} chars (value hidden in UI)</div>}
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Security notes</CardTitle><CardDescription>Enforced by backend/runtime design.</CardDescription></CardHeader>
        <CardContent className="text-sm text-muted space-y-2">
          <ul className="list-disc pl-5 space-y-1">
            <li>Server binds to a local address (default loopback <code>127.0.0.1</code>; configurable for containerized local use) and serves the embedded static console.</li>
            <li>All <code>/api/*</code> endpoints require the session token.</li>
            <li>No reconstructed private keys or seeds are returned to the UI/logs/AI. Generated disposable test keys may be shown once at creation time only.</li>
            <li>Deterministic audit scoring is the only blocking mechanism; AI checks are advisory, asynchronous, and metadata-only.</li>
            <li>Chain helpers target Solana localnet/devnet and Cosmos local (Tendermint RPC) as configured.</li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
