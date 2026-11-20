import { useEffect, useRef, useState } from "react";
import jsQR from "jsqr";
import { api } from "../api";

/// Web-only: open the device camera and scan a shard/share QR code (from
/// `write_shard_qr`/the TUI's Ctrl+Q overlay/the web "Show as QR" button)
/// straight into a form, instead of exporting a PNG file first.
///
/// `jsQR` runs client-side purely to *detect* a code is in frame and
/// trigger an automatic capture — the actual HX3 decode/validation stays
/// server-side via the same `/api/shard-qr-import` endpoint the file-upload
/// "Import from QR" button already uses, so the wire protocol is never
/// duplicated in JS. A false-positive (someone's webcam catches an
/// unrelated QR code) just gets an error from the server and the scanner
/// keeps running.
export function QrScanner({ onImport }: { onImport: (path: string) => void }) {
  const [active, setActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("Point the camera at a shard/share QR code");
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const rafRef = useRef<number | null>(null);
  const lockedRef = useRef(false);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;

    (async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment" },
        });
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          await videoRef.current.play();
        }
        tick();
      } catch (e) {
        setError(
          "Couldn't open the camera: " + (e instanceof Error ? e.message : String(e))
        );
        setActive(false);
      }
    })();

    function tick() {
      if (cancelled) return;
      const video = videoRef.current;
      const canvas = canvasRef.current;
      if (video && canvas && video.readyState === video.HAVE_ENOUGH_DATA && !lockedRef.current) {
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
          const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
          const code = jsQR(imageData.data, imageData.width, imageData.height);
          if (code) {
            void handleDetected(canvas);
          }
        }
      }
      rafRef.current = requestAnimationFrame(tick);
    }

    async function handleDetected(canvas: HTMLCanvasElement) {
      lockedRef.current = true;
      setStatus("Code detected — decoding…");
      try {
        const blob: Blob | null = await new Promise((resolve) =>
          canvas.toBlob(resolve, "image/png")
        );
        if (!blob) throw new Error("failed to capture frame");
        const png_base64 = await blobToBase64(blob);
        const res = await api<{ path: string }>("/shard-qr-import", {
          method: "POST",
          body: { png_base64 },
        });
        setStatus("Imported.");
        stopStream();
        setActive(false);
        onImport(res.path);
        return;
      } catch (e) {
        setStatus(
          "That code isn't a shard/share QR (" +
            (e instanceof Error ? e.message : String(e)) +
            ") — still scanning…"
        );
      }
      // Cooldown before trying again, so a code that keeps failing doesn't
      // hammer the server every animation frame.
      setTimeout(() => {
        lockedRef.current = false;
      }, 1200);
    }

    function stopStream() {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
    }

    return () => {
      cancelled = true;
      stopStream();
    };
  }, [active, onImport]);

  return (
    <div>
      <button
        className="btn btn-outline"
        type="button"
        onClick={() => {
          setError(null);
          setStatus("Point the camera at a shard/share QR code");
          setActive((a) => !a);
        }}
      >
        {active ? "Stop camera" : "Scan with camera"}
      </button>
      {error && (
        <div className="banner banner-error" style={{ marginTop: 8 }}>
          {error}
        </div>
      )}
      {active && (
        <div className="scanner-wrap">
          <video ref={videoRef} playsInline muted />
          <div className="scanner-hint">{status}</div>
        </div>
      )}
      <canvas ref={canvasRef} style={{ display: "none" }} />
    </div>
  );
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",").pop() || "");
    reader.onerror = () => reject(reader.error || new Error("failed to read captured frame"));
    reader.readAsDataURL(blob);
  });
}
