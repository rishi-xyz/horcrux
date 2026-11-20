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

export function DriveDestinationPlanner(props: {
  shares: number;
  prefix: "shard" | "mpc";
  outDir: string;
  onOutDirChange: (v: string) => void;
  passwords: string[];
  onPasswordsChange: (v: string[]) => void;
  destinations: string[]; // per-share directory/mount (empty => outDir)
  onDestinationsChange: (v: string[]) => void;
}) {
  const { shares, prefix, outDir, onOutDirChange, passwords, onPasswordsChange, destinations, onDestinationsChange } = props;
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [drivesErr, setDrivesErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const loadDrives = async () => {
    setLoading(true); setDrivesErr(null);
    try {
      const res = await api.get<{ drives: DriveInfo[] }>("/drives");
      setDrives(res.drives || []);
    } catch (e: any) {
      setDrivesErr(e?.message || "Failed to detect drives");
      setDrives([]);
    } finally { setLoading(false); }
  };
  useEffect(() => { loadDrives(); }, []);

  // keep arrays sized
  useEffect(() => {
    const n = Math.max(0, shares || 0);
    if (passwords.length !== n) onPasswordsChange(Array.from({ length: n }, (_, i) => passwords[i] ?? ""));
    if (destinations.length !== n) onDestinationsChange(Array.from({ length: n }, (_, i) => destinations[i] ?? ""));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shares]);

  const setPw = (i: number, v: string) => {
    const next = passwords.slice(); next[i] = v; onPasswordsChange(next);
  };
  const setDest = (i: number, v: string) => {
    const next = destinations.slice(); next[i] = v; onDestinationsChange(next);
  };
  const applyDriveToAll = (mount: string) => {
    onDestinationsChange(Array.from({ length: Math.max(0, shares) }, () => mount));
  };

  const rows = useMemo(() => Array.from({ length: Math.max(0, shares || 0) }, (_, i) => i), [shares]);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-end gap-3">
        <div className="space-y-2 min-w-[260px] flex-1">
          <Label>Default output directory (fallback)</Label>
          <Input value={outDir} onChange={(e) => onOutDirChange(e.target.value)} placeholder="e.g. shards or mpc" />
          <p className="text-xs text-faint">Used for any share left as “Use output dir”. Files are written on the backend machine.</p>
        </div>
        <div className="flex items-center gap-2 pb-1">
          <Button type="button" variant="secondary" size="sm" onClick={loadDrives} disabled={loading}>{loading ? "Scanning…" : "Refresh drives"}</Button>
          {drives.length > 0 && (
            <select className="h-9 rounded-md border border-border bg-surface-2 px-2 text-xs" defaultValue="" onChange={(e)=>{ if(e.target.value !== "__noop__") { applyDriveToAll(e.target.value); e.target.value=""; } }}>
              <option value="__noop__">Apply drive to all…</option>
              {drives.map((d) => <option key={d.mount_point} value={d.mount_point}>{d.removable ? "● " : ""}{d.name || d.mount_point} ({d.mount_point})</option>)}
              <option value="">Use output dir for all</option>
            </select>
          )}
        </div>
      </div>

      {drivesErr && <Alert variant="warn"><AlertTitle>Drive detection</AlertTitle><AlertDescription>{drivesErr}</AlertDescription></Alert>}
      <div className="text-xs text-muted">
        {drives.length > 0 ? (
          <span>
            Detected drives (removable first): {drives.map(d => (
              <span key={d.mount_point} className="font-mono mr-2">
                {d.name ? `${d.name} → ${d.mount_point}` : d.mount_point}{d.removable ? " [removable]" : ""}
              </span>
            ))}
          </span>
        ) : (
          <span>No drives detected. Insert a pendrive and click Refresh, or use custom paths/output directory.</span>
        )}
      </div>

      <div className="space-y-2">
        <div className="grid grid-cols-12 gap-2 text-xs text-faint px-1">
          <div className="col-span-1">#</div>
          <div className="col-span-5">Destination (directory/mount)</div>
          <div className="col-span-3">Password</div>
          <div className="col-span-3">Will write</div>
        </div>
        {rows.map((i) => {
          const destDir = destinations[i] ?? "";
          const effectiveDir = destDir.trim() ? destDir.trim() : (outDir || (prefix === "shard" ? "shards" : "mpc"));
          const filename = `${prefix}-${i + 1}.hx`;
          const preview = joinPath(effectiveDir, filename);
          return (
            <div key={i} className="grid grid-cols-12 gap-2 items-start rounded-md border border-border bg-surface-2/30 p-2">
              <div className="col-span-1 pt-2 text-xs text-muted font-mono">{i + 1}</div>
              <div className="col-span-5 space-y-1">
                <div className="flex gap-2">
                  <select
                    className="h-9 w-[46%] rounded-md border border-border bg-bg px-2 text-xs"
                    value={drives.some(d => d.mount_point === destDir) ? destDir : (destDir ? "__custom__" : "")}
                    onChange={(e) => {
                      const v = e.target.value;
                      if (v === "__custom__") return; // keep current custom text
                      setDest(i, v);
                    }}
                  >
                    <option value="">Use output dir</option>
                    {drives.map((d) => (
                      <option key={d.mount_point} value={d.mount_point}>{d.name || "Drive"} — {d.mount_point}</option>
                    ))}
                    <option value="__custom__">Custom path…</option>
                  </select>
                  <Input
                    className="h-9 text-xs font-mono"
                    value={destDir}
                    onChange={(e) => setDest(i, e.target.value)}
                    placeholder="mount point or directory (blank = output dir)"
                  />
                </div>
              </div>
              <div className="col-span-3">
                <Input
                  className="h-9 text-xs"
                  type="password"
                  value={passwords[i] ?? ""}
                  onChange={(e) => setPw(i, e.target.value)}
                  placeholder={`password for ${filename}`}
                />
              </div>
              <div className="col-span-3 pt-2 text-[11px] font-mono text-faint break-all">{preview}</div>
            </div>
          );
        })}
      </div>
      <p className="text-xs text-faint">Tip: assign different removable drives per share for physical separation, or leave blank to write all to the output directory. Passwords are sent only to the local backend for encryption and are not stored in responses/logs/AI.</p>
    </div>
  );
}
