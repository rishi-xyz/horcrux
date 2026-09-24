const TERM_LINES = [
  { text: "$ horcrux init --threshold 2 --shares 3 --out-dir ./usb", cls: "text-accent" },
  { text: "Wrote 3 shards to ./usb (threshold 2):", cls: "text-ok" },
  { text: "  ./usb/shard-1.hx" },
  { text: "  ./usb/shard-2.hx" },
  { text: "  ./usb/shard-3.hx" },
  { text: "" },
  { text: "$ horcrux sign ./usb/shard-1.hx ./usb/shard-2.hx \\", cls: "text-accent" },
  { text: "    --to 7bYw...q9Fp --lamports 1000000000 \\" },
  { text: "    --blockhash 5eyk...Hn2c" },
  { text: "Audit: ALLOW — no anomalies", cls: "text-faint" },
  { text: "From:      6Fkq...s2Ab", cls: "text-ok" },
  { text: "Signature: 4hQe...pM1x", cls: "text-ok" },
];

export function Hero() {
  return (
    <section className="relative z-10 pt-22 pb-15">
      <div className="page-container grid items-center gap-14 lg:grid-cols-[1.05fr_0.95fr]">
        <div>
          <p className="mb-5.5 inline-flex items-center gap-2 rounded-full border border-accent/30 bg-accent-soft px-3 py-1.5 font-mono text-[0.78rem] tracking-wider text-accent uppercase">
            <span className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_8px_var(--color-accent)]" />
            Rust · Shamir&apos;s Secret Sharing · FROST MPC
          </p>

          <h1 className="mb-5.5 text-[clamp(2.1rem,4.4vw,3.4rem)] leading-[1.12] font-extrabold tracking-tight">
            Your private key was never
            <br />
            meant to live in one place.
          </h1>

          <p className="mb-7.5 max-w-[52ch] text-[1.08rem] text-muted">
            HORCRUX splits a blockchain private key into encrypted shards held by
            independent guardians. Reconstruct offline to sign, or never
            reconstruct it at all with threshold MPC. Solana, Bitcoin, and Cosmos —
            air-gapped by default.
          </p>

          <div className="mb-10 flex flex-wrap gap-3.5">
            <a
              href="#install"
              className="inline-flex items-center gap-2 rounded-full bg-linear-to-br from-accent to-accent-2 px-6.5 py-3.5 text-base font-semibold text-[#04120f] transition-transform hover:-translate-y-px hover:brightness-105"
            >
              Install HORCRUX
            </a>
            <a
              href="#how-it-works"
              className="inline-flex items-center gap-2 rounded-full border border-border-strong bg-white/2 px-6.5 py-3.5 text-base font-semibold transition-colors hover:border-accent hover:text-accent"
            >
              See how it works
            </a>
          </div>

          <div className="flex flex-wrap gap-8.5">
            {[
              ["2-of-3", "to N-of-M thresholds"],
              ["AES-256-GCM", "+ Argon2id per shard"],
              ["0", "network calls to sign"],
            ].map(([stat, label]) => (
              <div key={stat} className="flex flex-col gap-0.5">
                <strong className="font-mono text-[1.15rem]">{stat}</strong>
                <span className="text-[0.8rem] text-faint">{label}</span>
              </div>
            ))}
          </div>
        </div>

        <div>
          <div className="overflow-hidden rounded-[20px] border border-border-strong bg-surface shadow-[0_1px_0_rgba(255,255,255,0.03)_inset,0_20px_50px_-20px_rgba(0,0,0,0.6)]">
            <div className="flex items-center gap-2 border-b border-border bg-surface-2 px-4 py-3">
              <span className="h-2.5 w-2.5 rounded-full bg-[#ff5f57]" />
              <span className="h-2.5 w-2.5 rounded-full bg-[#febc2e]" />
              <span className="h-2.5 w-2.5 rounded-full bg-[#28c840]" />
              <span className="ml-2.5 font-mono text-[0.76rem] text-faint">
                guardian@offline — horcrux
              </span>
            </div>
            <div className="overflow-x-auto px-5 py-5.5 font-mono text-[0.82rem] leading-relaxed">
              <pre className="whitespace-pre-wrap break-words">
                {TERM_LINES.map((line, i) => (
                  <div key={i} className={line.cls}>
                    {line.text || " "}
                  </div>
                ))}
                <span className="animate-blink text-accent">▍</span>
              </pre>
            </div>
          </div>

          <div className="mt-5 flex justify-end gap-3 pr-1.5">
            <span className="flex h-10.5 w-10.5 rotate-[-6deg] items-center justify-center rounded-xl border border-border-strong bg-surface-2 font-mono text-sm font-bold text-accent shadow-[0_10px_24px_rgba(0,0,0,0.35)]">
              A
            </span>
            <span className="flex h-10.5 w-10.5 translate-y-[-4px] rotate-[4deg] items-center justify-center rounded-xl border border-accent-2/40 bg-surface-2 font-mono text-sm font-bold text-accent-2 shadow-[0_10px_24px_rgba(0,0,0,0.35)]">
              B
            </span>
            <span className="flex h-10.5 w-10.5 rotate-[8deg] items-center justify-center rounded-xl border border-border-strong bg-surface-2 font-mono text-sm font-bold text-accent shadow-[0_10px_24px_rgba(0,0,0,0.35)]">
              C
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
