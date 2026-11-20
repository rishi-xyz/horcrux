import { useState } from "react";
import { api } from "../api";

/// "Show as QR" button for a just-written shard/share path — reuses the
/// same HX3 QR transport as the TUI's Ctrl+Q overlay and the CLI's
/// air-gapped `qr-*` commands via `/api/shard-qr`, so a guardian can scan
/// their shard onto their own device instead of copying a file.
export function QrShow({ path }: { path: string }) {
  const [frames, setFrames] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function show() {
    setBusy(true);
    setError(null);
    try {
      const res = await api<{ frames: string[] }>("/shard-qr", { method: "POST", body: { path } });
      setFrames(res.frames);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ marginTop: 6 }}>
      <button className="btn btn-outline" type="button" disabled={busy} onClick={show}>
        Show as QR
      </button>
      {error && <div className="banner banner-error" style={{ marginTop: 8 }}>{error}</div>}
      {frames && (
        <div className="qr-frames">
          {frames.map((b64, i) => (
            <div className="qr-frame" key={i}>
              <img src={"data:image/png;base64," + b64} alt={"QR frame " + (i + 1)} />
              {frames.length > 1 && (
                <p className="qr-frame-label">
                  frame {i + 1}/{frames.length}
                </p>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
