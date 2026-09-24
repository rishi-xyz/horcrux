import type { LucideIcon } from "lucide-react";
import { Cpu, HardDrive, Layers, Lock, ShieldAlert, ShieldCheck, Share2, Workflow } from "lucide-react";
import { SectionHeading } from "./section-heading";

const FEATURES: { icon: LucideIcon; title: string; body: React.ReactNode }[] = [
  {
    icon: ShieldCheck,
    title: "Offline first",
    body: "Every critical operation — split, decrypt, sign — runs with zero network access. Broadcasting is a separate, opt-in step.",
  },
  {
    icon: Share2,
    title: "Threshold cryptography",
    body: "2-of-3, 3-of-5, 5-of-7, or any N-of-M split via Shamir's Secret Sharing. Fewer than the threshold reveals nothing.",
  },
  {
    icon: HardDrive,
    title: "USB-bound storage",
    body: "Each shard is written to a dedicated guardian USB drive. Stealing one drive compromises nothing on its own.",
  },
  {
    icon: Lock,
    title: "Password-hardened encryption",
    body: "AES-256-GCM with Argon2id key derivation, random salts, and authentication tags — sealed before anything leaves memory.",
  },
  {
    icon: Layers,
    title: "Dual signing modes",
    body: (
      <>
        <strong className="text-text">Mode A</strong> reconstructs the key in locked RAM for
        milliseconds. <strong className="text-text">Mode B</strong> uses FROST MPC so the key
        never exists anywhere at all.
      </>
    ),
  },
  {
    icon: Workflow,
    title: "Multi-chain",
    body: "One reconstructed seed signs Solana (Ed25519), Bitcoin Taproot (BIP340/341/342), and Cosmos SDK transactions.",
  },
  {
    icon: Cpu,
    title: "Memory safety",
    body: "Written in Rust. Sensitive buffers are zeroized immediately after use — never left to linger for a GC or a swap file.",
  },
  {
    icon: ShieldAlert,
    title: "Behavioral anomaly detection",
    body: "A rule-based scorer flags repeated failures, odd-hour access, and unfamiliar shard combinations before a signature happens.",
  },
];

export function FeaturesSection() {
  return (
    <section id="features" className="relative z-10 border-y border-border bg-bg-soft py-24">
      <div className="page-container">
        <SectionHeading kicker="Features" title="Custody, engineered like infrastructure." />
        <div className="grid gap-4.5 sm:grid-cols-2 lg:grid-cols-4">
          {FEATURES.map(({ icon: Icon, title, body }) => (
            <article
              key={title}
              className="rounded-[14px] border border-border bg-surface p-6.5 transition-all hover:-translate-y-0.5 hover:border-accent/35"
            >
              <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-xl bg-accent-soft text-accent">
                <Icon size={22} />
              </div>
              <h3 className="mb-2 text-[1.02rem] font-semibold">{title}</h3>
              <p className="text-[0.9rem] text-muted">{body}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
