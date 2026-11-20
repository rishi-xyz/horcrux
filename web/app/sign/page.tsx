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
import { ShardSourcePlanner } from "@/components/shard-source-planner";

function lines(s: string) { return s.split(/\r?\n/).map(x=>x.trim()).filter(Boolean); }

export default function SignPage() {
  const [chain, setChain] = useState<"solana"|"bitcoin"|"cosmos">("solana");
  const [shardCount, setShardCount] = useState(2);
  const [shardPaths, setShardPaths] = useState<string[]>(["", ""]);
  const [shardPasswords, setShardPasswords] = useState<string[]>(["", ""]);
  const [to, setTo] = useState("");
  const [broadcast, setBroadcast] = useState(false);
  const [force, setForce] = useState(false);
  const [rpcUrl, setRpcUrl] = useState("");

  // solana
  const [lamports, setLamports] = useState("1000");
  const [blockhash, setBlockhash] = useState("");
  // bitcoin
  const [amountSat, setAmountSat] = useState("");
  const [feeSat, setFeeSat] = useState("");
  const [changeAddr, setChangeAddr] = useState("");
  const [utxos, setUtxos] = useState("");
  const [rpcUser, setRpcUser] = useState("");
  const [rpcPass, setRpcPass] = useState("");
  // cosmos
  const [chainId, setChainId] = useState("");
  const [accountNumber, setAccountNumber] = useState("");
  const [sequence, setSequence] = useState("");
  const [amount, setAmount] = useState("");
  const [denom, setDenom] = useState("uatom");
  const [gas, setGas] = useState("200000");
  const [fee, setFee] = useState("5000");
  const [feeDenom, setFeeDenom] = useState("");
  const [memo, setMemo] = useState("");
  const [timeoutHeight, setTimeoutHeight] = useState("0");

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [blocked, setBlocked] = useState<string[] | null>(null);
  const [result, setResult] = useState<any>(null);

  const run = async () => {
    setLoading(true); setError(null); setBlocked(null); setResult(null);
    try {
      const shards = shardPaths.map(p => p.trim()).filter(Boolean);
      const passwords = shards.map((_, i) => (shardPasswords[i] ?? "").trim());
      if (shards.length === 0) throw new Error("Select at least one shard file");
      const base: any = {
        chain, shards, passwords, to: to.trim(), broadcast, force,
        rpc_url: rpcUrl.trim() || undefined,
      };
      if (chain === "solana") {
        base.lamports = Number(lamports || 0);
        if (blockhash.trim()) base.blockhash = blockhash.trim();
      } else if (chain === "bitcoin") {
        base.amount_sat = Number(amountSat || 0);
        base.fee_sat = Number(feeSat || 0);
        base.utxo = lines(utxos);
        if (changeAddr.trim()) base.change_address = changeAddr.trim();
        if (rpcUser.trim()) base.rpc_user = rpcUser.trim();
        if (rpcPass) base.rpc_password = rpcPass;
      } else if (chain === "cosmos") {
        base.chain_id = chainId.trim();
        base.account_number = Number(accountNumber || 0);
        base.sequence = Number(sequence || 0);
        base.amount = Number(amount || 0);
        base.denom = denom.trim();
        base.gas = Number(gas || 0);
        base.fee = Number(fee || 0);
        if (feeDenom.trim()) base.fee_denom = feeDenom.trim();
        base.memo = memo;
        base.timeout_height = Number(timeoutHeight || 0);
      }
      const res = await api.post<any>("/sign", base);
      setResult(res);
    } catch (e: any) {
      if (isBlocked(e)) setBlocked(e.data.reasons || ["Blocked by deterministic audit preflight"]);
      else setError(e?.data?.error && typeof e.data.error === "string" ? e.data.error : (e?.message || "Sign failed"));
    } finally { setLoading(false); }
  };

  return (
    <div className="space-y-5 max-w-6xl">
      <div>
        <h1 className="text-xl font-semibold">Sign (Mode A / SSS)</h1>
        <p className="text-muted text-sm mt-1">Select shard files (including from detected removable drives) and passwords. The key is reconstructed transiently in backend memory for signing and discarded. Deterministic audit preflight applies; blocks are enforced unless you explicitly force (still logged). AI checks are advisory.</p>
      </div>

      <Card>
        <CardHeader><CardTitle>Inputs</CardTitle><CardDescription>Select chain and provide shard paths/passwords in order.</CardDescription></CardHeader>
        <CardContent className="space-y-4">
          <Tabs value={chain} onValueChange={(v) => setChain(v as any)}>
            <TabsList>
              <TabsTrigger value="solana">Solana</TabsTrigger>
              <TabsTrigger value="bitcoin">Bitcoin</TabsTrigger>
              <TabsTrigger value="cosmos">Cosmos</TabsTrigger>
            </TabsList>

            <div className="mt-4 space-y-3">
              <div className="max-w-xs space-y-2">
                <Label>Number of shards to provide</Label>
                <Input type="number" min={1} value={shardCount} onChange={(e)=>{ const n=Math.max(1, parseInt(e.target.value||"1",10)||1); setShardCount(n); }} />
              </div>
              <ShardSourcePlanner
                count={shardCount}
                onCountChange={setShardCount}
                paths={shardPaths}
                onPathsChange={setShardPaths}
                passwords={shardPasswords}
                onPasswordsChange={setShardPasswords}
                defaultNamePrefix="shard"
                allowCountEdit
                title="Shard files"
              />
            </div>

            <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3 mt-4">
              <div className="space-y-2"><Label>Recipient / To</Label><Input value={to} onChange={(e)=>setTo(e.target.value)} /></div>
              <div className="space-y-2"><Label>RPC URL (optional)</Label><Input value={rpcUrl} onChange={(e)=>setRpcUrl(e.target.value)} placeholder="Uses chain default if empty" /></div>
              <div className="flex items-end gap-4 pb-2">
                <label className="flex items-center gap-2 text-sm text-muted"><input type="checkbox" checked={broadcast} onChange={(e)=>setBroadcast(e.target.checked)} /> Broadcast</label>
                <label className="flex items-center gap-2 text-sm text-warn"><input type="checkbox" checked={force} onChange={(e)=>setForce(e.target.checked)} /> Force (override block)</label>
              </div>
            </div>

            <TabsContent value="solana" className="space-y-3">
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Lamports</Label><Input value={lamports} onChange={(e)=>setLamports(e.target.value)} /></div>
                <div className="space-y-2"><Label>Recent blockhash (optional if broadcast/fetchable)</Label><Input value={blockhash} onChange={(e)=>setBlockhash(e.target.value)} /></div>
              </div>
              <p className="text-xs text-faint">For localnet use RPC <code>http://127.0.0.1:8899</code>. Broadcast requires a funded sender and reachable RPC.</p>
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

            <TabsContent value="cosmos" className="space-y-3">
              <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3">
                <div className="space-y-2"><Label>Chain ID</Label><Input value={chainId} onChange={(e)=>setChainId(e.target.value)} placeholder="e.g. localnet-1" /></div>
                <div className="space-y-2"><Label>Account number</Label><Input value={accountNumber} onChange={(e)=>setAccountNumber(e.target.value)} /></div>
                <div className="space-y-2"><Label>Sequence</Label><Input value={sequence} onChange={(e)=>setSequence(e.target.value)} /></div>
                <div className="space-y-2"><Label>Amount</Label><Input value={amount} onChange={(e)=>setAmount(e.target.value)} /></div>
                <div className="space-y-2"><Label>Denom</Label><Input value={denom} onChange={(e)=>setDenom(e.target.value)} /></div>
                <div className="space-y-2"><Label>Gas</Label><Input value={gas} onChange={(e)=>setGas(e.target.value)} /></div>
                <div className="space-y-2"><Label>Fee</Label><Input value={fee} onChange={(e)=>setFee(e.target.value)} /></div>
                <div className="space-y-2"><Label>Fee denom (optional)</Label><Input value={feeDenom} onChange={(e)=>setFeeDenom(e.target.value)} /></div>
                <div className="space-y-2"><Label>Timeout height</Label><Input value={timeoutHeight} onChange={(e)=>setTimeoutHeight(e.target.value)} /></div>
              </div>
              <div className="space-y-2"><Label>Memo</Label><Input value={memo} onChange={(e)=>setMemo(e.target.value)} /></div>
              <p className="text-xs text-faint">Cosmos local RPC default: <code>http://127.0.0.1:26657</code>.</p>
            </TabsContent>
          </Tabs>

          <Button onClick={run} disabled={loading}>{loading ? "Signing…" : "Sign transaction"}</Button>

          {blocked && (
            <Alert variant="danger">
              <AlertTitle>Blocked by deterministic audit</AlertTitle>
              <AlertDescription>
                <ul className="list-disc pl-5 space-y-1">{blocked.map((r,i)=><li key={i}>{r}</li>)}</ul>
                <div className="mt-2 text-xs opacity-90">Enable "Force" only if you understand the risk; forced overrides are still logged by backend logic.</div>
              </AlertDescription>
            </Alert>
          )}
          {error && <Alert variant="danger"><AlertTitle>Sign error</AlertTitle><AlertDescription className="break-all">{error}</AlertDescription></Alert>}
        </CardContent>
      </Card>

      {result && (
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Result</CardTitle><CardDescription>Signed output and any warnings/broadcast status.</CardDescription></div><Badge variant={result.broadcast_error ? "warn" : "ok"}>{result.broadcast_error ? "signed (broadcast issue)" : "signed"}</Badge></CardHeader>
          <CardContent className="space-y-3 text-sm">
            {Array.isArray(result.warnings) && result.warnings.length > 0 && (
              <Alert variant="warn"><AlertTitle>Audit warnings</AlertTitle><AlertDescription><ul className="list-disc pl-5">{result.warnings.map((w: string, i: number)=><li key={i}>{w}</li>)}</ul></AlertDescription></Alert>
            )}
            <div className="grid sm:grid-cols-2 gap-2">
              {Object.entries(result).filter(([k]) => !["warnings"].includes(k)).map(([k,v]) => (
                <div key={k} className="rounded-md border border-border bg-surface-2/40 px-3 py-2">
                  <div className="text-xs text-faint mb-1">{k}</div>
                  <div className="font-mono text-xs break-all whitespace-pre-wrap">{typeof v === "string" ? v : JSON.stringify(v)}</div>
                </div>
              ))}
            </div>
            {result.broadcast_error && <Alert variant="warn"><AlertTitle>Broadcast error</AlertTitle><AlertDescription className="break-all">{String(result.broadcast_error)}</AlertDescription></Alert>}
            {result.mined_signature && <Alert variant="ok"><AlertTitle>Broadcast confirmed/submitted</AlertTitle><AlertDescription className="font-mono break-all">{String(result.mined_signature)}</AlertDescription></Alert>}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
