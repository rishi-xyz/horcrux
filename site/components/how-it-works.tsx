import { SectionHeading } from "./section-heading";

const MODE_A_STEPS = [
  "Decrypt a threshold of shards",
  "Reconstruct the key in RAM",
  "Sign the transaction",
  "Zeroize memory",
  "Output the signed transaction",
];

const MODE_B_STEPS = [
  "Each guardian decrypts their own share",
  "Round 1: nonce commitments",
  "Round 2: signature shares",
  "Coordinator aggregates shares",
  "Valid Ed25519 / Schnorr signature",
];

export function HowItWorks() {
  return (
    <section id="how-it-works" className="relative z-10 py-24">
      <div className="page-container">
        <SectionHeading
          kicker="How it works"
          title="Two ways to turn shards into a signature."
          subtitle="Both modes start the same way: split once, distribute to guardians, sign later. They differ in what happens to the key at signing time."
        />

        <div className="mb-14 grid gap-5.5 lg:grid-cols-2">
          <div className="rounded-[20px] border border-border bg-surface p-7.5">
            <span className="mb-3.5 inline-block rounded-full bg-white/6 px-2.5 py-1 font-mono text-[0.72rem] tracking-wider text-muted uppercase">
              Mode A
            </span>
            <h3 className="mb-2.5 text-xl font-semibold">Air-gapped reconstruction</h3>
            <p className="mb-5 text-[0.94rem] text-muted">
              The key exists in locked RAM for a few milliseconds, signs the transaction, then
              is zeroized. Simple, fast, fully offline.
            </p>
            <ol className="grid list-decimal gap-2 pl-5 text-[0.9rem] marker:font-mono marker:text-accent">
              {MODE_A_STEPS.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ol>
          </div>

          <div className="rounded-[20px] border border-accent/30 bg-surface bg-linear-to-b from-accent-soft to-transparent to-40% p-7.5">
            <span className="mb-3.5 inline-block rounded-full bg-accent-soft px-2.5 py-1 font-mono text-[0.72rem] tracking-wider text-accent uppercase">
              Mode B
            </span>
            <h3 className="mb-2.5 text-xl font-semibold">Threshold MPC (FROST)</h3>
            <p className="mb-5 text-[0.94rem] text-muted">
              Guardians each hold one encrypted key share and cooperatively produce a valid
              signature. The private key never exists on any machine, ever.
            </p>
            <ol className="grid list-decimal gap-2 pl-5 text-[0.9rem] marker:font-mono marker:text-accent">
              {MODE_B_STEPS.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ol>
          </div>
        </div>

        <div className="grid items-center gap-10 rounded-[20px] border border-border bg-surface p-9 lg:grid-cols-2">
          <div>
            <h3 className="mb-2.5 text-[1.15rem] font-semibold">Two-factor custody, per guardian</h3>
            <p className="text-muted">
              Every shard requires <strong className="text-text">physical possession</strong>{" "}
              of its USB drive <em>and</em> <strong className="text-text">knowledge</strong> of
              its password. Stealing the drive alone is insufficient. Knowing the password alone
              is insufficient.
            </p>
          </div>
          <div className="flex flex-wrap items-center justify-center gap-3.5">
            <div className="rounded-[9px] border border-border-strong bg-surface-2 px-4.5 py-3.5 text-center font-mono text-[0.85rem]">
              USB
            </div>
            <span className="text-xl text-faint">+</span>
            <div className="rounded-[9px] border border-border-strong bg-surface-2 px-4.5 py-3.5 text-center font-mono text-[0.85rem]">
              Password
            </div>
            <span className="text-xl text-faint">=</span>
            <div className="rounded-[9px] border border-accent/40 bg-surface-2 px-4.5 py-3.5 text-center font-mono text-[0.85rem] text-accent">
              One usable shard
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
