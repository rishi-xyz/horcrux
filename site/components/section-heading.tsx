export function SectionHeading({
  kicker,
  title,
  subtitle,
}: {
  kicker: string;
  title: string;
  subtitle?: string;
}) {
  return (
    <div className="mb-12 max-w-[640px]">
      <p className="mb-3 font-mono text-[0.78rem] tracking-wider text-accent uppercase">
        {kicker}
      </p>
      <h2 className="mb-3.5 text-[clamp(1.6rem,3vw,2.2rem)] tracking-tight">{title}</h2>
      {subtitle && <p className="text-[1.02rem] text-muted">{subtitle}</p>}
    </div>
  );
}
