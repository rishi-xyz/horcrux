import { useEffect, useState } from "react";
import { api } from "../api";
import type { LogEntry } from "../types";

const KIND_BADGE: Record<string, string> = {
  decrypt_ok: "ok",
  decrypt_fail: "fail",
  blocked: "blocked",
  signed: "signed",
};

export function LogPage() {
  const [tail, setTail] = useState("");
  const [entries, setEntries] = useState<LogEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const q = tail ? "?tail=" + encodeURIComponent(tail) : "";
      const res = await api<{ entries: LogEntry[] }>("/log" + q);
      setEntries(res.entries);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <section className="panel">
      <header className="panel-head">
        <h1>Access log</h1>
        <p>Read-only, append-only. Every shard decryption attempt this machine has recorded.</p>
      </header>
      <div className="toolbar">
        <label className="field-inline">
          Tail
          <input
            type="number"
            min={1}
            placeholder="all"
            style={{ width: 90 }}
            value={tail}
            onChange={(e) => setTail(e.target.value)}
          />
        </label>
        <button className="btn btn-outline" type="button" onClick={load} disabled={loading}>
          Reload
        </button>
      </div>
      <div className="result">
        {error && <div className="banner banner-error">{error}</div>}
        {loading && !entries && <p>Loading…</p>}
        {entries && entries.length === 0 && <p>No access log entries yet.</p>}
        {entries && entries.length > 0 && (
          <table className="data">
            <thead>
              <tr>
                <th>Time (UTC)</th>
                <th>Attempt</th>
                <th>Shard</th>
                <th>Kind</th>
              </tr>
            </thead>
            <tbody>
              {entries
                .slice()
                .reverse()
                .map((e, i) => (
                  <tr key={i}>
                    <td>{e.ts_utc}</td>
                    <td>{e.attempt}</td>
                    <td>{e.shard_id ?? "—"}</td>
                    <td>
                      <span className={"badge badge-" + (KIND_BADGE[e.kind] || "ok")}>{e.kind}</span>
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
