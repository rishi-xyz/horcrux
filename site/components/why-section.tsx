import { SectionHeading } from "./section-heading";

const REASONS = [
  {
    title: "Single device failure",
    body: "Lose the phone, lose the wallet. One point of compromise ends custody entirely.",
  },
  {
    title: "Plaintext backups",
    body: "Seed phrases photographed, typed into notes apps, or laminated in a drawer.",
  },
  {
    title: "Vendor dependence",
    body: "Custodial recovery flows and proprietary hardware you can't audit or replace.",
  },
  {
    title: "Internet dependency",
    body: "Signing that requires connectivity is signing that can be intercepted.",
  },
];

export function WhySection() {
  return (
    <section id="why" className="relative z-10 py-24">
      <div className="page-container">
        <SectionHeading
          kicker="Why HORCRUX"
          title="Wallets keep failing the same way."
          subtitle="Every mainstream wallet architecture eventually concentrates trust in one device, one backup, or one vendor. HORCRUX treats key management as a lifecycle, not a storage problem."
        />
        <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {REASONS.map((r) => (
            <div key={r.title} className="rounded-[14px] border border-border bg-surface p-6">
              <h3 className="mb-2 text-[1.02rem] font-semibold">{r.title}</h3>
              <p className="text-[0.92rem] text-muted">{r.body}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
