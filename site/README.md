# HORCRUX marketing site

The public-facing marketing/docs site for HORCRUX: install guide, feature
overview, CLI reference, and a slot for the demo video once it's ready.
Next.js (App Router) + React + Tailwind CSS v4, no other runtime dependencies
besides `lucide-react` for icons.

## Develop

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Build

```bash
npm run build
npm run start
```

## Structure

```
app/
  layout.tsx     fonts (Inter + JetBrains Mono), metadata
  page.tsx       composes every section
  globals.css    Tailwind v4 theme tokens (@theme) + a few utility classes
components/      one file per section (hero, features, install, ...)
lib/nav.ts       shared nav links / GitHub URL
```

Dark theme only, by design — this mirrors the terminal/security aesthetic of
the CLI and TUI it's documenting.
