import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, ApiError } from "../api";
import { useDrives } from "../hooks/useDrives";
import { GuardianDestRows, guardianPasswords, resolveDestinations, useGuardianCount } from "../components/GuardianDestRows";
import { lines, SimplePasswordRows } from "../components/SimplePasswordRows";
import { QrFileImport } from "../components/QrFileImport";
import { QrScanner } from "../components/QrScanner";
import { QrShow } from "../components/QrShow";
import { AuditModal } from "../components/AuditModal";
import type { Guardian, SignResult } from "../types";

export function MpcPage() {
  const { tab } = useParams();
  const navigate = useNavigate();
  const active = tab === "sign" ? "sign" : "split";

  return (
    <section className="panel">
      <header className="panel-head">
        <h1>MPC — Mode B</h1>
        <p>Dealer-split into FROST key shares, or sign with a threshold subset. The full key is never reconstructed on any machine. Solana only.</p>
      </header>
      <div className="tabbar">
        <button
          className={"tab" + (active === "split" ? " is-active" : "")}
          type="button"
          onClick={() => navigate("/mpc/split")}
        >
          Split
        </button>
        <button
          className={"tab" + (active === "sign" ? " is-active" : "")}
          type="button"
          onClick={() => navigate("/mpc/sign")}
        >
          Sign
        </button>
      </div>
      {active === "split" ? <MpcSplitForm /> : <MpcSignForm />}
    </section>
  );
}

function MpcSplitForm() {
  const [threshold, setThreshold] = useState(2);
  const [shares, setShares] = useState(3);
  const [generate, setGenerate] = useState(false);
  const [keyHex, setKeyHex] = useState("");
  const [outDir, setOutDir] = useState("mpc");
  const [guardians, setGuardians] = useState<Guardian[]>([]);
  const { drives, rescan } = useDrives();
  useGuardianCount(shares, guardians, setGuardians);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    generated_key_hex?: string;
    paths: string[];
    group_path: string;
  } | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await api<{ generated_key_hex?: string; paths: string[]; group_path: string }>("/mpc-split", {
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
    <>
      <form className="form-grid" onSubmit={submit}>
        <div className="field-row">
          <label className="field">
            <span>
              Threshold <em>(min 2)</em>
            </span>
            <input type="number" min={2} required value={threshold} onChange={(e) => setThreshold(Number(e.target.value))} />
          </label>
          <label className="field">
            <span>Shares</span>
            <input
              type="number"
              min={2}
              required
              value={shares}
              onChange={(e) => setShares(Math.max(0, Number(e.target.value) || 0))}
            />
          </label>
        </div>
        <label className="field checkbox-field">
          <input type="checkbox" checked={generate} onChange={(e) => setGenerate(e.target.checked)} />
          <span>
            Generate a disposable test key <em>(shown once)</em>
          </span>
        </label>
        <label className="field">
          <span>Private key hex</span>
          <input type="text" autoComplete="off" spellCheck={false} disabled={generate} value={keyHex} onChange={(e) => setKeyHex(e.target.value)} />
        </label>
        <label className="field">
          <span>
            Output directory <em>(used for any guardian left on "this device" below, and always for group.pub)</em>
          </span>
          <input type="text" value={outDir} onChange={(e) => setOutDir(e.target.value)} />
        </label>
        <div className="field">
          <span className="guardian-list-head">
            Guardians <em>(one destination + password per share, distinct per guardian)</em>
            <button className="btn-inline" type="button" onClick={() => void rescan()}>
              rescan drives
            </button>
          </span>
          <GuardianDestRows guardians={guardians} setGuardians={setGuardians} drives={drives} />
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" type="submit" disabled={busy}>
            Split key (FROST)
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
              Wrote {result.paths.length} key share(s). Group package: {result.group_path}
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
    </>
  );
}

