"use client";

import { useState } from "react";

export function CopyCodeBlock({ code, display }: { code: string; display?: React.ReactNode }) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(code);
    } catch {
      // clipboard API unavailable — fail silently, the code is still selectable
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  }

  return (
    <div className="relative overflow-x-auto rounded-[9px] border border-border-strong bg-surface px-4.5 py-4">
      <pre className="pr-16 font-mono text-[0.85rem] leading-relaxed whitespace-pre">
        {display ?? code}
      </pre>
      <button
        type="button"
        onClick={handleCopy}
        className={`absolute top-2.5 right-2.5 rounded-md border px-2.5 py-1 font-sans text-[0.72rem] font-semibold transition-colors ${
          copied
            ? "border-accent text-accent"
            : "border-border-strong bg-white/4 text-muted hover:border-accent hover:text-accent"
        }`}
      >
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
}
