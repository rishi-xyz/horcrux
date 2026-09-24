import { GITHUB_URL } from "@/lib/nav";

export function CtaSection() {
  return (
    <section className="relative z-10 py-22 text-center">
      <div className="page-container mx-auto max-w-[620px]">
        <h2 className="mb-3 text-[clamp(1.6rem,3vw,2.1rem)]">
          Stop trusting one device with everything.
        </h2>
        <p className="mb-7 text-muted">
          MIT-licensed, written in Rust, and designed to be read before it&apos;s run.
        </p>
        <div className="flex flex-wrap justify-center gap-3.5">
          <a
            href="#install"
            className="inline-flex items-center gap-2 rounded-full bg-linear-to-br from-accent to-accent-2 px-6.5 py-3.5 text-base font-semibold text-[#04120f] transition-transform hover:-translate-y-px hover:brightness-105"
          >
            Install HORCRUX
          </a>
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 rounded-full border border-border-strong bg-white/2 px-6.5 py-3.5 text-base font-semibold transition-colors hover:border-accent hover:text-accent"
          >
            Read the source
          </a>
        </div>
      </div>
    </section>
  );
}
