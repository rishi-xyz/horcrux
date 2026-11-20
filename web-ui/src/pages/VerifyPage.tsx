import { useState } from "react";
import { api } from "../api";
import { lines } from "../components/SimplePasswordRows";
import { QrScanner } from "../components/QrScanner";
import type { VerifyReport } from "../types";

export function VerifyPage() {
  const [files, setFiles] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    reports: VerifyReport[];
    consistency_error: string | null;
    all_ok: boolean;
  } | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await api<{
        reports: VerifyReport[];
        consistency_error: string | null;
        all_ok: boolean;
      }>("/verify", {
        method: "POST",
        body: { files: lines(files), password: password || null },
      });
      setResult(res);
    } catch (e2) {
      setError(e2 instanceof Error ? e2.message : String(e2));
    } finally {
      setBusy(false);
    }
  }

  function appendPath(path: string) {
    setFiles((prev) => (prev && !prev.endsWith("\n") ? prev + "\n" + path : prev + path));
  }

  return (
    <section className="panel">
      <header className="panel-head">
        <h1>Verify</h1>
        <p>Structural integrity check for shard/share files. Never decrypts into the clear, never touches the access log.</p>
      </header>
      <form className="form-grid" onSubmit={submit}>
        <label className="field">
          <span>Files (one path per line)</span>
          <textarea
            rows={4}
            placeholder={"shards/shard-1.hx\nshards/shard-2.hx"}
            required
            value={files}
            onChange={(e) => setFiles(e.target.value)}
          />
        </label>
        <QrScanner onImport={appendPath} />
        <label className="field">
          <span>
            Password <em>(optional — checks the AES-GCM auth tag)</em>
          </span>
          <input
            type="password"
            autoComplete="off"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        <div className="form-actions">
          <button className="btn btn-primary" type="submit" disabled={busy}>
            Verify files
          </button>
        </div>
      </form>
      <div className="result">
        {error && <div className="banner banner-error">{error}</div>}
        {result && (
          <>
            {result.consistency_error ? (
              <div className="banner banner-error">Inconsistent set: {result.consistency_error}</div>
            ) : (
              <div className={"banner " + (result.all_ok ? "banner-ok" : "banner-error")}>
                {result.all_ok ? "All files verified." : "Verification failed for one or more files."}
              </div>
            )}
            <table className="data">
              <thead>
                <tr>
                  <th>Path</th>
                  <th>Kind</th>
                  <th>t / n</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {result.reports.map((r, i) => (
                  <tr key={i}>
                    <td>{r.path}</td>
                    <td>{r.kind ? r.kind.toUpperCase() : "invalid"}</td>
                    <td>{r.threshold != null ? `${r.threshold} / ${r.shares}` : "—"}</td>
                    <td>
                      <span className={"badge badge-" + (r.ok ? "ok" : "fail")}>{r.ok ? "ok" : "fail"}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </div>
    </section>
  );
}
