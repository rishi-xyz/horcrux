import { useRef, useState } from "react";
import { api } from "../api";

/// "Import from QR" file picker: decode one or more saved/screenshotted QR
/// PNGs via `/api/shard-qr-import` and hand each resulting staged path back
/// to the caller. Pairs with `QrScanner` (camera) as the other way to get a
/// shard/share onto this form without a plain file path.
export function QrFileImport({ onImport }: { onImport: (path: string) => void }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    const files = Array.from(inputRef.current?.files || []);
    if (!files.length) return;
    setBusy(true);
    setError(null);
    try {
      for (const file of files) {
        const png_base64 = await fileToBase64(file);
        const res = await api<{ path: string }>("/shard-qr-import", {
          method: "POST",
          body: { png_base64 },
        });
        onImport(res.path);
      }
      if (inputRef.current) inputRef.current.value = "";
    } catch (e) {
      setError("QR import failed: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <div className="field-row qr-import-row">
        <input type="file" accept="image/png" multiple ref={inputRef} />
        <button className="btn btn-outline" type="button" disabled={busy} onClick={run}>
          Import from QR file
        </button>
      </div>
      {error && <div className="banner banner-error">{error}</div>}
    </div>
  );
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",").pop() || "");
    reader.onerror = () => reject(reader.error || new Error("failed to read file"));
    reader.readAsDataURL(file);
  });
}
