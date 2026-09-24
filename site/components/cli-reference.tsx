import { SectionHeading } from "./section-heading";

const COMMANDS: { cmd: string; desc: string; accent?: boolean }[] = [
  { cmd: "init", desc: "Split a private key into encrypted shard files (Mode A)" },
  { cmd: "reconstruct", desc: "Reconstruct a key from a threshold of shards" },
  { cmd: "sign", desc: "Mode A signing — Solana, Bitcoin, or Cosmos — offline or broadcast" },
  { cmd: "mpc-split", desc: "Dealer-split a key into FROST key shares (Mode B)" },
  { cmd: "mpc-sign", desc: "Threshold FROST signing without ever reconstructing the key" },
  {
    cmd: "qr-request … qr-finalize",
    desc: "Mode B over a real air gap — every protocol message crosses as a QR code",
  },
  { cmd: "verify", desc: "Check shard/share files for structural integrity, read-only" },
  { cmd: "log", desc: "View the append-only access log" },
  { cmd: "tui", desc: "Launch the interactive terminal UI" },
  { cmd: "web", desc: "Launch the local, loopback-only web UI", accent: true },
];

export function CliReference() {
  return (
    <section id="cli" className="relative z-10 border-y border-border bg-bg-soft py-24">
      <div className="page-container">
        <SectionHeading kicker="CLI reference" title="Every command, at a glance." />
        <div className="overflow-hidden rounded-[14px] border border-border">
          <table className="w-full border-collapse bg-surface">
            <thead>
              <tr className="bg-surface-2">
                <th className="border-b border-border px-5 py-3.5 text-left font-mono text-[0.72rem] tracking-wider text-faint uppercase">
                  Command
                </th>
                <th className="border-b border-border px-5 py-3.5 text-left font-mono text-[0.72rem] tracking-wider text-faint uppercase">
                  Purpose
                </th>
              </tr>
            </thead>
            <tbody>
              {COMMANDS.map(({ cmd, desc, accent }) => (
                <tr key={cmd} className="border-b border-border last:border-b-0">
                  <td className="px-5 py-3.5">
                    <code
                      className={`rounded-md px-2 py-0.5 text-[0.85em] ${
                        accent ? "bg-accent-2/12 text-accent-2" : "bg-accent-soft text-accent"
                      }`}
                    >
                      {cmd}
                    </code>
                  </td>
                  <td className="px-5 py-3.5 text-[0.9rem] text-muted">{desc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-4.5 text-[0.85rem] text-faint">
          Full flag reference, environment variables, and cryptographic design notes live in{" "}
          <code className="rounded-md bg-white/6 px-1.5 py-0.5">README.md</code>.
        </p>
      </div>
    </section>
  );
}
