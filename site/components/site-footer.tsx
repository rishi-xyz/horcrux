import { BrandMark } from "./brand-mark";
import { GITHUB_URL } from "@/lib/nav";

const FOOTER_LINKS = [
  { href: GITHUB_URL, label: "GitHub" },
  { href: `${GITHUB_URL}/blob/main/README.md`, label: "Documentation" },
  { href: `${GITHUB_URL}/blob/main/CONTRIBUTING.md`, label: "Contributing" },
  { href: `${GITHUB_URL}/blob/main/LICENSE`, label: "License" },
];

export function SiteFooter() {
  return (
    <footer className="relative z-10 border-t border-border py-8.5">
      <div className="page-container flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-2 font-bold text-accent">
          <BrandMark size={20} />
          <span>HORCRUX</span>
        </div>
        <nav className="flex flex-wrap gap-5" aria-label="Footer">
          {FOOTER_LINKS.map((link) => (
            <a
              key={link.label}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-[0.88rem] text-muted transition-colors hover:text-text"
            >
              {link.label}
            </a>
          ))}
        </nav>
        <p className="text-[0.82rem] text-faint">MIT Licensed. Offline by design.</p>
      </div>
    </footer>
  );
}
