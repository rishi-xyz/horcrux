import { useState } from "react";
import { api, ApiError } from "../api";
import { lines, SimplePasswordRows } from "../components/SimplePasswordRows";
import { QrFileImport } from "../components/QrFileImport";
import { QrScanner } from "../components/QrScanner";
import { AuditModal } from "../components/AuditModal";
import type { SignResult } from "../types";

type Chain = "solana" | "bitcoin" | "cosmos";

export function SignPage() {
  const [shards, setShards] = useState("");
  const [passwords, setPasswords] = useState<string[]>([]);
  const [chain, setChain] = useState<Chain>("solana");
  const [to, setTo] = useState("");
  const [broadcast, setBroadcast] = useState(false);
  const [rpcUrl, setRpcUrl] = useState("");

  // Solana
  const [lamports, setLamports] = useState("");
  const [blockhash, setBlockhash] = useState("");

  // Bitcoin
  const [rpcUser, setRpcUser] = useState("");
  const [rpcPassword, setRpcPassword] = useState("");
  const [utxos, setUtxos] = useState("");
  const [amountSat, setAmountSat] = useState("");
  const [changeAddress, setChangeAddress] = useState("");
  const [feeSat, setFeeSat] = useState("");

  // Cosmos
  const [chainId, setChainId] = useState("");
  const [accountNumber, setAccountNumber] = useState("");
  const [sequence, setSequence] = useState("");
  const [amount, setAmount] = useState("");
  const [denom, setDenom] = useState("");
  const [gas, setGas] = useState("");
  const [fee, setFee] = useState("");
  const [feeDenom, setFeeDenom] = useState("");
  const [memo, setMemo] = useState("");
  const [timeoutHeight, setTimeoutHeight] = useState("");

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SignResult | null>(null);
  const [blocked, setBlocked] = useState<string[] | null>(null);

  function appendShard(path: string) {
    setShards((prev) => (prev && !prev.endsWith("\n") ? prev + "\n" + path : prev + path));
  }

  function body(force: boolean) {
    const shared = {
      shards: lines(shards),
      passwords,
      chain,
      to,
      broadcast,
      rpc_url: rpcUrl || null,
      force,
    };
    if (chain === "solana") {
      return { ...shared, lamports: lamports ? Number(lamports) : null, blockhash: blockhash || null };
    }
    if (chain === "bitcoin") {
      return {
        ...shared,
        rpc_user: rpcUser || null,
        rpc_password: rpcPassword || null,
        utxo: lines(utxos),
        amount_sat: amountSat ? Number(amountSat) : null,
        change_address: changeAddress || null,
        fee_sat: feeSat ? Number(feeSat) : null,
      };
    }
    return {
      ...shared,
      chain_id: chainId || null,
      account_number: accountNumber ? Number(accountNumber) : null,
      sequence: sequence ? Number(sequence) : null,
      amount: amount ? Number(amount) : null,
      denom: denom || null,
      gas: gas ? Number(gas) : null,
      fee: fee ? Number(fee) : null,
      fee_denom: feeDenom || null,
      memo,
      timeout_height: timeoutHeight ? Number(timeoutHeight) : null,
    };
  }

  async function doSign(force: boolean) {
    setBusy(true);
    setError(null);
    setBlocked(null);
    try {
      const res = await api<SignResult>("/sign", { method: "POST", body: body(force) });
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
    <section className="panel">
      <header className="panel-head">
        <h1>Sign — Mode A</h1>
        <p>Reconstruct the key from a threshold of shards, sign a transaction offline, zeroize, optionally broadcast.</p>
      </header>
      <form
        className="form-grid"
        onSubmit={(e) => {
          e.preventDefault();
          void doSign(false);
        }}
      >
        <label className="field">
          <span>Shard files (one path per line)</span>
          <textarea
            rows={3}
            placeholder={"shards/shard-1.hx\nshards/shard-2.hx"}
            required
            value={shards}
            onChange={(e) => setShards(e.target.value)}
          />
        </label>
        <QrFileImport onImport={appendShard} />
        <QrScanner onImport={appendShard} />
        <div className="field">
          <span>
            Passwords <em>(one per shard above, in the same order — distinct per guardian)</em>
          </span>
          <SimplePasswordRows labels={lines(shards)} passwords={passwords} setPasswords={setPasswords} />
        </div>

        <label className="field">
          <span>Chain</span>
          <select value={chain} onChange={(e) => setChain(e.target.value as Chain)}>
            <option value="solana">Solana</option>
            <option value="bitcoin">Bitcoin (Taproot)</option>
            <option value="cosmos">Cosmos SDK</option>
          </select>
        </label>

        <label className="field">
          <span>Recipient address {chain === "solana" ? "(base58)" : "(bech32)"}</span>
          <input type="text" required value={to} onChange={(e) => setTo(e.target.value)} />
        </label>

        {chain === "solana" && (
          <>
            <div className="field-row">
              <label className="field">
                <span>Lamports</span>
                <input type="number" min={0} required value={lamports} onChange={(e) => setLamports(e.target.value)} />
              </label>
              <label className="field">
                <span>
                  Blockhash <em>(leave empty to fetch when broadcasting)</em>
                </span>
                <input type="text" value={blockhash} onChange={(e) => setBlockhash(e.target.value)} />
              </label>
            </div>
          </>
        )}

        {chain === "bitcoin" && (
          <>
            <label className="field">
              <span>
                UTXOs <em>(one per line, as txid:vout:amount-sat)</em>
              </span>
              <textarea rows={2} value={utxos} onChange={(e) => setUtxos(e.target.value)} />
            </label>
            <div className="field-row">
              <label className="field">
                <span>Amount (sat)</span>
                <input type="number" min={0} required value={amountSat} onChange={(e) => setAmountSat(e.target.value)} />
              </label>
              <label className="field">
                <span>Fee (sat)</span>
                <input type="number" min={0} required value={feeSat} onChange={(e) => setFeeSat(e.target.value)} />
              </label>
            </div>
            <label className="field">
              <span>
                Change address <em>(defaults to the sender's own address)</em>
              </span>
              <input type="text" value={changeAddress} onChange={(e) => setChangeAddress(e.target.value)} />
            </label>
            <div className="field-row">
              <label className="field">
                <span>
                  Bitcoin Core RPC user <em>(optional)</em>
                </span>
                <input type="text" value={rpcUser} onChange={(e) => setRpcUser(e.target.value)} />
              </label>
              <label className="field">
                <span>
                  Bitcoin Core RPC password <em>(optional)</em>
                </span>
                <input type="password" autoComplete="off" value={rpcPassword} onChange={(e) => setRpcPassword(e.target.value)} />
              </label>
            </div>
          </>
        )}

        {chain === "cosmos" && (
          <>
            <div className="field-row">
              <label className="field">
                <span>Chain id</span>
                <input type="text" placeholder="cosmoshub-4" required value={chainId} onChange={(e) => setChainId(e.target.value)} />
              </label>
              <label className="field">
                <span>Account number</span>
                <input type="number" min={0} required value={accountNumber} onChange={(e) => setAccountNumber(e.target.value)} />
              </label>
            </div>
            <div className="field-row">
              <label className="field">
                <span>Sequence</span>
                <input type="number" min={0} required value={sequence} onChange={(e) => setSequence(e.target.value)} />
              </label>
              <label className="field">
                <span>Amount</span>
                <input type="number" min={0} required value={amount} onChange={(e) => setAmount(e.target.value)} />
              </label>
            </div>
            <div className="field-row">
              <label className="field">
                <span>Denom</span>
                <input type="text" placeholder="uatom" required value={denom} onChange={(e) => setDenom(e.target.value)} />
              </label>
              <label className="field">
                <span>Gas</span>
                <input type="number" min={0} required value={gas} onChange={(e) => setGas(e.target.value)} />
              </label>
            </div>
            <div className="field-row">
              <label className="field">
                <span>Fee amount</span>
                <input type="number" min={0} required value={fee} onChange={(e) => setFee(e.target.value)} />
              </label>
              <label className="field">
                <span>
                  Fee denom <em>(defaults to denom)</em>
                </span>
                <input type="text" value={feeDenom} onChange={(e) => setFeeDenom(e.target.value)} />
              </label>
            </div>
            <div className="field-row">
              <label className="field">
                <span>Memo</span>
                <input type="text" value={memo} onChange={(e) => setMemo(e.target.value)} />
              </label>
              <label className="field">
                <span>
                  Timeout height <em>(0 disables)</em>
                </span>
                <input type="number" min={0} value={timeoutHeight} onChange={(e) => setTimeoutHeight(e.target.value)} />
              </label>
            </div>
          </>
        )}

        <div className="field-row">
          <label className="field checkbox-field">
            <input type="checkbox" checked={broadcast} onChange={(e) => setBroadcast(e.target.checked)} />
            <span>Broadcast to a {chain === "solana" ? "Solana cluster" : chain === "bitcoin" ? "Bitcoin node" : "Cosmos node"}</span>
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
            Sign transaction
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
                {result.signature && (
                  <tr>
                    <td>Signature</td>
                    <td>{result.signature}</td>
                  </tr>
                )}
                {result.raw_base58 && (
                  <tr>
                    <td>Raw (base58)</td>
                    <td>{result.raw_base58}</td>
                  </tr>
                )}
                {result.raw_hex && (
                  <tr>
                    <td>Raw (hex)</td>
                    <td>{result.raw_hex}</td>
                  </tr>
                )}
                {result.txid && (
                  <tr>
                    <td>Txid</td>
                    <td>{result.txid}</td>
                  </tr>
                )}
                {result.fee_sat != null && (
                  <tr>
                    <td>Fee (sat)</td>
                    <td>{result.fee_sat}</td>
                  </tr>
                )}
                {result.change_sat != null && (
                  <tr>
                    <td>Change (sat)</td>
                    <td>{result.change_sat}</td>
                  </tr>
                )}
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
    </section>
  );
}
