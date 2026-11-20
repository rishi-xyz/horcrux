"use client";

import { useMemo, useState } from "react";
import { api, isBlocked } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { DriveDestinationPlanner } from "@/components/drive-destination-planner";
import { ShardSourcePlanner } from "@/components/shard-source-planner";

function splitLines(s: string) { return s.split(/\r?\n/).map(x => x.trim()).filter(Boolean); }

export default function MpcPage() {
  const [mode, setMode] = useState<"split" | "sign">("split");

  // split
  const [chain, setChain] = useState<"solana" | "bitcoin">("solana");
  const [gen, setGen] = useState(true);
  const [keyHex, setKeyHex] = useState("");
  const [threshold, setThreshold] = useState("2");
  const [shares, setShares] = useState("3");
  const [outDir, setOutDir] = useState("mpc");
  const [splitPasswords, setSplitPasswords] = useState<string[]>(["", "", ""]);
  const [splitDestinations, setSplitDestinations] = useState<string[]>(["", "", ""]);
  const [splitLoading, setSplitLoading] = useState(false);
  const [splitErr, setSplitErr] = useState<string | null>(null);
  const [splitRes, setSplitRes] = useState<any>(null);

  const nShares = Math.max(1, parseInt(shares || "1", 10) || 1);

  // sign
  const [sChain, setSChain] = useState<"solana" | "bitcoin">("solana");
  const [signShareCount, setSignShareCount] = useState(2);
  const [sSharePaths, setSSharePaths] = useState<string[]>(["", ""]);
  const [sSharePasswords, setSSharePasswords] = useState<string[]>(["", ""]);
  const [groupDir, setGroupDir] = useState("mpc");
  const [to, setTo] = useState("");
  const [broadcast, setBroadcast] = useState(false);
  const [force, setForce] = useState(false);
  const [rpcUrl, setRpcUrl] = useState("");
  const [lamports, setLamports] = useState("1000");
  const [blockhash, setBlockhash] = useState("");
  const [amountSat, setAmountSat] = useState("");
  const [feeSat, setFeeSat] = useState("");
  const [changeAddr, setChangeAddr] = useState("");
  const [utxos, setUtxos] = useState("");
  const [rpcUser, setRpcUser] = useState("");
  const [rpcPass, setRpcPass] = useState("");
  const [signLoading, setSignLoading] = useState(false);
  const [signErr, setSignErr] = useState<string | null>(null);
  const [signBlocked, setSignBlocked] = useState<string[] | null>(null);
  const [signRes, setSignRes] = useState<any>(null);

  const runSplit = async () => {
    setSplitLoading(true); setSplitErr(null); setSplitRes(null);
    try {
      const t = parseInt(threshold || "0", 10);
      const pw = Array.from({ length: nShares }, (_, i) => (splitPasswords[i] ?? "").trim());
      const dest = Array.from({ length: nShares }, (_, i) => (splitDestinations[i] ?? "").trim());
      const allEmptyDest = dest.every((d) => !d);
      const payload: any = {
        chain,
        threshold: t,
        shares: nShares,
        out_dir: (outDir || "mpc").trim(),
        passwords: pw,
        destinations: allEmptyDest ? [] : dest,
        generate: gen,
      };
      if (!gen) payload.key_hex = keyHex.trim();
      const res = await api.post<any>("/mpc-split", payload);
      setSplitRes(res);
    } catch (e: any) { setSplitErr(e?.data?.error || e?.message || "MPC split failed"); }
    finally { setSplitLoading(false); }
  };

  const runSign = async () => {
    setSignLoading(true); setSignErr(null); setSignBlocked(null); setSignRes(null);
    try {
      const sharePaths = sSharePaths.map(p => p.trim()).filter(Boolean);
      const passwordsAligned = sharePaths.map((_, i) => (sSharePasswords[i] ?? "").trim());
      if (sharePaths.length === 0) throw new Error("Select at least one MPC share file");
      const payload: any = {
        chain: sChain,
        shares: sharePaths,
        passwords: passwordsAligned,
        group_dir: (groupDir || "mpc").trim(),
        to: to.trim(),
        broadcast,
        force,
        rpc_url: rpcUrl.trim() || undefined,
      };
      if (sChain === "solana") {
        payload.lamports = Number(lamports || 0);
        if (blockhash.trim()) payload.blockhash = blockhash.trim();
      } else {
        payload.amount_sat = Number(amountSat || 0);
        payload.fee_sat = Number(feeSat || 0);
        payload.utxo = splitLines(utxos);
        if (changeAddr.trim()) payload.change_address = changeAddr.trim();
        if (rpcUser.trim()) payload.rpc_user = rpcUser.trim();
        if (rpcPass) payload.rpc_password = rpcPass;
      }
      const res = await api.post<any>("/mpc-sign", payload);
      setSignRes(res);
    } catch (e: any) {
      if (isBlocked(e)) setSignBlocked(e.data.reasons || ["Blocked by deterministic audit preflight"]);
      else setSignErr(e?.data?.error || e?.message || "MPC sign failed");
    } finally { setSignLoading(false); }
  };

  return (
    <div className="space-y-5 max-w-6xl">
      <div>
        <h1 className="text-xl font-semibold">MPC (FROST, Mode B)</h1>
        <p className="text-muted text-sm mt-1">Dealer-split into key shares and threshold-sign without reconstructing the full key. Supports Solana and Bitcoin FROST flows exposed by the backend. Detect removable drives to choose per-share destinations (split) and share file locations/passwords (sign).</p>
      </div>

      <Tabs value={mode} onValueChange={(v) => setMode(v as any)}>
        <TabsList><TabsTrigger value="split">Split (create shares)</TabsTrigger><TabsTrigger value="sign">Sign (threshold)</TabsTrigger></TabsList>

        <TabsContent value="split">
          <Card>
            <CardHeader><CardTitle>MPC Split</CardTitle><CardDescription>Generate or import a key, split into FROST shares and a group public key file. Disposable generated keys are shown once.</CardDescription></CardHeader>
            <CardContent className="space-y-4">
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Chain</Label>
                  <select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={chain} onChange={(e)=>setChain(e.target.value as any)}>
                    <option value="solana">Solana (Ed25519 FROST)</option>
                    <option value="bitcoin">Bitcoin (secp256k1 FROST)</option>
                  </select>
                </div>
                <div className="space-y-2"><Label>Key source</Label>
                  <div className="flex gap-2 pt-1">
                    <Button type="button" size="sm" variant={gen?"default":"secondary"} onClick={()=>setGen(true)}>Generate disposable</Button>
                    <Button type="button" size="sm" variant={!gen?"default":"secondary"} onClick={()=>setGen(false)}>Provide key hex</Button>
                  </div>
                </div>
              </div>
              {chain === "bitcoin" && (
                <Alert variant="warn"><AlertTitle>Bitcoin MPC destinations</AlertTitle><AlertDescription>Per-share custom destination paths may not be applied by the current Bitcoin MPC split backend; shares are typically written to the output directory. Solana MPC supports per-share destinations.</AlertDescription></Alert>
              )}
              {!gen && <div className="space-y-2"><Label>Private key hex</Label><Input className="font-mono" value={keyHex} onChange={(e)=>setKeyHex(e.target.value)} placeholder="0x..." /></div>}
              <div className="grid sm:grid-cols-3 gap-3">
                <div className="space-y-2"><Label>Threshold (t)</Label><Input value={threshold} onChange={(e)=>setThreshold(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares (n)</Label><Input value={shares} onChange={(e)=>setShares(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares count (effective)</Label><Input value={String(nShares)} readOnly className="opacity-80" /></div>
              </div>

              <DriveDestinationPlanner
                shares={nShares}
                prefix="mpc"
                outDir={outDir}
                onOutDirChange={setOutDir}
                passwords={splitPasswords}
                onPasswordsChange={setSplitPasswords}
                destinations={splitDestinations}
                onDestinationsChange={setSplitDestinations}
              />

              <Button onClick={runSplit} disabled={splitLoading}>{splitLoading ? "Splitting…" : "Create MPC shares"}</Button>
              {splitErr && <Alert variant="danger"><AlertTitle>Split error</AlertTitle><AlertDescription className="break-all">{splitErr}</AlertDescription></Alert>}
              {splitRes && (
                <div className="space-y-3">
                  {splitRes.generated_key_hex && <Alert variant="warn"><AlertTitle>Disposable test key (shown once)</AlertTitle><AlertDescription className="font-mono text-xs break-all mt-1">0x{splitRes.generated_key_hex}</AlertDescription></Alert>}
                  <div className="text-sm text-muted">Group file: <span className="font-mono text-text break-all">{splitRes.group_path}</span></div>
                  <div className="space-y-1">{(splitRes.paths||[]).map((p:string,i:number)=><div key={i} className="font-mono text-xs border border-border rounded-md px-3 py-2 bg-surface-2/40 break-all">{p}</div>)}</div>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="sign">
          <Card>
            <CardHeader><CardTitle>MPC Sign</CardTitle><CardDescription>Select threshold share files (from detected drives if mounted) and their passwords. Deterministic audit preflight applies.</CardDescription></CardHeader>
            <CardContent className="space-y-4">
              <Tabs value={sChain} onValueChange={(v)=>setSChain(v as any)}>
                <TabsList><TabsTrigger value="solana">Solana</TabsTrigger><TabsTrigger value="bitcoin">Bitcoin</TabsTrigger></TabsList>

                <div className="mt-4 space-y-4">
                  <div className="max-w-xs space-y-2">
                    <Label>Number of shares to provide</Label>
                    <Input type="number" min={1} value={signShareCount} onChange={(e)=>{ const n=Math.max(1, parseInt(e.target.value||"1",10)||1); setSignShareCount(n); }} />
                  </div>
                  <ShardSourcePlanner
                    count={signShareCount}
                    onCountChange={setSignShareCount}
                    paths={sSharePaths}
                    onPathsChange={setSSharePaths}
                    passwords={sSharePasswords}
                    onPasswordsChange={setSSharePasswords}
                    defaultNamePrefix="mpc"
                    allowCountEdit
                    title="MPC share files"
                  />
                </div>

                <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3 mt-4">
                  <div className="space-y-2"><Label>Group directory (or path context)</Label><Input value={groupDir} onChange={(e)=>setGroupDir(e.target.value)} placeholder="e.g. mpc or /media/.../mpc" /></div>
                  <div className="space-y-2"><Label>Recipient / To</Label><Input value={to} onChange={(e)=>setTo(e.target.value)} /></div>
                  <div className="space-y-2"><Label>RPC URL (optional)</Label><Input value={rpcUrl} onChange={(e)=>setRpcUrl(e.target.value)} /></div>
                </div>
                <div className="flex items-center gap-4 mt-3">
                  <label className="flex items-center gap-2 text-sm text-muted"><input type="checkbox" checked={broadcast} onChange={(e)=>setBroadcast(e.target.checked)} /> Broadcast</label>
                  <label className="flex items-center gap-2 text-sm text-warn"><input type="checkbox" checked={force} onChange={(e)=>setForce(e.target.checked)} /> Force (override block)</label>
                </div>

                <TabsContent value="solana" className="space-y-3">
                  <div className="grid sm:grid-cols-2 gap-3">
                    <div className="space-y-2"><Label>Lamports</Label><Input value={lamports} onChange={(e)=>setLamports(e.target.value)} /></div>
                    <div className="space-y-2"><Label>Recent blockhash (optional if broadcast/fetchable)</Label><Input value={blockhash} onChange={(e)=>setBlockhash(e.target.value)} /></div>
                  </div>
                </TabsContent>
                <TabsContent value="bitcoin" className="space-y-3">
                  <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3">
                    <div className="space-y-2"><Label>Amount (sat)</Label><Input value={amountSat} onChange={(e)=>setAmountSat(e.target.value)} /></div>
                    <div className="space-y-2"><Label>Fee (sat)</Label><Input value={feeSat} onChange={(e)=>setFeeSat(e.target.value)} /></div>
                    <div className="space-y-2"><Label>Change address (optional)</Label><Input value={changeAddr} onChange={(e)=>setChangeAddr(e.target.value)} /></div>
                  </div>
                  <div className="space-y-2"><Label>UTXOs (one per line: txid:vout:amount_sat)</Label><Textarea rows={4} value={utxos} onChange={(e)=>setUtxos(e.target.value)} /></div>
                  <div className="grid sm:grid-cols-2 gap-3">
                    <div className="space-y-2"><Label>RPC user (optional)</Label><Input value={rpcUser} onChange={(e)=>setRpcUser(e.target.value)} /></div>
                    <div className="space-y-2"><Label>RPC password (optional)</Label><Input type="password" value={rpcPass} onChange={(e)=>setRpcPass(e.target.value)} /></div>
                  </div>
                </TabsContent>
              </Tabs>

              <Button onClick={runSign} disabled={signLoading}>{signLoading ? "Signing…" : "MPC sign"}</Button>
              {signBlocked && <Alert variant="danger"><AlertTitle>Blocked by deterministic audit</AlertTitle><AlertDescription><ul className="list-disc pl-5">{signBlocked.map((r,i)=><li key={i}>{r}</li>)}</ul></AlertDescription></Alert>}
              {signErr && <Alert variant="danger"><AlertTitle>Sign error</AlertTitle><AlertDescription className="break-all">{signErr}</AlertDescription></Alert>}
              {signRes && (
                <Card className="bg-surface-2/30"><CardHeader className="pb-2 flex-row items-center justify-between"><CardTitle className="text-base">MPC sign result</CardTitle><Badge variant={signRes.broadcast_error?"warn":"ok"}>{signRes.broadcast_error?"signed (broadcast issue)":"signed"}</Badge></CardHeader>
                  <CardContent className="space-y-2 text-sm">
                    {Array.isArray(signRes.warnings) && signRes.warnings.length>0 && <Alert variant="warn"><AlertTitle>Audit warnings</AlertTitle><AlertDescription><ul className="list-disc pl-5">{signRes.warnings.map((w:string,i:number)=><li key={i}>{w}</li>)}</ul></AlertDescription></Alert>}
                    <div className="grid sm:grid-cols-2 gap-2">{Object.entries(signRes).filter(([k])=>k!=="warnings").map(([k,v])=><div key={k} className="rounded-md border border-border px-3 py-2"><div className="text-xs text-faint mb-1">{k}</div><div className="font-mono text-xs break-all whitespace-pre-wrap">{typeof v==="string"?v:JSON.stringify(v)}</div></div>)}</div>
                  </CardContent>
                </Card>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
