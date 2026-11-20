"use client";

import { useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api";
import type { DriveInfo } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

function joinPath(dir: string, name: string) {
  if (!dir) return name;
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith("/") || dir.endsWith("\\") ? dir + name : dir + sep + name;
}

export function ShardSourcePlanner(props: {
  count: number;
  onCountChange?: (n: number) => void;
  paths: string[];
  onPathsChange: (v: string[]) => void;
  passwords: string[];
  onPasswordsChange: (v: string[]) => void;
  defaultNamePrefix?: string; // e.g. shard / mpc-share
  allowCountEdit?: boolean;
  title?: string;
}) {
  const { count, paths, onPathsChange, passwords, onPasswordsChange, defaultNamePrefix = "shard", allowCountEdit = true, title } = props;
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [drivesErr, setDrivesErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const loadDrives = async () => {
    setLoading(true); setDrivesErr(null);
    try { const res = await api.get<{ drives: DriveInfo[] }>("/drives"); setDrives(res.drives || []); }
    catch (e: any) { setDrivesErr(e?.message || "Failed to detect drives"); setDrives([]); }
    finally { setLoading(false); }
  };
  useEffect(() => { loadDrives(); }, []);

  useEffect(() => {
    const n = Math.max(1, count || 1);
    if (paths.length !== n) onPathsChange(Array.from({ length: n }, (_, i) => paths[i] ?? ""));
    if (passwords.length !== n) onPasswordsChange(Array.from({ length: n }, (_, i) => passwords[i] ?? ""));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [count]);

  const setPath = (i: number, v: string) => { const next = paths.slice(); next[i] = v; onPathsChange(next); };
  const setPw = (i: number, v: string) => { const next = passwords.slice(); next[i] = v; onPasswordsChange(next); };

  const suggestFromDrive = (mount: string) => {
    const next = Array.from({ length: Math.max(1, count) }, (_, i) => paths[i] && !paths[i].includes(mount) && paths[i].trim() ? paths[i] : joinPath(mount, `${defaultNamePrefix}-${i + 1}.hx`));
    // better: fill sequentially
    onPathsChange(Array.from({ length: Math.max(1, count) }, (_, i) => joinPath(mount, `${defaultNamePrefix}-${i + 1}.hx`)));
  };
  const addRow = () => { onPathsChange([...paths, ""]); onPasswordsChange([...passwords, ""]); props.onCountChange?.(paths.length + 1); };
  const removeRow = (i: number) => {
    if (paths.length <= 1) return;
    onPathsChange(paths.filter((_, idx) => idx !== i));
    onPasswordsChange(passwords.filter((_, idx) => idx !== i));
    props.onCountChange?.(paths.length - 1);
  };

  const rows = useMemo(() => Array.from({ length: Math.max(1, count || 1) }, (_, i) => i), [count]);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          {title && <Label className="text-sm">{title}</Label>}
          <p className="text-xs text-faint">Select shard/share files from detected removable drives or enter paths manually. Passwords are per file (same order).</p>
        </div>
        <div className="flex items-center gap-2">
          <Button type="button" variant="secondary" size="sm" onClick={loadDrives} disabled={loading}>{loading ? "Scanning…" : "Refresh drives"}</Button>
          {drives.length > 0 && (
            <select className="h-9 rounded-md border border-border bg-surface-2 px-2 text-xs" defaultValue="" onChange={(e)=>{ if(e.target.value){ suggestFromDrive(e.target.value); e.target.value=""; } }}>
              <option value="">Fill paths from drive…</option>
              {drives.map((d) => <option key={d.mount_point} value={d.mount_point}>{d.name || "Drive"} — {d.mount_point}</option>)}
            </select>
          )}
          {allowCountEdit && <Button type="button" variant="outline" size="sm" onClick={addRow}>+ Add file</Button>}
        </div>
      </div>
      {drivesErr && <Alert variant="warn"><AlertTitle>Drive detection</AlertTitle><AlertDescription>{drivesErr}</AlertDescription></Alert>}
      <div className="text-xs text-muted">
        {drives.length > 0 ? <>Detected drives (removable first): {drives.map(d => <span key={d.mount_point} className="font-mono mr-2">{d.name ? `${d.name} → ${d.mount_point}` : d.mount_point}{d.removable ? " [removable]" : ""}</span>)}</> : <>No drives detected. Insert a pendrive and Refresh, or enter paths manually.</>}
      </div>

      <div className="space-y-2">
        <div className="grid grid-cols-12 gap-2 text-xs text-faint px-1">
          <div className="col-span-1">#</div>
          <div className="col-span-7">Shard/share file path</div>
          <div className="col-span-3">Password</div>
          <div className="col-span-1"></div>
        </div>
        {rows.map((i) => (
          <div key={i} className="grid grid-cols-12 gap-2 items-start rounded-md border border-border bg-surface-2/30 p-2">
            <div className="col-span-1 pt-2 text-xs text-muted font-mono">{i + 1}</div>
            <div className="col-span-7 space-y-1">
              <Input className="h-9 text-xs font-mono" value={paths[i] ?? ""} onChange={(e)=>setPath(i, e.target.value)} placeholder={`${defaultNamePrefix}-${i+1}.hx or /media/.../${defaultNamePrefix}-${i+1}.hx`} />
              {drives.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {drives.map((d) => (
                    <button key={d.mount_point} type="button" className="text-[10px] px-2 py-0.5 rounded border border-border hover:bg-surface-3 text-faint" onClick={() => setPath(i, joinPath(d.mount_point, `${defaultNamePrefix}-${i+1}.hx`))}>
                      Use {d.name || "drive"}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="col-span-3"><Input className="h-9 text-xs" type="password" value={passwords[i] ?? ""} onChange={(e)=>setPw(i, e.target.value)} placeholder="password" /></div>
            <div className="col-span-1 pt-1 flex justify-end">{allowCountEdit && paths.length > 1 && <Button type="button" variant="ghost" size="sm" className="h-8 px-2 text-faint" onClick={()=>removeRow(i)}>×</Button>}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
