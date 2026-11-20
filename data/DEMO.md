# Horcrux Docker Demo Flow

This workspace's `data/` directory is mounted into the Horcrux container at `/data` (working dir). It is pre-seeded with demo shard-backed wallets and sample shards so you can demo without generating anything first.

## Start (single command)

From repo root:

```bash
docker compose up --build
```

(detached: `docker compose up --build -d`)

Get the login URL/token (required for `/api/*`):

```bash
docker compose logs -f horcrux
```

Open the printed URL in your PC browser, e.g. `http://127.0.0.1:7420/?token=<token>`.

Solana localnet runs as `solana-test-validator` in compose and is reachable from Horcrux at `http://solana:8899` (and from host at `http://127.0.0.1:8899`).

Stop: `docker compose down` (keep data/ledger). Use `-v` only if you want to remove volumes.

## Pre-seeded demo assets (in /data)

- **Solana SSS (2-of-3, Horcrux-protected)**: `demo/solana-sss/shard-{1,2,3}.hx`, password for all shards: `demo`
  - Derived address: `AWH5AsavbwvQZ9NGcpc33pcBc5HCeifYJzXebLEBb5ge`
  - (Source disposable test key used to generate demo shards: `0xd8793eb242fd36459540d41fbc18a9d52b6754307c85ab41e4a83f5a6c438488` — for demo/auditing only; treat as exposed/non-production.)
- **Cosmos SSS (2-of-3, Horcrux-protected)**: `demo/cosmos-sss/shard-{1,2,3}.hx`, password for all shards: `demo`
  - Derived cosmos address: `cosmos1nhkdhzeesrtvruct6cklf3zayz3qphu4h339gs`
  - (Source disposable test key: `0xb25e97e77f19179e99e03c15d718656b499874923d02f9ca3484c3a9d3cb2fea` — demo only.)
- **Wallet profiles**: `horcrux-wallets.json` includes the above shard-backed wallets and a watch-only Solana example.

> Security: demo keys/passwords/shards are intentionally public/test-only. Never reuse for real funds. The console never returns reconstructed private keys/seeds.

## Suggested demo flow (in browser console)

1. **Dashboard / Status**: confirm backend reachable, token accepted, Solana RPC default (`http://solana:8899` in container) and AI/advisory status.
2. **Access Log**: show audit log/history view.
3. **Verify**: verify demo shards (e.g. `demo/solana-sss/shard-1.hx, shard-2.hx, shard-3.hx` with password `demo`) to show integrity/consistency checks.
4. **Safe Reconstruct (parity)**: select 2 demo Solana shards with password `demo` → shows derived/public info (addresses) and success outcome only, **no raw key/seed**.
5. **Sign (Mode A, Solana localnet)**: use 2 Solana demo shards (pw `demo`), set RPC to localnet (`http://solana:8899`), recipient/to and lamports/blockhash as needed (or broadcast if funded/available). Observe deterministic audit warn/block behavior and advisory AI event recording.
6. **MPC (FROST)**: split/sign demo (Solana/Bitcoin as supported) to show threshold signing without key reconstruction.
7. **QR Air-gap**: shard QR export/import and 5-step flow UI (can demo export/import of demo shards).
8. **Wallets**: see pre-seeded shard-backed (Horcrux-protected) and watch-only profiles; create/import/connect more; note "Horcrux-protected vs not protected" labeling.
9. **On-chain**: check balances/history for Solana localnet/devnet and Cosmos local (graceful degradation if RPC unavailable/limited). You can fund a localnet address via host `solana` CLI/airdrop against `http://127.0.0.1:8899` if desired, then view in console.
10. **AI & Security / Settings**: review advisory AI events (derived from existing actions), manual check/config, and runtime/security notes (token-gated API, no key/seed leakage).

## Notes
- `./data` is bind-mounted to `/data`: generated shards, `horcrux-wallets.json`, `horcrux-ai-events.jsonl`, `horcrux-web-state.json`, and access log/state persist on host.
- Container binds web UI to `0.0.0.0:7420` for port publishing, but `/api/*` requires the per-run `X-Horcrux-Token` (from logs/URL). Keep it local/trusted; do not expose publicly.
- Cosmos local RPC/balance-history may be limited unless you run a local Cosmos node; UI should degrade gracefully.
