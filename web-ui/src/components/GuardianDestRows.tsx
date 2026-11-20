import { useEffect } from "react";
import type { DestChoice, DriveInfo, Guardian } from "../types";

/// Per-guardian destination + password rows for a split (Init / MPC Split).
/// A single shared password/directory here was the exact bug fixed in the
/// TUI: shards created with per-guardian passwords couldn't be
/// reconstructed from a UI that only ever asked for one. `guardians` is
/// grown/shrunk to `count` by the parent (see `useGuardianCount` below),
/// preserving already-typed values at surviving positions.
export function GuardianDestRows({
  guardians,
  setGuardians,
  drives,
}: {
  guardians: Guardian[];
  setGuardians: (g: Guardian[]) => void;
  drives: DriveInfo[];
}) {
  function update(i: number, patch: Partial<Guardian>) {
    const next = guardians.slice();
    next[i] = { ...next[i], ...patch };
    setGuardians(next);
  }

  if (guardians.length === 0) {
    return <p className="guardian-list-empty">nothing to fill in yet — set "Shares" above</p>;
  }

  return (
    <div className="guardian-list">
      {guardians.map((g, i) => {
        const selectValue =
          g.dest.kind === "this-device" ? "" : g.dest.kind === "custom" ? "custom" : "drive:" + g.dest.index;
        return (
          <div key={i} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <div className="guardian-row">
              <span className="guardian-label">Guardian {i + 1}</span>
              <select
                className="guardian-dest"
                value={selectValue}
                onChange={(e) => {
                  const v = e.target.value;
                  if (v === "") update(i, { dest: { kind: "this-device" } });
                  else if (v === "custom") update(i, { dest: { kind: "custom", path: "" } });
                  else update(i, { dest: { kind: "drive", index: Number(v.split(":")[1]) } });
                }}
              >
                <option value="">This device</option>
                {drives.map((d, idx) => (
                  <option key={idx} value={"drive:" + idx}>
                    USB: {d.name} ({(d.available_bytes / 1e9).toFixed(1)} GB free)
                  </option>
                ))}
                <option value="custom">Custom path…</option>
              </select>
              <input
                type="password"
                className="guardian-password"
                autoComplete="off"
                placeholder="password"
                value={g.password}
                onChange={(e) => update(i, { password: e.target.value })}
                required
              />
            </div>
            {g.dest.kind === "custom" && (
              <input
                type="text"
                className="guardian-custom-path"
                style={{ marginLeft: 98 }}
                placeholder="/path/to/directory"
                value={g.dest.path}
                onChange={(e) => update(i, { dest: { kind: "custom", path: e.target.value } })}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

/// Keeps `guardians` sized to `count`, adding default (this-device, empty
/// password) rows or truncating at the tail — same convention as the TUI's
/// `sync_guardians`/`resize_with`.
export function useGuardianCount(
  count: number,
  guardians: Guardian[],
  setGuardians: (g: Guardian[]) => void
) {
  useEffect(() => {
    if (guardians.length === count) return;
    const next = guardians.slice(0, count);
    while (next.length < count) {
      next.push({ dest: { kind: "this-device" }, password: "" });
    }
    setGuardians(next);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [count]);
}

/// Resolves each guardian's destination into a directory string for the
/// `/init` or `/mpc-split` request body's `destinations` field: empty
/// string means "this device" (the shared `out_dir`), matching the Rust
/// handler's `resolve_destinations` convention. Returns an empty array
/// (rather than all-empty-strings) when every guardian is "this device",
/// so the server takes its simpler single-`out_dir` path.
export function resolveDestinations(guardians: Guardian[], drives: DriveInfo[]): string[] {
  const destinations = guardians.map((g) => {
    if (g.dest.kind === "this-device") return "";
    if (g.dest.kind === "custom") return g.dest.path.trim();
    const drive = drives[g.dest.index];
    return drive ? drive.mount_point + "/horcrux" : "";
  });
  return destinations.every((d) => d === "") ? [] : destinations;
}

export function guardianPasswords(guardians: Guardian[]): string[] {
  return guardians.map((g) => g.password);
}

export type { DestChoice };
