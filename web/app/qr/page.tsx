"use client";

import { useState } from "react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

function Frames({ frames }: { frames?: string[] }) {
  if (!frames || frames.length === 0) return null;
  return (
    <div className="space-y-2">
      <div className="text-sm text-muted">{frames.length} frame(s)</div>
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
        {frames.map((b64, i) => (
          <div key={i} className="border border-border rounded-lg p-2 bg-white/95 flex flex-col items-center gap-1">
            <img src={`data:image/png;base64,${b64}`} alt={`QR frame ${i + 1}`} className="w-full h-auto" />
            <div className="text-[10px] text-black/60">frame {i + 1}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

async function fileToBase64(file: File): Promise<string> {
  const buf = await file.arrayBuffer();
  const bytes = new Uint8Array(buf);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

export default function QrPage() {
  const [tab, setTab] = useState("shard");

  // shard export
  const [shardPath, setShardPath] = useState("");
  const [exportFrames, setExportFrames] = useState<string[]>([]);
  const [exportErr, setExportErr] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState(false);

  // shard import
  const [importPath, setImportPath] = useState<string | null>(null);
  const [importErr, setImportErr] = useState<string | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  // 5-step
  const [dir, setDir] = useState("qr");
  const [groupDir, setGroupDir] = useState("mpc");
  const [messageHex, setMessageHex] = useState("");
  const [session, setSession] = useState("");
  const [requestRes, setRequestRes] = useState<any>(null);
  const [commitSharePath, setCommitSharePath] = useState("mpc/mpc-share-1.key");
  const [commitPassword, setCommitPassword] = useState("");
  const [commitNoncePath, setCommitNoncePath] = useState("");
  const [commitRes, setCommitRes] = useState<any>(null);
  const [packageRes, setPackageRes] = useState<any>(null);
  const [shareSharePath, setShareSharePath] = useState("mpc/mpc-share-1.key");
  const [sharePassword, setSharePassword] = useState("");
  const [shareNoncePath, setShareNoncePath] = useState("");
  const [shareRes, setShareRes] = useState<any>(null);
  const [finalizeRes, setFinalizeRes] = useState<any>(null);
  const [flowErr, setFlowErr] = useState<string | null>(null);
  const [flowBusy, setFlowBusy] = useState<string | null>(null);

  const doExport = async () => {
    setExportBusy(true); setExportErr(null); setExportFrames([]);
    try {
      const res = await api.post<{ frames: string[] }>("/shard-qr", { path: shardPath.trim() });
      setExportFrames(res.frames || []);
    } catch (e: any) { setExportErr(e?.message || "Export failed"); }
    finally { setExportBusy(false); }
  };

  const doImport = async (file: File) => {
    setImportBusy(true); setImportErr(null); setImportPath(null);
    try {
      const b64 = await fileToBase64(file);
      const res = await api.post<{ path: string }>("/shard-qr-import", { png_base64: b64 });
      setImportPath(res.path);
    } catch (e: any) { setImportErr(e?.message || "Import failed"); }
    finally { setImportBusy(false); }
  };

  const step = async (name: string, fn: () => Promise<void>) => {
    setFlowBusy(name); setFlowErr(null);
    try { await fn(); } catch (e: any) { setFlowErr(e?.data?.error || e?.message || `${name} failed`); }
    finally { setFlowBusy(null); }
  };

  return (
    <div className="space-y-5 max-w-5xl">
      <div>
        <h1 className="text-xl font-semibold">QR Air-Gap Flows</h1>
        <p className="text-muted text-sm mt-1">Export/import shard files as QR frames and run the 5-step air-gapped FROST MPC signing flow (request → commit → package → share → finalize). Frames and transport files are written/read on the local backend machine.</p>
      </div>

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="shard">Shard QR Export/Import</TabsTrigger>
          <TabsTrigger value="mpc">5-Step MPC Flow</TabsTrigger>
        </TabsList>

        <TabsContent value="shard" className="space-y-4">
          <Card>
            <CardHeader><CardTitle>Export shard/share as QR</CardTitle><CardDescription>Reads a local file and returns QR frame PNGs (HX3) for offline transfer/display.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-2"><Label>Local file path</Label><Input value={shardPath} onChange={(e)=>setShardPath(e.target.value)} placeholder="shards/shard-1.hx or mpc/mpc-share-1.key" /></div>
              <Button onClick={doExport} disabled={exportBusy || !shardPath.trim()}>{exportBusy ? "Encoding…" : "Generate QR frames"}</Button>
              {exportErr && <Alert variant="danger"><AlertTitle>Error</AlertTitle><AlertDescription>{exportErr}</AlertDescription></Alert>}
              <Frames frames={exportFrames} />
            </CardContent>
          </Card>
          <Card>
            <CardHeader><CardTitle>Import shard/share from QR PNG</CardTitle><CardDescription>Upload a single-frame shard QR PNG image. The backend decodes it and saves the payload to a temp file path on this machine.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <input type="file" accept="image/png" onChange={(e)=>{ const f=e.target.files?.[0]; if(f) doImport(f); }} className="text-sm text-muted" />
              {importBusy && <div className="text-sm text-muted">Decoding…</div>}
              {importErr && <Alert variant="danger"><AlertTitle>Import failed</AlertTitle><AlertDescription>{importErr}</AlertDescription></Alert>}
              {importPath && <Alert variant="ok"><AlertTitle>Imported to local temp file</AlertTitle><AlertDescription className="font-mono break-all">{importPath}</AlertDescription></Alert>}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="mpc" className="space-y-4">
          <Alert variant="info"><AlertTitle>Air-gap notes</AlertTitle><AlertDescription>Steps read/write HX3 transport files in the specified directory on the backend host (e.g. <code>request.hx3</code>, <code>commit-&lt;id&gt;.hx3</code>, <code>package.hx3</code>, <code>share-&lt;id&gt;.hx3</code>). Keep nonce files local to the committing participant until share is produced. Commit/share require share passwords (used only for decrypting share/nonce material locally).</AlertDescription></Alert>
          {flowErr && <Alert variant="danger"><AlertTitle>Flow error</AlertTitle><AlertDescription className="break-all">{flowErr}</AlertDescription></Alert>}

          <Card>
            <CardHeader><CardTitle>1) Request (coordinator)</CardTitle><CardDescription>Create a signing request from group public key and message bytes (hex). Writes request transport and returns QR frames.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Group directory</Label><Input value={groupDir} onChange={(e)=>setGroupDir(e.target.value)} /></div>
                <div className="space-y-2"><Label>Output/transport dir</Label><Input value={dir} onChange={(e)=>setDir(e.target.value)} /></div>
              </div>
              <div className="space-y-2"><Label>Message hex (serialized message bytes to sign)</Label><Textarea rows={3} value={messageHex} onChange={(e)=>setMessageHex(e.target.value)} placeholder="hex…" /></div>
              <Button onClick={() => step("request", async () => { const r = await api.post<any>("/qr/request", { group_dir: groupDir, message_hex: messageHex, out_dir: dir }); setRequestRes(r); setSession(r.session || ""); })} disabled={flowBusy!==null}>{flowBusy==="request"?"Working…":"Create request"}</Button>
              {requestRes && <div className="space-y-2"><div className="text-xs text-muted">Session: <span className="font-mono text-text">{requestRes.session}</span></div><Frames frames={requestRes.frames} /></div>}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle>2) Commit (participant, offline side)</CardTitle><CardDescription>Reads request from dir, produces commitment + local nonce file, writes commit transport.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Transport dir (has request)</Label><Input value={dir} onChange={(e)=>setDir(e.target.value)} /></div>
                <div className="space-y-2"><Label>Share path</Label><Input value={commitSharePath} onChange={(e)=>setCommitSharePath(e.target.value)} /></div>
                <div className="space-y-2"><Label>Share password</Label><Input type="password" value={commitPassword} onChange={(e)=>setCommitPassword(e.target.value)} /></div>
                <div className="space-y-2"><Label>Nonce path (optional)</Label><Input value={commitNoncePath} onChange={(e)=>setCommitNoncePath(e.target.value)} placeholder="auto if blank" /></div>
              </div>
              <Button onClick={() => step("commit", async () => { const r = await api.post<any>("/qr/commit", { dir, share_path: commitSharePath, password: commitPassword, nonce_path: commitNoncePath || undefined, out_dir: dir }); setCommitRes(r); if (r.nonce_path) setShareNoncePath(r.nonce_path); })} disabled={flowBusy!==null}>{flowBusy==="commit"?"Working…":"Produce commitment"}</Button>
              {commitRes && <div className="space-y-2 text-sm"><div className="text-xs text-muted">Participant: <span className="font-mono text-text">{commitRes.participant_id}</span> · Nonce: <span className="font-mono text-text break-all">{commitRes.nonce_path}</span></div>{commitRes.note && <Alert variant="warn"><AlertDescription>{commitRes.note}</AlertDescription></Alert>}<Frames frames={commitRes.frames} /></div>}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle>3) Package (coordinator)</CardTitle><CardDescription>Collect commitments in dir and build signing package transport.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-2 max-w-md"><Label>Transport dir (has request + commit-*.hx3)</Label><Input value={dir} onChange={(e)=>setDir(e.target.value)} /></div>
              <Button onClick={() => step("package", async () => { const r = await api.post<any>("/qr/package", { dir, out_dir: dir }); setPackageRes(r); })} disabled={flowBusy!==null}>{flowBusy==="package"?"Working…":"Build package"}</Button>
              {packageRes && <div className="space-y-2 text-sm"><div className="text-xs text-muted">Participants: <span className="font-mono text-text">{(packageRes.participants||[]).join(", ")}</span></div><Frames frames={packageRes.frames} /></div>}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle>4) Share (participant, offline side)</CardTitle><CardDescription>Reads request+package, uses local nonce and share to produce signature share; deletes nonce file on success.</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="grid sm:grid-cols-2 gap-3">
                <div className="space-y-2"><Label>Transport dir (has request + package)</Label><Input value={dir} onChange={(e)=>setDir(e.target.value)} /></div>
                <div className="space-y-2"><Label>Share path</Label><Input value={shareSharePath} onChange={(e)=>setShareSharePath(e.target.value)} /></div>
                <div className="space-y-2"><Label>Share password</Label><Input type="password" value={sharePassword} onChange={(e)=>setSharePassword(e.target.value)} /></div>
                <div className="space-y-2"><Label>Nonce path (from commit step)</Label><Input value={shareNoncePath} onChange={(e)=>setShareNoncePath(e.target.value)} /></div>
              </div>
              <Button onClick={() => step("share", async () => { const r = await api.post<any>("/qr/share", { dir, share_path: shareSharePath, password: sharePassword, nonce_path: shareNoncePath, out_dir: dir }); setShareRes(r); })} disabled={flowBusy!==null}>{flowBusy==="share"?"Working…":"Produce signature share"}</Button>
              {shareRes && <div className="space-y-2 text-sm"><div className="text-xs text-muted">Participant: <span className="font-mono text-text">{shareRes.participant_id}</span></div><Frames frames={shareRes.frames} /></div>}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle>5) Finalize (coordinator)</CardTitle><CardDescription>Collect signature shares in dir and produce final signature (and raw tx base58 when request message was a Solana message).</CardDescription></CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-2 max-w-md"><Label>Transport dir (has request + package + share-*.hx3)</Label><Input value={dir} onChange={(e)=>setDir(e.target.value)} /></div>
              <Button onClick={() => step("finalize", async () => { const r = await api.post<any>("/qr/finalize", { dir }); setFinalizeRes(r); })} disabled={flowBusy!==null}>{flowBusy==="finalize"?"Working…":"Finalize signature"}</Button>
              {finalizeRes && (
                <div className="space-y-2 text-sm">
                  <div className="grid sm:grid-cols-2 gap-2">
                    {Object.entries(finalizeRes).map(([k,v]) => (
                      <div key={k} className="rounded-md border border-border bg-surface-2/40 px-3 py-2"><div className="text-xs text-faint mb-1">{k}</div><div className="font-mono text-xs break-all whitespace-pre-wrap">{typeof v==="string"?v:JSON.stringify(v)}</div></div>
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
