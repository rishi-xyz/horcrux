"use client";

import { useEffect, useState } from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
import { api } from "@/lib/api";
import type { WalletProfile } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { DriveDestinationPlanner } from "@/components/drive-destination-planner";

function short(s: string) { return s.length > 16 ? s.slice(0, 8) + "…" + s.slice(-6) : s; }

export default function WalletsPage() {
  const [wallets, setWallets] = useState<WalletProfile[]>([]);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [tab, setTab] = useState("list");

  // create/import shared
  const [label, setLabel] = useState("");
  const [chain, setChain] = useState<"solana" | "cosmos">("solana");
  const [network, setNetwork] = useState("localnet");
  const [mode, setMode] = useState<"sss" | "frost">("sss");
  const [threshold, setThreshold] = useState("2");
  const [shares, setShares] = useState("3");
  const [sharePasswords, setSharePasswords] = useState<string[]>(["", "", ""]);
  const [shareDestinations, setShareDestinations] = useState<string[]>(["", "", ""]);
  const [outDir, setOutDir] = useState("shards");
  const [cosmosPrefix, setCosmosPrefix] = useState("cosmos");
  const [importKey, setImportKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastResult, setLastResult] = useState<any>(null);

  // connect/watch
  const [cLabel, setCLabel] = useState("");
  const [cChain, setCChain] = useState<"solana" | "cosmos">("solana");
  const [cNetwork, setCNetwork] = useState("devnet");
  const [cAddress, setCAddress] = useState("");
  const [wLabel, setWLabel] = useState("");
  const [wChain, setWChain] = useState<"solana" | "cosmos">("solana");
  const [wNetwork, setWNetwork] = useState("devnet");
  const [wAddress, setWAddress] = useState("");

  const sol = useWallet();

  const load = async () => {
    setLoading(true); setErr(null);
    try { const res = await api.get<{ wallets: WalletProfile[] }>("/wallets"); setWallets(res.wallets || []); }
    catch (e: any) { setErr(e?.message || "Failed to load wallets"); }
    finally { setLoading(false); }
  };
  useEffect(() => { load(); }, []);

  // keep network hints sane
  useEffect(() => {
    if (chain === "solana") {
      setNetwork((n) => n === "local" ? "localnet" : n);
      if (mode === "frost") setOutDir((d) => d === "shards" ? "mpc" : d);
      else setOutDir((d) => d === "mpc" ? "shards" : d);
    } else {
      setNetwork((n) => n === "localnet" ? "local" : n);
      if (mode === "frost") setMode("sss");
    }
  }, [chain, mode]);

  const nShares = Math.max(1, parseInt(shares || "1", 10) || 1);
  const effectiveMode = chain === "cosmos" ? "sss" : mode;
  const prefix: "shard" | "mpc" = effectiveMode === "frost" ? "mpc" : "shard";

  const networksFor = (ch: string) => ch === "solana" ? ["localnet", "devnet"] : ["local"];

  const shardPayload = (withKey = false) => {
    const t = parseInt(threshold || "0", 10);
    const pw = Array.from({ length: nShares }, (_, i) => (sharePasswords[i] ?? "").trim());
    const dest = Array.from({ length: nShares }, (_, i) => (shareDestinations[i] ?? "").trim());
    const allEmptyDest = dest.every((d) => !d);
    const p: any = {
      label: label.trim(),
      chain,
      network,
      mode: effectiveMode,
      threshold: t,
      shares: nShares,
      passwords: pw,
      out_dir: (outDir || (effectiveMode === "frost" ? "mpc" : "shards")).trim(),
      destinations: allEmptyDest ? [] : dest,
      cosmos_prefix: cosmosPrefix || "cosmos",
    };
    if (withKey) p.key_hex = importKey.trim();
    return p;
  };

  const create = async () => {
    setBusy(true); setErr(null); setMsg(null); setLastResult(null);
    try {
      const res = await api.post<any>("/wallets", shardPayload(false));
      setLastResult(res);
      setMsg("Shard-backed wallet created (Horcrux-protected). Files written on the backend machine.");
      setTab("list");
      await load();
    } catch (e: any) { setErr(e?.data?.error || e?.message || "Create failed"); }
    finally { setBusy(false); }
  };
  const doImport = async () => {
    setBusy(true); setErr(null); setMsg(null); setLastResult(null);
    try {
      const res = await api.post<any>("/wallets/import", shardPayload(true));
      setLastResult(res);
      setMsg("Imported key split into Horcrux-protected shards/shares. Key material was not stored.");
      setTab("list");
      setImportKey("");
      await load();
    } catch (e: any) { setErr(e?.data?.error || e?.message || "Import failed"); }
    finally { setBusy(false); }
  };
  const saveConnected = async () => {
    setBusy(true); setErr(null); setMsg(null);
    try {
      await api.post("/wallets/connect", { label: cLabel || "Browser wallet", chain: cChain, network: cNetwork, address: cAddress.trim(), source: cChain === "solana" ? "solana_wallet_adapter" : "browser" });
      setMsg("Browser-connected wallet saved as read-only (not Horcrux-protected)."); setCLabel(""); setCAddress(""); await load();
    } catch (e: any) { setErr(e?.data?.error || e?.message || "Connect/save failed"); }
    finally { setBusy(false); }
  };
  const saveWatch = async () => {
    setBusy(true); setErr(null); setMsg(null);
    try {
      await api.post("/wallets/watch", { label: wLabel || "Watch-only", chain: wChain, network: wNetwork, address: wAddress.trim() });
      setMsg("Watch-only address added."); setWLabel(""); setWAddress(""); await load();
    } catch (e: any) { setErr(e?.data?.error || e?.message || "Watch add failed"); }
    finally { setBusy(false); }
  };
  const remove = async (id: string) => {
    if (!confirm("Remove this wallet profile from local console state? (Shard/share files on disk are not deleted.)")) return;
    try { await api.del(`/wallets/${encodeURIComponent(id)}`); await load(); } catch (e: any) { setErr(e?.message || "Delete failed"); }
  };

  const browserPk = sol.publicKey?.toBase58() || "";

  return (
    <div className="space-y-5 max-w-6xl">
      <div>
        <h1 className="text-xl font-semibold">Wallets</h1>
        <p className="text-muted text-sm mt-1">Manage shard-backed (Horcrux-protected), browser-connected (read-only), and watch-only addresses. Shard-backed wallets sign via Horcrux SSS/MPC flows; browser-connected wallets have no Horcrux signing authority.</p>
      </div>

      {err && <Alert variant="danger"><AlertTitle>Error</AlertTitle><AlertDescription className="break-all">{err}</AlertDescription></Alert>}
      {msg && <Alert variant="ok"><AlertTitle>OK</AlertTitle><AlertDescription>{msg}</AlertDescription></Alert>}

      {lastResult?.generated_key_hex && (
        <Alert variant="warn"><AlertTitle>Disposable test key (shown once)</AlertTitle><AlertDescription className="font-mono text-xs break-all mt-1">0x{lastResult.generated_key_hex}</AlertDescription><div className="text-xs mt-2 opacity-90">Not stored or returned again. Save only if needed for testing.</div></Alert>
      )}

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="list">Profiles</TabsTrigger>
          <TabsTrigger value="create">Create shard-backed</TabsTrigger>
          <TabsTrigger value="import">Import to shards</TabsTrigger>
          <TabsTrigger value="connect">Browser connect</TabsTrigger>
          <TabsTrigger value="watch">Watch-only</TabsTrigger>
        </TabsList>

        <TabsContent value="list">
          <Card>
            <CardHeader className="flex-row items-center justify-between"><div><CardTitle>Wallet profiles</CardTitle><CardDescription>Stored locally in console state. Protected = requires Horcrux shards/MPC to sign.</CardDescription></div><Button variant="outline" size="sm" onClick={load} disabled={loading}>{loading?"Loading…":"Refresh"}</Button></CardHeader>
            <CardContent className="space-y-2">
              {wallets.length === 0 && <div className="text-sm text-faint">No wallet profiles yet.</div>}
              {wallets.map((w) => {
                const kind = w.type || w.kind || "unknown";
                return (
                <div key={w.id} className="rounded-lg border border-border bg-surface-2/40 p-3 flex flex-col sm:flex-row sm:items-start justify-between gap-3">
                  <div className="min-w-0 space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{w.label}</span>
                      <Badge variant={w.protected ? "ok" : "secondary"}>{w.protected ? "Horcrux-protected" : "Not Horcrux-protected"}</Badge>
                      <Badge variant={kind === "shard_backed" ? "default" : kind === "browser_connected" ? "info" : "secondary"}>{kind}</Badge>
                      <span className="text-xs text-faint font-mono">{w.chain}/{w.network}</span>
                    </div>
                    <div className="font-mono text-xs break-all text-text">{w.address}</div>
                    {w.shard_refs && w.shard_refs.length > 0 && <div className="text-xs text-muted break-all">shards: {w.shard_refs.join(", ")}</div>}
                    {w.group_ref && <div className="text-xs text-muted break-all">group: {w.group_ref}</div>}
                    {w.notes && <div className="text-xs text-faint">{w.notes}</div>}
                  </div>
                  <Button variant="ghost" size="sm" className="text-danger hover:text-danger" onClick={() => remove(w.id)}>Remove</Button>
                </div>
              ); })}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="create">
          <Card>
            <CardHeader><CardTitle>Create shard-backed wallet</CardTitle><CardDescription>Generates a fresh disposable key, splits it into SSS shards or FROST shares, derives the address, and stores only non-secret profile metadata. Generated key is shown once. Detect removable drives and choose per-share destinations/passwords.</CardDescription></CardHeader>
            <CardContent className="space-y-4">
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="space-y-2 lg:col-span-2"><Label>Label</Label><Input value={label} onChange={(e)=>setLabel(e.target.value)} placeholder="e.g. Treasury (localnet)" /></div>
                <div className="space-y-2"><Label>Chain</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={chain} onChange={(e)=>setChain(e.target.value as any)}><option value="solana">Solana</option><option value="cosmos">Cosmos</option></select></div>
                <div className="space-y-2"><Label>Network</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={network} onChange={(e)=>setNetwork(e.target.value)}>{networksFor(chain).map(n=><option key={n} value={n}>{n}</option>)}</select></div>
              </div>
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="space-y-2"><Label>Mode</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={effectiveMode} disabled={chain==="cosmos"} onChange={(e)=>setMode(e.target.value as any)}><option value="sss">SSS (Mode A)</option><option value="frost">FROST MPC (Mode B)</option></select>{chain==="cosmos"&&<div className="text-xs text-faint">Cosmos supports SSS wallets only.</div>}</div>
                <div className="space-y-2"><Label>Threshold</Label><Input value={threshold} onChange={(e)=>setThreshold(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares</Label><Input value={shares} onChange={(e)=>setShares(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares count (effective)</Label><Input value={String(nShares)} readOnly className="opacity-80" /></div>
              </div>
              {chain==="cosmos" && <div className="space-y-2 max-w-xs"><Label>Cosmos bech32 prefix</Label><Input value={cosmosPrefix} onChange={(e)=>setCosmosPrefix(e.target.value)} /></div>}

              <DriveDestinationPlanner
                shares={nShares}
                prefix={prefix}
                outDir={outDir}
                onOutDirChange={setOutDir}
                passwords={sharePasswords}
                onPasswordsChange={setSharePasswords}
                destinations={shareDestinations}
                onDestinationsChange={setShareDestinations}
              />

              <Button onClick={create} disabled={busy || !label.trim()}>{busy?"Creating…":"Create Horcrux-protected wallet"}</Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="import">
          <Card>
            <CardHeader><CardTitle>Import key into Horcrux shards</CardTitle><CardDescription>Accepts a 32-byte hex private key, derives address, splits into shards/shares, and discards the key. The key is not stored and not returned; send only to this local backend. Detect removable drives and choose per-share destinations/passwords.</CardDescription></CardHeader>
            <CardContent className="space-y-4">
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="space-y-2 lg:col-span-2"><Label>Label</Label><Input value={label} onChange={(e)=>setLabel(e.target.value)} /></div>
                <div className="space-y-2"><Label>Chain</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={chain} onChange={(e)=>setChain(e.target.value as any)}><option value="solana">Solana</option><option value="cosmos">Cosmos</option></select></div>
                <div className="space-y-2"><Label>Network</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={network} onChange={(e)=>setNetwork(e.target.value)}>{networksFor(chain).map(n=><option key={n} value={n}>{n}</option>)}</select></div>
              </div>
              <div className="space-y-2"><Label>Private key hex</Label><Input className="font-mono" type="password" value={importKey} onChange={(e)=>setImportKey(e.target.value)} placeholder="0x..." /></div>
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="space-y-2"><Label>Mode</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={effectiveMode} disabled={chain==="cosmos"} onChange={(e)=>setMode(e.target.value as any)}><option value="sss">SSS</option><option value="frost">FROST MPC</option></select></div>
                <div className="space-y-2"><Label>Threshold</Label><Input value={threshold} onChange={(e)=>setThreshold(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares</Label><Input value={shares} onChange={(e)=>setShares(e.target.value)} /></div>
                <div className="space-y-2"><Label>Shares count (effective)</Label><Input value={String(nShares)} readOnly className="opacity-80" /></div>
              </div>

              <DriveDestinationPlanner
                shares={nShares}
                prefix={prefix}
                outDir={outDir}
                onOutDirChange={setOutDir}
                passwords={sharePasswords}
                onPasswordsChange={setSharePasswords}
                destinations={shareDestinations}
                onDestinationsChange={setShareDestinations}
              />

              <Button onClick={doImport} disabled={busy || !label.trim() || !importKey.trim()}>{busy?"Importing…":"Import and split"}</Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="connect" className="space-y-4">
          <Alert variant="info"><AlertTitle>Browser-connected wallets are read-only</AlertTitle><AlertDescription>They can be viewed for on-chain balance/history in this console, but <strong>they are not Horcrux-protected</strong> and cannot sign via Horcrux shard/MPC flows. No browser wallet private key is ever sent to the backend.</AlertDescription></Alert>
          <Card>
            <CardHeader><CardTitle>Solana browser wallet (view-only)</CardTitle><CardDescription>Connect a browser extension wallet to capture its public address, then save as a read-only profile.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="flex flex-wrap items-center gap-3">
                <WalletMultiButton />
                {browserPk && <Badge variant="info">Connected: {short(browserPk)}</Badge>}
              </div>
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3 items-end">
                <div className="space-y-2 lg:col-span-2"><Label>Label</Label><Input value={cLabel} onChange={(e)=>setCLabel(e.target.value)} placeholder="e.g. Phantom (read-only)" /></div>
                <div className="space-y-2"><Label>Network</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={cNetwork} onChange={(e)=>setCNetwork(e.target.value)}><option value="devnet">devnet</option><option value="localnet">localnet</option></select></div>
                <div><Button variant="secondary" onClick={() => { setCChain("solana"); if (browserPk) setCAddress(browserPk); if (!cLabel) setCLabel("Solana browser wallet"); }} disabled={!browserPk}>Use connected address</Button></div>
              </div>
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Chain</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={cChain} onChange={(e)=>{setCChain(e.target.value as any); setCNetwork(e.target.value==="solana"?"devnet":"local");}}><option value="solana">Solana</option><option value="cosmos">Cosmos</option></select></div>
                <div className="space-y-2"><Label>Address</Label><Input className="font-mono" value={cAddress} onChange={(e)=>setCAddress(e.target.value)} placeholder="Public address" /></div>
              </div>
              <Button onClick={saveConnected} disabled={busy || !cAddress.trim()}>{busy?"Saving…":"Save read-only connected wallet"}</Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="watch">
          <Card>
            <CardHeader><CardTitle>Add watch-only address</CardTitle><CardDescription>Track any public address for on-chain views. Not Horcrux-protected.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
                <div className="space-y-2 lg:col-span-2"><Label>Label</Label><Input value={wLabel} onChange={(e)=>setWLabel(e.target.value)} /></div>
                <div className="space-y-2"><Label>Chain</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={wChain} onChange={(e)=>{setWChain(e.target.value as any); setWNetwork(e.target.value==="solana"?"devnet":"local");}}><option value="solana">Solana</option><option value="cosmos">Cosmos</option></select></div>
                <div className="space-y-2"><Label>Network</Label><select className="h-10 w-full rounded-lg border border-border bg-surface-2 px-3 text-sm" value={wNetwork} onChange={(e)=>setWNetwork(e.target.value)}>{networksFor(wChain).map(n=><option key={n} value={n}>{n}</option>)}</select></div>
              </div>
              <div className="space-y-2"><Label>Public address</Label><Input className="font-mono" value={wAddress} onChange={(e)=>setWAddress(e.target.value)} /></div>
              <Button onClick={saveWatch} disabled={busy || !wAddress.trim()}>{busy?"Saving…":"Add watch-only address"}</Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
