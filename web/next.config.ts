import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  images: { unoptimized: true },
  trailingSlash: true,
  // In dev, proxy /api to the Rust backend so the app stays same-origin
  // (required because the backend enforces X-Horcrux-Token and does not
  // answer cross-origin CORS preflights). Rewrites are ignored for export
  // builds but work with `next dev`.
  async rewrites() {
    const backend = process.env.HORCRUX_BACKEND_URL || "http://127.0.0.1:7420";
    return [{ source: "/api/:path*", destination: `${backend}/api/:path*` }];
  },
};

export default nextConfig;
