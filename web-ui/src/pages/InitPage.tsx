import { useState } from "react";
import { api } from "../api";
import { useDrives } from "../hooks/useDrives";
import { GuardianDestRows, guardianPasswords, resolveDestinations, useGuardianCount } from "../components/GuardianDestRows";
import { QrShow } from "../components/QrShow";
import type { Guardian } from "../types";

export function InitPage() {
  const [threshold, setThreshold] = useState(2);
  const [shares, setShares] = useState(3);
  const [generate, setGenerate] = useState(false);
  const [keyHex, setKeyHex] = useState("");
  const [outDir, setOutDir] = useState("shards");
  const [guardians, setGuardians] = useState<Guardian[]>([]);
  const { drives, rescan } = useDrives();
  useGuardianCount(shares, guardians, setGuardians);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{ generated_key_hex?: string; out_dir: string; paths: string[] } | null>(
    null
  );

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await api<{ generated_key_hex?: string; out_dir: string; paths: string[] }>("/init", {
        method: "POST",
        body: {
          threshold,
          shares,
          key_hex: keyHex || null,
          generate,
          out_dir: outDir,
          passwords: guardianPasswords(guardians),
          destinations: resolveDestinations(guardians, drives),
        },
      });
      setResult(res);
    } catch (e2) {
      setError(e2 instanceof Error ? e2.message : String(e2));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel">
      <header className="panel-head">
        <h1>Init — Mode A split</h1>
        <p>Split a private key into encrypted shard files with Shamir's Secret Sharing.</p>
      </header>
      <form className="form-grid" onSubmit={submit}>
        <div className="field-row">
          <label className="field">
            <span>Threshold</span>
            <input
              type="number"
              min={1}
              required
              value={threshold}
              onChange={(e) => setThreshold(Number(e.target.value))}
            />
          </label>
          <label className="field">
            <span>Shares</span>
            <input
              type="number"
              min={1}
              required
              value={shares}
              onChange={(e) => setShares(Math.max(0, Number(e.target.value) || 0))}
            />
          </label>
        </div>
        <label className="field checkbox-field">
          <input type="checkbox" checked={generate} onChange={(e) => setGenerate(e.target.checked)} />
          <span>
            Generate a disposable test key <em>(shown once — never use for real funds)</em>
          </span>
        </label>
        <label className="field">
          <span>
            Private key hex <em>(64 hex chars, optional 0x — disabled when generating)</em>
          </span>
          <input
            type="text"
            autoComplete="off"
            spellCheck={false}
            disabled={generate}
            value={keyHex}
            onChange={(e) => setKeyHex(e.target.value)}
          />
        </label>
        <label className="field">
          <span>
            Output directory <em>(used for any guardian left on "this device" below)</em>
          </span>
          <input type="text" value={outDir} onChange={(e) => setOutDir(e.target.value)} />
        </label>
        <div className="field">
          <span className="guardian-list-head">
            Guardians <em>(one destination + password per shard, distinct per guardian)</em>
            <button className="btn-inline" type="button" onClick={() => void rescan()}>
              rescan drives
            </button>
          </span>
          <GuardianDestRows guardians={guardians} setGuardians={setGuardians} drives={drives} />
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" type="submit" disabled={busy}>
            Split key
          </button>
        </div>
      </form>
      <div className="result">
        {error && <div className="banner banner-error">{error}</div>}
        {result && (
          <>
            {result.generated_key_hex && (
              <div className="banner banner-warn">
                Generated test key (shown once — never use for real funds):
                <div style={{ marginTop: 8 }}>
                  <code>0x{result.generated_key_hex}</code>
                </div>
              </div>
            )}
            <div className="banner banner-ok">
              Wrote {result.paths.length} shard(s) to {result.out_dir}
            </div>
            <ul className="file-list">
              {result.paths.map((p) => (
                <li key={p}>
                  {p}
                  <QrShow path={p} />
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </section>
  );
}
