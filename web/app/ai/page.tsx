"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { AiEventRef, AiStatus } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export default function AiPage() {
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [events, setEvents] = useState<AiEventRef[]>([]);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [checkRes, setCheckRes] = useState<any>(null);

  const load = async () => {
    try {
      setErr(null);
      const [s, e] = await Promise.all([
        api.get<AiStatus>("/ai/status"),
        api.get<{ events: AiEventRef[] }>("/ai/events?tail=200"),
      ]);
      setStatus(s); setEvents(e.events || []);
      if (s.model && !model) setModel(s.model);
    } catch (e: any) { setErr(e?.message || "Failed to load AI status"); }
  };
  useEffect(() => { load(); /* eslint-disable-next-line */ }, []);

  const saveConfig = async () => {
    setBusy("config"); setErr(null); setMsg(null);
    try {
      const payload: any = {};
      if (apiKey.trim()) payload.api_key = apiKey.trim();
      if (model.trim()) payload.model = model.trim();
      const s = await api.post<AiStatus>("/ai/configure", payload);
      setStatus(s); setApiKey(""); setMsg("AI configuration saved locally (.env / web state). Key is never exposed back to the UI.");
      await load();
    } catch (e: any) { setErr(e?.message || "Failed to save config"); }
    finally { setBusy(null); }
  };

  const runCheck = async () => {
    setBusy("check"); setErr(null); setMsg(null); setCheckRes(null);
    try {
      const res = await api.post<any>("/ai/check", { tail: 50, model: model || undefined });
      setCheckRes(res); await load();
      setMsg("Manual advisory check completed and recorded.");
    } catch (e: any) { setErr(e?.data?.error || e?.message || "AI check failed"); }
    finally { setBusy(null); }
  };

  return (
    <div className="space-y-5 max-w-5xl">
      <div>
        <h1 className="text-xl font-semibold">AI & Security (Advisory)</h1>
        <p className="text-muted text-sm mt-1">Automatic AI checks run on relevant existing actions/transactions and are recorded as derived events. AI is <strong className="text-text">advisory only</strong> and never blocks; deterministic audit preflight remains the only blocking mechanism.</p>
      </div>

      {err && <Alert variant="danger"><AlertTitle>Error</AlertTitle><AlertDescription>{err}</AlertDescription></Alert>}
      {msg && <Alert variant="ok"><AlertTitle>OK</AlertTitle><AlertDescription>{msg}</AlertDescription></Alert>}

      <div className="grid lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader><CardTitle>Status</CardTitle><CardDescription>OpenRouter advisory model configuration (metadata-only checks).</CardDescription></CardHeader>
          <CardContent className="space-y-3 text-sm">
            <div className="flex items-center gap-2"><span className="text-muted">Configured:</span>{status?.configured ? <Badge variant="ok">yes</Badge> : <Badge variant="secondary">no</Badge>}</div>
            <div><span className="text-muted">Model:</span> <span className="font-mono text-text">{status?.model || "—"}</span></div>
            <div><span className="text-muted">Source:</span> <span className="font-mono text-text">{status?.source || "—"}</span></div>
            {status?.key_redacted && <div><span className="text-muted">API key:</span> <span className="font-mono text-text">{status.key_redacted}</span></div>}
            <div className="text-faint text-xs">AI checks send redacted/metadata summaries only. Passwords, seeds, and reconstructed private keys are never sent.</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader><CardTitle>Configure</CardTitle><CardDescription>Set <code>OPENROUTER_API_KEY</code> and/or preferred model. Saved locally.</CardDescription></CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-2"><Label>OpenRouter API key (stored in local .env; not echoed)</Label><Input type="password" value={apiKey} onChange={(e)=>setApiKey(e.target.value)} placeholder="sk-or-..." /></div>
            <div className="space-y-2"><Label>Model (optional)</Label><Input value={model} onChange={(e)=>setModel(e.target.value)} placeholder="meta-llama/llama-3.3-70b-instruct:free" /></div>
            <div className="flex gap-2">
              <Button onClick={saveConfig} disabled={busy!==null}>{busy==="config"?"Saving…":"Save config"}</Button>
              <Button variant="secondary" onClick={runCheck} disabled={busy!==null}>{busy==="check"?"Checking…":"Run manual check"}</Button>
              <Button variant="outline" onClick={load} disabled={busy!==null}>Refresh</Button>
            </div>
            {checkRes && <Alert variant={checkRes.status==="anomaly"?"warn":checkRes.status==="ok"?"ok":"info"}><AlertTitle>Manual check: {checkRes.status}</AlertTitle><AlertDescription className="break-words">{checkRes.summary || "(no summary)"}</AlertDescription></Alert>}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader><CardTitle>Derived Security / AI Events</CardTitle><CardDescription>Recorded from existing actions (inserted/configured/rejected/signed etc.) and advisory AI outcomes. Newest shown first.</CardDescription></CardHeader>
        <CardContent>
          {events.length === 0 && <div className="text-sm text-faint">No events recorded yet.</div>}
          <div className="space-y-2">
            {events.slice().reverse().map((e) => (
              <div key={e.id} className="rounded-lg border border-border bg-surface-2/40 px-3 py-2 text-sm">
                <div className="flex flex-wrap items-center gap-2 justify-between">
                  <div className="flex items-center gap-2 flex-wrap">
                    <Badge variant={e.status==="anomaly"?"warn":e.status==="ok"?"ok":e.status==="pending"?"info":"secondary"}>{e.status}</Badge>
                    <span className="font-medium">{e.action}</span>
                    {e.derived && <Badge variant="secondary">{e.derived}</Badge>}
                    {e.details && typeof (e.details as any).context?.chain === "string" && <span className="text-faint font-mono text-xs">{String((e.details as any).context.chain)}</span>}
                  </div>
                  <span className="text-faint font-mono text-xs">{e.ts_utc || new Date(e.ts).toLocaleString()}</span>
                </div>
                {e.summary && <p className="text-muted mt-1 whitespace-pre-wrap break-words">{e.summary}</p>}
                {e.details && Object.keys(e.details).length > 0 && <pre className="mt-2 text-xs text-faint bg-bg/50 border border-border rounded p-2 overflow-x-auto">{JSON.stringify(e.details, null, 2)}</pre>}
                <div className="text-[11px] text-faint mt-1">{e.model ? `model: ${e.model} · ` : ""}id: {e.id}</div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Alert variant="info">
        <AlertTitle>Advisory-only behavior</AlertTitle>
        <AlertDescription>AI results are informational and may be unavailable/rate-limited. They never override deterministic audit <code>Warn</code>/<code>Block</code> decisions and never trigger new shard lifecycle/quarantine state.</AlertDescription>
      </Alert>
    </div>
  );
}
