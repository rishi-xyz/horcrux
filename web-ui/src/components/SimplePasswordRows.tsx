import { useEffect } from "react";

/// One password field per currently-listed shard/share path (Sign / MPC
/// Sign), labeled by the path so it's unambiguous which password goes with
/// which file. Same fix as `GuardianDestRows`: a single shared password
/// couldn't reconstruct shards split with per-guardian passwords.
export function SimplePasswordRows({
  labels,
  passwords,
  setPasswords,
}: {
  labels: string[];
  passwords: string[];
  setPasswords: (p: string[]) => void;
}) {
  useEffect(() => {
    if (passwords.length === labels.length) return;
    const next = passwords.slice(0, labels.length);
    while (next.length < labels.length) next.push("");
    setPasswords(next);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [labels.length]);

  if (labels.length === 0) {
    return <p className="guardian-list-empty">nothing to fill in yet — list shard/share paths above</p>;
  }

  return (
    <div className="guardian-list">
      {labels.map((label, i) => (
        <div className="guardian-row guardian-row-simple" key={i}>
          <span className="guardian-label" title={label}>
            {label}
          </span>
          <input
            type="password"
            className="guardian-password"
            autoComplete="off"
            value={passwords[i] || ""}
            onChange={(e) => {
              const next = passwords.slice();
              next[i] = e.target.value;
              setPasswords(next);
            }}
            required
          />
        </div>
      ))}
    </div>
  );
}

/// Splits a textarea's value into non-empty, trimmed lines — the shared
/// "one path per line" convention used by Sign/MPC-Sign/Verify.
export function lines(value: string): string[] {
  return value
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}
