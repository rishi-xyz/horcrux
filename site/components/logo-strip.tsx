const ITEMS = [
  "Solana",
  "Bitcoin Taproot",
  "Cosmos SDK",
  "Ed25519",
  "secp256k1",
  "FROST (RFC 9591)",
];

export function LogoStrip() {
  return (
    <section className="relative z-10 border-y border-border bg-bg-soft">
      <div className="page-container flex flex-wrap justify-between gap-8 py-5 font-mono text-[0.78rem] tracking-wide text-faint">
        {ITEMS.map((item) => (
          <span key={item}>{item}</span>
        ))}
      </div>
    </section>
  );
}
