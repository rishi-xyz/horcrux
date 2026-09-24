import { Play } from "lucide-react";
import { SectionHeading } from "./section-heading";

export function DemoSection() {
  return (
    <section id="demo" className="relative z-10 py-24">
      <div className="page-container">
        <SectionHeading
          kicker="Demo"
          title="Watch it split, distribute, and sign."
          subtitle="A walkthrough video is on the way — splitting a key, distributing shards to guardians, and signing both offline and via threshold MPC."
        />
        <div className="overflow-hidden rounded-[20px] border border-border-strong">
          <div className="bg-noise bg-grid relative flex aspect-16/8 flex-col items-center justify-center gap-4 bg-surface">
            <div className="relative flex h-18 w-18 items-center justify-center rounded-full border border-border-strong bg-white/4 text-accent">
              <Play size={28} fill="currentColor" />
            </div>
            <p className="relative font-mono text-[0.85rem] text-faint">Demo video coming soon</p>
          </div>
        </div>
      </div>
    </section>
  );
}
