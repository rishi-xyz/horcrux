"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { DriveInfo } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

function joinPath(dir: string, name: string) {
  if (!dir) return name;
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith("/") || dir.endsWith("\\") ? dir + name : dir + sep + name;
}

export default function VerifyPage() {
  const [paths, setPaths] = useState<string[]>(["", ""]);
  const [password, setPassword] = useState("");
  const [result, setResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [drivesErr, setDrivesErr] = useState<string | null>(null);
  const [drivesLoading, setDrivesLoading] = useState(false);

  const loadDrives = async () => {
    setDrivesLoading(true); setDrivesErr(null);
    try { const res = await api.get<{ drives: DriveInfo[] }>("/drives"); setDrives(res.drives || []); }
    catch (e: any) { setDrivesErr(e?.message || "Failed to detect drives"); setDrives([]); }
    finally { setDrivesLoading(false); }
  };
  useEffect(() => { loadDrives(); }, []);

  const setPath = (i: number, v: string) => { const next = paths.slice(); next[i] = v; setPaths(next); };
  const add = () => setPaths([...paths, ""]);
  const remove = (i: number) => { if (paths.length <= 1) return; setPaths(paths.filter((_, idx) => idx !== i)); };

  const run = async () => {
    setLoading(true); setError(null); setResult(null);
    try {
      const files = paths.map((s) => s.trim()).filter(Boolean);
      if (files.length === 0) throw new Error("Select at least one file to verify");
      const res = await api.post<any>("/verify", { files, password: password || undefined });
      setResult(res);
    } catch (e: any) { setError(e?.data?.error || e?.message || "Verify failed"); }
    finally { setLoading(false); }
  };

  return (
    <div className="space-y-5 max-w-5xl">
      <div>
        <h1 className="text-xl font-semibold">Verify Shards / Shares</h1>
        <p className="text-muted text-sm mt-1">Checks structural integrity and consistency across provided files. Browse/select files from detected removable drives. Optional password verifies decryption/tag validity where applicable (without exposing secrets).</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Files & password</CardTitle>
          <CardDescription>Select local paths (SSS <code>.hx</code> shards or supported FROST/share files). One per row.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="secondary" size="sm" onClick={loadDrives} disabled={drivesLoading}>{drivesLoading ? "Scanning…" : "Refresh drives"}</Button>
            <Button type="button" variant="outline" size="sm" onClick={add}>+ Add file</Button>
            {drives.length > 0 && <span className="text-xs text-muted">Detected removable drives: {drives.map(d => <span key={d.mount_point} className="font-mono mr-2">{d.name ? `${d.name} → ${d.mount_point}` : d.mount_point}</span>)}</span>}
          </div>
          {drivesErr && <Alert variant="warn"><AlertTitle>Drive detection</AlertTitle><AlertDescription>{drivesErr}</AlertDescription></Alert>}

          <div className="space-y-2">
            {paths.map((p, i) => (
              <div key={i} className="rounded-md border border-border bg-surface-2/30 p-2 space-y-2">
                <div className="flex items-center gap-2">
                  <span className="text-xs text-faint font-mono w-5">{i + 1}</span>
                  <Input className="h-9 text-xs font-mono" value={p} onChange={(e) => setPath(i, e.target.value)} placeholder={`shard-${i + 1}.hx or /media/.../shard-${i + 1}.hx`} />
                  {paths.length > 1 && <Button type="button" variant="ghost" size="sm" className="h-8 px-2 text-faint" onClick={() => remove(i)}>×</Button>}
                </div>
                {drives.length > 0 && (
                  <div className="flex flex-wrap gap-1 pl-7">
                    {drives.map((d) => (
                      <button key={d.mount_point} type="button" className="text-[10px] px-2 py-0.5 rounded border border-border hover:bg-surface-3 text-faint" onClick={() => setPath(i, joinPath(d.mount_point, `shard-${i + 1}.hx`))}>
                        Use {d.name || "drive"}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>

          <div className="space-y-2 max-w-md">
            <Label>Password (optional, for auth-tag/decryption checks)</Label>
            <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Used only for integrity checks; not stored" />
          </div>

          <Button onClick={run} disabled={loading}>{loading ? "Verifying…" : "Run Verify"}</Button>
          {error && <Alert variant="danger"><AlertTitle>Verify error</AlertTitle><AlertDescription className="break-all">{error}</AlertDescription></Alert>}
        </CardContent>
      </Card>

      {result && (
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Result</CardTitle><CardDescription>Consistency and per-file reports.</CardDescription></div>
            <Badge variant={result.all_ok ? "ok" : "warn"}>{result.all_ok ? "All OK" : "Issues found"}</Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            {result.consistency_error && <Alert variant="warn"><AlertTitle>Consistency issue</AlertTitle><AlertDescription>{String(result.consistency_error)}</AlertDescription></Alert>}
            <div className="space-y-2">
              {(result.reports || []).map((r: any, i: number) => (
                <div key={i} className="rounded-lg border border-border bg-surface-2/40 px-3 py-2 text-sm flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="font-mono text-xs text-muted break-all">{r.path}</div>
                    <div className="mt-1 text-faint">kind: {r.kind || "—"} · threshold: {r.threshold ?? "—"} · shares: {r.shares ?? "—"}</div>
                  </div>
                  <Badge variant={r.ok ? "ok" : "danger"}>{r.ok ? "ok" : "invalid"}</Badge>
                </div>
              ))}
            </div>
            <p className="text-xs text-faint">Verify results are derived from local files and deterministic checks. Any AI follow-up is advisory and recorded separately in AI & Security.</p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
