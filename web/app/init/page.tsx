"use client";

import { useMemo, useState } from "react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { DriveDestinationPlanner } from "@/components/drive-destination-planner";

export default function InitPage() {
  const [mode, setMode] = useState<"generate" | "key">("generate");
  const [keyHex, setKeyHex] = useState("");
  const [threshold, setThreshold] = useState("2");
  const [shares, setShares] = useState("3");
  const [outDir, setOutDir] = useState("shards");
  const [passwords, setPasswords] = useState<string[]>(["", "", ""]);
  const [destinations, setDestinations] = useState<string[]>(["", "", ""]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<any>(null);

  const nShares = Math.max(1, parseInt(shares || "1", 10) || 1);

  const run = async () => {
    setLoading(true); setError(null); setResult(null);
    try {
      const t = parseInt(threshold || "0", 10);
      const pw = Array.from({ length: nShares }, (_, i) => (passwords[i] ?? "").trim());
      const dest = Array.from({ length: nShares }, (_, i) => (destinations[i] ?? "").trim());
      const allEmptyDest = dest.every((d) => !d);
      const payload: any = {
        threshold: t,
        shares: nShares,
        out_dir: (outDir || "shards").trim(),
        passwords: pw,
        destinations: allEmptyDest ? [] : dest,
        generate: mode === "generate",
      };
      if (mode === "key") payload.key_hex = keyHex.trim();
      const res = await api.post<any>("/init", payload);
      setResult(res);
    } catch (e: any) {
      setError(e?.data?.error || e?.message || "Init failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-5 max-w-5xl">
      <div>
        <h1 className="text-xl font-semibold">Init (Mode A / SSS)</h1>
        <p className="text-muted text-sm mt-1">
          Split a secret key into encrypted Shamir shards. Detect inserted removable drives (pendrives), choose per-share destinations, and set per-share passwords.
          Files are written on the backend machine. If you generate a disposable test key, it is shown once and never persisted.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Configuration</CardTitle>
          <CardDescription>Threshold-of-shares with per-shard passwords and optional per-shard destinations (drives/directories).</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-2">
            <Button variant={mode === "generate" ? "default" : "secondary"} size="sm" onClick={() => setMode("generate")}>Generate disposable test key</Button>
            <Button variant={mode === "key" ? "default" : "secondary"} size="sm" onClick={() => setMode("key")}>Use provided key hex</Button>
          </div>

          {mode === "key" && (
            <div className="space-y-2">
              <Label>Private key hex (32-byte secp256k1, with or without 0x)</Label>
              <Input value={keyHex} onChange={(e) => setKeyHex(e.target.value)} placeholder="0x..." className="font-mono" />
              <p className="text-xs text-faint">Sent only to the local backend for splitting; not stored in responses/logs/AI by design. Prefer generate for demos.</p>
            </div>
          )}

          <div className="grid sm:grid-cols-3 gap-3">
            <div className="space-y-2"><Label>Threshold (t)</Label><Input value={threshold} onChange={(e) => setThreshold(e.target.value)} /></div>
            <div className="space-y-2"><Label>Shares (n)</Label><Input value={shares} onChange={(e) => setShares(e.target.value)} /></div>
            <div className="space-y-2"><Label>Shares count (effective)</Label><Input value={String(nShares)} readOnly className="opacity-80" /></div>
          </div>

          <DriveDestinationPlanner
            shares={nShares}
            prefix="shard"
            outDir={outDir}
            onOutDirChange={setOutDir}
            passwords={passwords}
            onPasswordsChange={setPasswords}
            destinations={destinations}
            onDestinationsChange={setDestinations}
          />

          <Button onClick={run} disabled={loading}>{loading ? "Creating shards…" : "Create shards"}</Button>
          {error && <Alert variant="danger"><AlertTitle>Init failed</AlertTitle><AlertDescription className="break-all">{error}</AlertDescription></Alert>}
        </CardContent>
      </Card>

      {result && (
        <Card>
          <CardHeader><CardTitle>Shards created</CardTitle><CardDescription>Files written locally. Advisory AI event recorded.</CardDescription></CardHeader>
          <CardContent className="space-y-3">
            {result.generated_key_hex && (
              <Alert variant="warn">
                <AlertTitle>Disposable test key (shown once)</AlertTitle>
                <AlertDescription className="font-mono break-all text-xs mt-1">0x{result.generated_key_hex}</AlertDescription>
                <div className="text-xs mt-2 opacity-90">Save this now if needed for testing; it is not stored or returned again.</div>
              </Alert>
            )}
            <div className="text-sm text-muted">Output dir: <span className="font-mono text-text">{result.out_dir}</span></div>
            <div className="space-y-1">
              {(result.paths || []).map((p: string, i: number) => (
                <div key={i} className="font-mono text-xs border border-border rounded-md px-3 py-2 bg-surface-2/40 break-all">{p}</div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
