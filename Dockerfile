# syntax=docker/dockerfile:1

# ---- Web console (static export embedded into the Rust binary) ----
FROM node:22-bookworm-slim AS web-build
WORKDIR /app/web
COPY web/package.json ./
COPY web/package-lock.json* ./
RUN npm install --no-audit --no-fund --progress=false
COPY web/ ./
RUN npm run build

# ---- Horcrux binary (includes web/out at compile time) ----
FROM rust:1-bookworm AS build
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev clang ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# The web UI is embedded via include_dir!("$CARGO_MANIFEST_DIR/web/out").
COPY --from=web-build /app/web/out ./web/out
RUN cargo build --release --bin horcrux

# ---- Runtime ----
FROM debian:bookworm-slim
WORKDIR /data
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/horcrux /usr/local/bin/horcrux

# Default to container-friendly bind; token-gated /api remains required.
ENV HORCRUX_WEB_HOST=0.0.0.0
ENV HORCRUX_WEB_DATA_DIR=/data
# When used with docker compose, Solana localnet is reachable at this URL.
ENV HORCRUX_RPC_URL=http://solana:8899

EXPOSE 7420
CMD ["horcrux", "web", "--port", "7420"]
