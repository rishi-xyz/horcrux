import { SectionHeading } from "./section-heading";
import { CopyCodeBlock } from "./copy-code-block";

const PREREQS: [string, string][] = [
  ["Rust", "Stable (latest)"],
  ["Cargo", "Latest"],
  ["Git", "Latest"],
  ["USB drive(s)", "Recommended"],
  ["OS", "Linux / macOS / Windows"],
  ["solana-test-validator", "Optional, for live broadcast"],
];

interface Step {
  title: string;
  code: string;
  display?: React.ReactNode;
}

const STEPS: Step[] = [
  {
    title: "Clone the repository",
    code: "git clone https://github.com/rishi-xyz/horcrux.git\ncd horcrux",
  },
  {
    title: "Build a release binary",
    code: "cargo build --release\n# binary at target/release/horcrux",
    display: (
      <>
        cargo build --release{"\n"}
        <span className="text-faint"># binary at target/release/horcrux</span>
      </>
    ),
  },
  {
    title: "Run the test suite",
    code: "cargo test",
  },
  {
    title: "Split your first key",
    code: "./target/release/horcrux init \\\n    --threshold 2 --shares 3 --out-dir ./usb",
  },
];

export function InstallSection() {
  return (
    <section id="install" className="relative z-10 py-24">
      <div className="page-container">
        <SectionHeading kicker="Installation" title="Build it yourself. It's Rust, it's auditable." />

        <div className="grid min-w-0 gap-10 lg:grid-cols-[0.85fr_1.15fr]">
          <div className="min-w-0">
            <h3 className="mb-4 text-[1.05rem] font-semibold">Prerequisites</h3>
            <table className="mb-5 w-full table-fixed border-collapse">
              <tbody>
                {PREREQS.map(([label, value]) => (
                  <tr key={label} className="border-b border-border">
                    <td className="w-[46%] py-2.5 text-[0.9rem] font-semibold break-words">{label}</td>
                    <td className="py-2.5 text-[0.9rem] text-muted break-words">{value}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="mb-2.5 text-[0.88rem] text-muted">Verify your toolchain:</p>
            <CopyCodeBlock code={"rustc --version\ncargo --version"} />
          </div>

          <div className="grid min-w-0 gap-6">
            {STEPS.map((step, i) => (
              <div key={step.title} className="grid min-w-0 grid-cols-[34px_1fr] gap-4">
                <div className="flex h-8.5 w-8.5 items-center justify-center rounded-full bg-accent-soft font-mono text-[0.9rem] font-bold text-accent">
                  {i + 1}
                </div>
                <div className="min-w-0">
                  <h4 className="mt-1 mb-2.5 text-[0.98rem] font-semibold">{step.title}</h4>
                  <CopyCodeBlock code={step.code} display={step.display} />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
