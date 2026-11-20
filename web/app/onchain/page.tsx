"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { WalletProfile } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function OnchainPage() {
  const [chain, setChain] = useState<"solana" | "cosmos">("solana");
  const [network, setNetwork] = useState("localnet");
  const [address, setAddress] = useState("");
  const [rpcUrl, setRpcUrl] = useState("");
  const [wallets, setWallets] = useState<WalletProfile[]>([]);
  const [selectedWallet, setSelectedWallet] = useState("");
  const [balance, setBalance] = useState<any>(null);
  const [txs, setTxs] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => { api.get<{ wallets: WalletProfile[] }>("/wallets").then(r => setWallets(r.wallets || [])).catch(() => {}); }, []);
  useEffect(() => {
    if (chain === "solana") setNetwork((n) => n === "local" ? "localnet" : (n === "devnet" ? "devnet" : "localnet"));
    else setNetwork("local");
  }, [chain]);

  const pickWallet = (id: string) => {
    setSelectedWallet(id);
    const w = wallets.find(x => x.id === id);
    if (w) { setChain(w.chain as any); setNetwork(w.network); setAddress(w.address); }
  };

  const networksFor = chain === "solana" ? ["localnet", "devnet"] : ["local"];

  const fetchData = async () => {
    setLoading(true); setErr(null); setBalance(null); setTxs(null);
    try {
      const qs = new URLSearchParams({ chain, network, address: address.trim() });
      if (rpcUrl.trim()) qs.set("rpc_url", rpcUrl.trim());
      const [b, t] = await Promise.all([
        api.get<any>(`/onchain/balance?${qs.toString()}`),
        api.get<any>(`/onchain/transactions?${qs.toString()}&limit=25`),
      ]);
      setBalance(b); setTxs(t);
    } catch (e: any) { setErr(e?.data?.error || e?.message || "Failed to fetch on-chain data"); }
    finally { setLoading(false); }
  };

  return (
    <div className="space-y-5 max-w-6xl">
      <div>
        <h1 className="text-xl font-semibold">On-Chain Views</h1>
        <p className="text-muted text-sm mt-1">View balances and recent transaction history for Solana localnet/devnet and Cosmos local addresses. Reads use configured/public RPC endpoints; results may degrade gracefully if nodes/indexers are unavailable.</p>
      </div>

      <Alert variant="info">
        <AlertTitle>Scope and limits</AlertTitle>
        <AlertDescription>
          Supported: <strong>Solana localnet</strong> (<code>127.0.0.1:8899</code>) and <strong>devnet</strong>, and <strong>Cosmos local</strong> Tendermint RPC (<code>127.0.0.1:26657</code>). Cosmos balance/history via raw Tendermint RPC can be limited without LCD/gRPC/indexing and will be labeled accordingly.
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader><CardTitle>Query</CardTitle><CardDescription>Select a saved wallet/profile or enter an address manually. RPC override is optional.</CardDescription></CardHeader>
        <CardContent className="space-y-4">
          <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
            <div className="space-y-2 lg:col-span-2"><Label>Saved wallet/profile (optional)</Label>
              <select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={selectedWallet} onChange={(e)=>pickWallet(e.target.value)}>
                <option value="">— Manual address —</option>
                {wallets.map(w => <option key={w.id} value={w.id}>{w.label} · {w.chain}/{w.network} · {w.protected ? "protected" : "read-only"}</option>)}
              </select>
            </div>
            <div className="space-y-2"><Label>Chain</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={chain} onChange={(e)=>setChain(e.target.value as any)}><option value="solana">Solana</option><option value="cosmos">Cosmos</option></select></div>
            <div className="space-y-2"><Label>Network</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={network} onChange={(e)=>setNetwork(e.target.value)}>{networksFor.map(n=> <option key={n} value={n}>{n}</option>)}</select></div>
          </div>
          <div className="grid sm:grid-cols-2 gap-3">
            <div className="space-y-2"><Label>Address</Label><Input className="font-mono" value={address} onChange={(e)=>setAddress(e.target.value)} placeholder={chain==="solana"?"Solana public key":"cosmos1... / bech32"} /></div>
            <div className="space-y-2"><Label>RPC URL override (optional)</Label><Input value={rpcUrl} onChange={(e)=>setRpcUrl(e.target.value)} placeholder={chain==="solana" ? (network==="devnet"?"https://api.devnet.solana.com":"http://127.0.0.1:8899") : "http://127.0.0.1:26657"} /></div>
          </div>
          <Button onClick={fetchData} disabled={loading || !address.trim()}>{loading ? "Fetching…" : "Fetch balance & history"}</Button>
          {err && <Alert variant="danger"><AlertTitle>Fetch error</AlertTitle><AlertDescription className="break-all whitespace-pre-wrap">{err}</AlertDescription></Alert>}
        </CardContent>
      </Card>

      {balance && (
        <Card>
          <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Balance</CardTitle><CardDescription className="font-mono text-xs break-all">{balance.address}</CardDescription></div><Badge variant="info">{balance.chain}/{balance.network}</Badge></CardHeader>
          <CardContent className="space-y-2 text-sm">
            {balance.note && <Alert variant="warn"><AlertDescription>{String(balance.note)}</AlertDescription></Alert>}
            <div className="grid sm:grid-cols-3 gap-2">
              <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">Balance</div><div className="font-mono">{String(balance.balance ?? "—")} {balance.unit ? <span className="text-muted">{balance.unit}</span> : null}</div></div>
              {"sol" in balance && <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">SOL</div><div className="font-mono">{String(balance.sol)}</div></div>}
              {balance.rpc_url && <div className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">RPC</div><div className="font-mono text-xs break-all">{balance.rpc_url}</div></div>}
            </div>
          </CardContent>
        </Card>
      )}

      {txs && (
        <Card>
          <CardHeader><CardTitle>Recent transactions</CardTitle><CardDescription>{txs.count ?? (txs.transactions||[]).length} item(s)</CardDescription></CardHeader>
          <CardContent className="space-y-2 text-sm">
            {txs.note && <Alert variant="warn"><AlertDescription>{String(txs.note)}</AlertDescription></Alert>}
            <div className="overflow-x-auto rounded-lg border border-border">
              <table className="w-full text-sm">
                <thead className="bg-surface-2 text-muted">
                  <tr><th className="text-left font-medium px-3 py-2">Signature / Hash</th><th className="text-left font-medium px-3 py-2">Status</th><th className="text-left font-medium px-3 py-2">Slot/Time</th><th className="text-left font-medium px-3 py-2">Details</th></tr>
                </thead>
                <tbody>
                  {(txs.transactions || []).length === 0 && <tr><td colSpan={4} className="px-3 py-6 text-center text-faint">No transactions returned (or unsupported/indexing unavailable).</td></tr>}
                  {(txs.transactions || []).map((t: any, i: number) => (
                    <tr key={i} className="border-t border-border align-top">
                      <td className="px-3 py-2 font-mono text-xs break-all">{t.signature || t.hash || "—"}</td>
                      <td className="px-3 py-2"><Badge variant={t.status === "success" ? "ok" : t.status === "failed" ? "danger" : "secondary"}>{t.status || "—"}</Badge></td>
                      <td className="px-3 py-2 text-xs text-muted whitespace-nowrap">{t.slot ? `slot ${t.slot}` : ""}{t.timestamp ? <div>{new Date((t.timestamp as number) * 1000).toLocaleString()}</div> : null}</td>
                      <td className="px-3 py-2 text-xs text-faint break-all">{t.err ? String(t.err) : (t.memo || "")}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
