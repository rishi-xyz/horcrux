"use client";

import { useState } from "react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { ShardSourcePlanner } from "@/components/shard-source-planner";

export default function ReconstructPage() {
  const [count, setCount] = useState(2);
  const [paths, setPaths] = useState<string[]>(["", ""]);
  const [passwords, setPasswords] = useState<string[]>(["", ""]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<any>(null);

  const run = async () => {
    setLoading(true); setError(null); setResult(null);
    try {
      const shards = paths.map((p) => p.trim()).filter(Boolean);
      if (shards.length < 1) throw new Error("Select at least one shard file");
      const pws = shards.map((_, i) => (passwords[i] ?? "").trim());
      const res = await api.post<any>("/reconstruct", { shards, passwords: pws });
      setResult(res);
    } catch (e: any) {
      setError(e?.data?.error || e?.message || "Reconstruct failed");
    } finally { setLoading(false); }
  };

  return (
    <div className="space-y-5 max-w-5xl">
      <div>
        <h1 className="text-xl font-semibold">Reconstruct (Safe, Web Parity)</h1>
        <p className="text-muted text-sm mt-1">
          Select shard files (including from detected removable drives) with their passwords. Verifies reconstruction transiently and derives non-sensitive public addresses only. <strong className="text-text">No raw private key or seed is ever returned</strong>.
        </p>
      </div>

      <Alert variant="info">
        <AlertTitle>Safety boundary</AlertTitle>
        <AlertDescription>
          This validates recovery in the console without exporting secrets. The backend reconstructs in memory, derives addresses, discards secret material, and returns only derived public info and outcome metadata.
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader><CardTitle>Shards & passwords</CardTitle><CardDescription>Provide threshold shard paths and passwords in the same order. Use detected drives to browse/select files.</CardDescription></CardHeader>
        <CardContent className="space-y-4">
          <div className="max-w-xs space-y-2">
            <Label>Number of shards to provide</Label>
            <Input type="number" min={1} value={count} onChange={(e) => { const n = Math.max(1, parseInt(e.target.value || "1", 10) || 1); setCount(n); }} />
          </div>
          <ShardSourcePlanner
            count={count}
            onCountChange={setCount}
            paths={paths}
            onPathsChange={setPaths}
            passwords={passwords}
            onPasswordsChange={setPasswords}
            defaultNamePrefix="shard"
            allowCountEdit
            title="Shard files"
          />
          <Button onClick={run} disabled={loading}>{loading ? "Checking reconstruction…" : "Reconstruct & derive addresses"}</Button>
          {error && <Alert variant="danger"><AlertTitle>Reconstruct failed</AlertTitle><AlertDescription className="break-all">{error}</AlertDescription></Alert>}
        </CardContent>
      </Card>

      {result && (
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Derived public info</CardTitle><CardDescription>{result.note}</CardDescription></div><Badge variant="ok">Success</Badge></CardHeader>
          <CardContent className="space-y-3">
            {Array.isArray(result.warnings) && result.warnings.length > 0 && (
              <Alert variant="warn"><AlertTitle>Audit warnings</AlertTitle><AlertDescription><ul className="list-disc pl-5">{result.warnings.map((w:string,i:number)=><li key={i}>{w}</li>)}</ul></AlertDescription></Alert>
            )}
            <div className="grid sm:grid-cols-2 gap-2">
              {Object.entries(result.derived || {}).map(([k,v]) => (
                <div key={k} className="rounded-md border border-border bg-surface-2/40 px-3 py-2">
                  <div className="text-xs text-faint mb-1">{k}</div>
                  <div className="font-mono text-xs break-all">{v ? String(v) : "—"}</div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
