"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { LogEntry } from "@/lib/types";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

export default function AccessLogPage() {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [tail, setTail] = useState("200");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true); setError(null);
    try {
      const q = tail ? `?tail=${encodeURIComponent(tail)}` : "";
      const res = await api.get<{ entries: LogEntry[] }>(`/log${q}`);
      setEntries(res.entries || []);
    } catch (e: any) { setError(e?.message || "Failed to load access log"); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); /* eslint-disable-next-line */ }, []);

  const badge = (k: LogEntry["kind"]) => {
    if (k === "blocked") return <Badge variant="danger">blocked</Badge>;
    if (k === "decrypt_fail") return <Badge variant="warn">decrypt_fail</Badge>;
    if (k === "signed") return <Badge variant="default">signed</Badge>;
    return <Badge variant="ok">decrypt_ok</Badge>;
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-xl font-semibold">Access Log</h1>
        <p className="text-muted text-sm mt-1">Append-only deterministic audit trail. Preflight scoring may warn or block risky shard access; blocks are enforced unless explicitly forced (where supported) and still logged.</p>
      </div>
      <Card>
        <CardHeader className="flex-row items-end justify-between gap-4">
          <div><CardTitle>Entries</CardTitle><CardDescription>Read from the local audit log on this machine.</CardDescription></div>
          <div className="flex items-center gap-2">
            <Input className="w-28" value={tail} onChange={(e) => setTail(e.target.value)} placeholder="tail" />
            <Button variant="outline" size="sm" onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</Button>
          </div>
        </CardHeader>
        <CardContent>
          {error && <div className="text-sm text-danger mb-3">{error}</div>}
          <div className="overflow-x-auto rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead className="bg-surface-2 text-muted">
                <tr>
                  <th className="text-left font-medium px-3 py-2">Time</th>
                  <th className="text-left font-medium px-3 py-2">Kind</th>
                  <th className="text-left font-medium px-3 py-2">Shard</th>
                  <th className="text-left font-medium px-3 py-2">Attempt</th>
                  <th className="text-left font-medium px-3 py-2">Unix ms</th>
                </tr>
              </thead>
              <tbody>
                {entries.length === 0 && <tr><td colSpan={5} className="px-3 py-6 text-center text-faint">No entries.</td></tr>}
                {entries.slice().reverse().map((e, i) => (
                  <tr key={i} className="border-t border-border">
                    <td className="px-3 py-2 text-muted whitespace-nowrap">{e.ts_utc || new Date(e.ts).toLocaleString()}</td>
                    <td className="px-3 py-2">{badge(e.kind)}</td>
                    <td className="px-3 py-2 font-mono">{e.shard_id}</td>
                    <td className="px-3 py-2 font-mono text-faint">{e.attempt}</td>
                    <td className="px-3 py-2 font-mono text-faint">{e.ts}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
