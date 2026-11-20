import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// Base is relative ("./") so the built assets work when served from any
// path by the embedded axum server — no absolute "/assets/..." URLs that
// would break if the app were ever mounted under a sub-path.
export default defineConfig({
    plugins: [react()],
    base: "./",
    build: {
        outDir: "dist",
        emptyOutDir: true,
    },
    server: {
        // Dev-mode proxy so `npm run dev` can talk to a `cargo run -- web`
        // instance without CORS/token gymnastics — matches the real deployment
        // (same-origin) closely enough for local development.
        proxy: {
            "/api": "http://127.0.0.1:7420",
        },
    },
});