function MpcSignForm() {
  const [shares, setShares] = useState("");
  const [groupDir, setGroupDir] = useState("mpc");
  const [passwords, setPasswords] = useState<string[]>([]);
  const [to, setTo] = useState("");
  const [lamports, setLamports] = useState("");
  const [blockhash, setBlockhash] = useState("");
  const [broadcast, setBroadcast] = useState(false);
  const [rpcUrl, setRpcUrl] = useState("");

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SignResult | null>(null);
  const [blocked, setBlocked] = useState<string[] | null>(null);

  function appendShare(path: string) {
    setShares((prev) => (prev && !prev.endsWith("\n") ? prev + "\n" + path : prev + path));
  }

  async function doSign(force: boolean) {
    setBusy(true);
    setError(null);
    setBlocked(null);
    try {
      const res = await api<SignResult>("/mpc-sign", {
        method: "POST",
        body: {
          shares: lines(shares),
          group_dir: groupDir,
          passwords,
          to,
          lamports: lamports ? Number(lamports) : null,
          blockhash: blockhash || null,
          broadcast,
          rpc_url: rpcUrl || null,
          force,
        },
      });
      setResult(res);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409 && e.data && typeof e.data === "object" && "reasons" in e.data) {
        setBlocked((e.data as { reasons: string[] }).reasons);
      } else {
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <form
        className="form-grid"
        onSubmit={(e) => {
          e.preventDefault();
          void doSign(false);
        }}
      >
        <label className="field">
          <span>Key share files (one path per line)</span>
          <textarea rows={3} placeholder={"mpc/mpc-1.hx\nmpc/mpc-2.hx"} required value={shares} onChange={(e) => setShares(e.target.value)} />
        </label>
        <QrFileImport onImport={appendShare} />
        <QrScanner onImport={appendShare} />
        <label className="field">
          <span>
            Group directory <em>(contains group.pub)</em>
          </span>
          <input type="text" value={groupDir} onChange={(e) => setGroupDir(e.target.value)} />
        </label>
        <div className="field">
          <span>
            Passwords <em>(one per share above, in the same order — distinct per guardian)</em>
          </span>
          <SimplePasswordRows labels={lines(shares)} passwords={passwords} setPasswords={setPasswords} />
        </div>
        <div className="field-row">
          <label className="field">
            <span>Recipient address (base58)</span>
            <input type="text" required value={to} onChange={(e) => setTo(e.target.value)} />
          </label>
          <label className="field">
            <span>Lamports</span>
            <input type="number" min={0} required value={lamports} onChange={(e) => setLamports(e.target.value)} />
          </label>
        </div>
        <label className="field">
          <span>
            Blockhash <em>(optional — fetched when broadcasting)</em>
          </span>
          <input type="text" value={blockhash} onChange={(e) => setBlockhash(e.target.value)} />
        </label>
        <div className="field-row">
          <label className="field checkbox-field">
            <input type="checkbox" checked={broadcast} onChange={(e) => setBroadcast(e.target.checked)} />
            <span>Broadcast to a Solana cluster</span>
          </label>
          <label className="field">
            <span>
              RPC URL <em>(optional override)</em>
            </span>
            <input type="text" placeholder="http://127.0.0.1:8899" value={rpcUrl} onChange={(e) => setRpcUrl(e.target.value)} />
          </label>
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" type="submit" disabled={busy}>
            Sign with FROST
          </button>
        </div>
      </form>
      <div className="result">
        {error && <div className="banner banner-error">{error}</div>}
        {result && (
          <>
            {result.warnings.length > 0 && (
              <div className="banner banner-warn">
                Audit warnings:
                <ul>
                  {result.warnings.map((w, i) => (
                    <li key={i}>{w}</li>
                  ))}
                </ul>
              </div>
            )}
            <table className="kv">
              <tbody>
                <tr>
                  <td>From</td>
                  <td>{result.from}</td>
                </tr>
                <tr>
                  <td>Signature</td>
                  <td>{result.signature}</td>
                </tr>
                <tr>
                  <td>Raw (base58)</td>
                  <td>{result.raw_base58}</td>
                </tr>
                {result.mined_signature && (
                  <tr>
                    <td>Mined</td>
                    <td>{result.mined_signature}</td>
                  </tr>
                )}
                {result.broadcast_error && (
                  <tr>
                    <td>Broadcast error</td>
                    <td>{result.broadcast_error}</td>
                  </tr>
                )}
              </tbody>
            </table>
          </>
        )}
      </div>
      {blocked && (
        <AuditModal
          blocking
          reasons={blocked}
          onForce={() => void doSign(true)}
          onContinue={() => void doSign(true)}
          onCancel={() => setBlocked(null)}
        />
      )}
    </>
  );
}
