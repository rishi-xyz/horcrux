import { SectionHeading } from "./section-heading";

export function InterfacesSection() {
  return (
    <section id="interfaces" className="relative z-10 border-y border-border bg-bg-soft py-24">
      <div className="page-container">
        <SectionHeading
          kicker="Two interfaces, one core"
          title="Drive it from a terminal, or a browser on the same machine."
          subtitle="Both talk to the exact same Rust library — no separate implementation, no separate trust boundary. Pick whichever fits how you work."
        />

        <div className="grid gap-6 lg:grid-cols-2">
          <article className="flex flex-col gap-4 rounded-[20px] border border-border bg-surface p-7.5">
            <div className="flex flex-col gap-2.5">
              <span className="w-fit rounded-full bg-white/6 px-2.5 py-1 font-mono text-[0.78rem] text-muted">
                horcrux tui
              </span>
              <h3 className="text-xl font-semibold">Interactive terminal UI</h3>
            </div>
            <p className="text-[0.94rem] text-muted">
              Menu-driven screens for Access log, Verify, Init, Sign, and MPC split/sign — for
              guardians who&apos;d rather navigate forms than memorize flags. Runs entirely in
              your terminal, over SSH or locally.
            </p>
            <div className="overflow-hidden rounded-xl border border-border-strong bg-bg-soft">
              <div className="flex gap-1.5 bg-surface-2 px-3 py-2.5">
                <span className="h-2 w-2 rounded-full bg-white/18" />
                <span className="h-2 w-2 rounded-full bg-white/18" />
                <span className="h-2 w-2 rounded-full bg-white/18" />
              </div>
              <div className="grid gap-1.5 p-4 font-mono text-[0.84rem]">
                <div className="text-accent">▸ Sign</div>
                <div className="text-faint">  MPC split / sign</div>
                <div className="text-faint">  Verify shards</div>
                <div className="text-faint">  Access log</div>
              </div>
            </div>
            <code className="w-fit self-start rounded-full border border-border-strong bg-white/5 px-3.5 py-2 font-mono text-[0.86rem]">
              horcrux tui
            </code>
          </article>

          <article className="flex flex-col gap-4 rounded-[20px] border border-accent/32 bg-surface p-7.5">
            <div className="flex flex-col gap-2.5">
              <span className="w-fit rounded-full bg-accent-soft px-2.5 py-1 font-mono text-[0.78rem] text-accent">
                horcrux web
              </span>
              <h3 className="flex items-center gap-2.5 text-xl font-semibold">
                Local web UI
                <span className="rounded-full bg-accent px-2 py-0.5 font-mono text-[0.68rem] font-bold text-[#04120f]">
                  new
                </span>
              </h3>
            </div>
            <p className="text-[0.94rem] text-muted">
              Run one command and a lightweight dashboard opens in your browser — same Solana
              MVP flows as the TUI, with forms, live audit warnings, and readable results. Bound
              to <code className="rounded-md bg-white/6 px-1.5 py-0.5 text-[0.86em]">127.0.0.1</code>{" "}
              only, gated by a per-run token: nothing off this machine can ever reach it.
            </p>
            <div className="overflow-hidden rounded-xl border border-border-strong bg-bg-soft">
              <div className="flex gap-1.5 bg-surface-2 px-3 py-2.5">
                <span className="h-2 w-2 rounded-full bg-white/18" />
                <span className="h-2 w-2 rounded-full bg-white/18" />
                <span className="h-2 w-2 rounded-full bg-white/18" />
              </div>
              <div className="grid gap-3 p-4">
                <div className="rounded-full border border-border bg-surface px-3 py-1.5 font-mono text-[0.76rem] text-faint">
                  127.0.0.1:7420/?token=•••••
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div className="rounded-lg border border-border bg-surface py-3 text-center text-[0.82rem] text-muted">
                    Access log
                  </div>
                  <div className="rounded-lg border border-border bg-surface py-3 text-center text-[0.82rem] text-muted">
                    Verify
                  </div>
                  <div className="rounded-lg border border-accent/40 bg-surface py-3 text-center text-[0.82rem] text-accent">
                    Sign
                  </div>
                  <div className="rounded-lg border border-border bg-surface py-3 text-center text-[0.82rem] text-muted">
                    MPC split/sign
                  </div>
                </div>
              </div>
            </div>
            <code className="w-fit self-start rounded-full border border-border-strong bg-white/5 px-3.5 py-2 font-mono text-[0.86rem]">
              horcrux web --port 7420
            </code>
          </article>
        </div>
      </div>
    </section>
  );
}
