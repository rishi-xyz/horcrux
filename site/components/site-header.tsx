"use client";

import { useState } from "react";
import { Menu, X } from "lucide-react";
import { BrandMark } from "./brand-mark";
import { GithubIcon } from "./github-icon";
import { GITHUB_URL, NAV_LINKS } from "@/lib/nav";

export function SiteHeader() {
  const [open, setOpen] = useState(false);

  return (
    <header className="sticky top-0 z-50 border-b border-border bg-bg/72 backdrop-blur-xl">
      <div className="page-container flex h-[68px] items-center gap-7">
        <a href="#top" className="flex items-center gap-2.5 font-bold tracking-wide">
          <BrandMark />
          <span className="text-[1.05rem]">HORCRUX</span>
        </a>

        <nav className="ml-2 hidden flex-1 gap-5.5 md:flex" aria-label="Primary">
          {NAV_LINKS.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="text-sm font-medium text-muted transition-colors hover:text-text"
            >
              {link.label}
            </a>
          ))}
        </nav>

        <div className="hidden items-center gap-2.5 md:flex">
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 rounded-full px-4.5 py-2.5 text-sm font-semibold text-muted transition-colors hover:text-text"
          >
            <GithubIcon size={16} />
            GitHub
          </a>
          <a
            href="#install"
            className="inline-flex items-center gap-2 rounded-full bg-linear-to-br from-accent to-accent-2 px-4.5 py-2.5 text-sm font-semibold text-[#04120f] transition-transform hover:-translate-y-px hover:brightness-105"
          >
            Get started
          </a>
        </div>

        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-label="Toggle navigation"
          aria-expanded={open}
          className="ml-auto flex h-9.5 w-9.5 items-center justify-center rounded-lg border border-border-strong md:hidden"
        >
          {open ? <X size={18} /> : <Menu size={18} />}
        </button>
      </div>

      {open && (
        <nav className="flex flex-col border-t border-border px-6 pb-4.5 md:hidden" aria-label="Mobile">
          {NAV_LINKS.map((link) => (
            <a
              key={link.href}
              href={link.href}
              onClick={() => setOpen(false)}
              className="border-b border-border py-2.5 text-muted"
            >
              {link.label}
            </a>
          ))}
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="py-2.5 text-muted"
          >
            GitHub
          </a>
        </nav>
      )}
    </header>
  );
}
