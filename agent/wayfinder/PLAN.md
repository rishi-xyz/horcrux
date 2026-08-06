# HORCRUX — Build Plan
### Final-year major project · 8-week timeline 

---

## Destination

A working Rust CLI (`horcrux`) that: splits a private key via Shamir's Secret Sharing,
binds encrypted shards to USB-simulated paths, signs and broadcasts an EVM transaction
in two modes (Mode A: air-gapped RAM reconstruction, Mode B: true threshold-ECDSA MPC
where the key is never assembled anywhere), and flags anomalous access attempts via a
lightweight anomaly-detection layer. Every phase below ends in something you can
literally run and show someone.

---

## Ground rules (read once, keep in mind every week)

1. **Don't implement cryptography from scratch.** SSS and threshold-ECDSA are both
   available as mature, audited crates. Your job is correct integration + a clean CLI +
   understanding *why* each piece works — not reproducing 40 years of published crypto
   research in 8 weeks. A viva panel will respect "I integrated an audited library
   correctly and can explain the protocol" far more than a shaky from-scratch
   implementation.
2. **Mode A is the floor, not a footnote.** If Weeks 4–6 (Mode B) blow up, you still hand
   in a complete, working, demoable system. Sequence protects you.
3. **Lean on AI coding agents for boilerplate; review closely anything touching key
   material.** CLI plumbing, test scaffolding, serialization, error types → let an agent
   draft it. Memory-wipe logic, the actual MPC message flow, and shard decryption paths →
   write/review those yourself line by line. You need to defend this in a viva, and this
   is also just where bugs are dangerous.
4. **Every phase = a demo, not just code.** If you can't show it working end-to-end,
   the phase isn't done.

---

## Tech stack — decisions locked in now

| Layer | Choice | Why |
|---|---|---|
| Language | Rust | Per your report; memory safety matters for key material |
| SSS | `vsss-rs` | Already named in your own lit review table |
| Shard encryption | `aes-gcm` + `argon2` crates | AES-256-GCM + Argon2id as specced |
| Memory wipe | `zeroize` | Standard, well-audited zeroing crate |
| secp256k1 signing | `k256` (pure Rust) | No C bindings to fight with |
| Chain interaction | `alloy` — **not** `ethers-rs` | ethers-rs is deprecated; alloy is its successor |
| Testnet | Sepolia (assumption — swap freely) | Free faucet, EVM-standard |
| Threshold ECDSA (Mode B) | `cggmp21` crate (LFDT-Lockness / ex-DFNS) | Audited by Kudelski Security, production-used, supports general t-of-n |
| Anomaly detection | **Open — see Phase 3** | See fog section below |

---

## Phase 0 — Scaffolding [Done]

**Build:** Rust workspace, `cargo` project skeleton, `horcrux` binary with stub
subcommands (`init`, `sign`, `mpc-sign`), git repo, CI running `cargo test` +
`cargo clippy` on push, Sepolia RPC access sorted (Alchemy/Infura free tier or public RPC).

**AI agent fit:** near-total — this is pure boilerplate.

**Visible outcome:** `cargo run -- --help` prints the full command structure; one green
CI check; you can hit the Sepolia RPC and get back a block number.

---

## Phase 1 — SSS + Shard Crypto (Week 1) [Done]

**Build:** `horcrux init` — accept a raw secp256k1 private key (start with a disposable
test key, never a real one), split into N shares via `vsss-rs` at a configurable
threshold (default 2-of-3), derive an AES key per shard from a guardian password via
Argon2id, encrypt with AES-256-GCM, write each encrypted shard + salt + auth tag as a
binary file to a local path (stand-in for a USB mount).

**AI agent fit:** high for the file I/O and CLI wiring; review the crypto glue yourself.

**Visible outcome:** round-trip test — split a test key into 2-of-3 shards, reconstruct
using any 2 via Lagrange interpolation, assert reconstructed key == original. Bonus demo:
feed a wrong password and watch decryption fail cleanly (AES-GCM auth tag rejection).

---

## Phase 2 — Mode A Signing + Broadcast (Week 2) [Done]

**Build:** `horcrux sign` — take t shard paths + passwords, decrypt, reconstruct key in
memory, build and sign a standard EVM transaction with `k256`/`alloy`, immediately
`zeroize` the key, output signed tx hex, broadcast via `alloy` to Sepolia.

**AI agent fit:** medium — alloy's transaction-building API has some boilerplate an
agent can draft, but you should personally verify the memory-wipe timing.

**Visible outcome — your real milestone:** live demo. Two of three shard files +
passwords → reconstructed key in RAM → signed tx → broadcast → transaction visible and
confirmed on Sepolia Etherscan. **At this point you have a complete, defensible project**
even if nothing below gets built.

> **Status — closed** with the commits from `2fc4ac6` through the Phase 2 docs commit.
> See [`agent/discussions/PHASE_2.md`](agent/discussions/PHASE_2.md) for the report.
> Signing happens entirely offline (EIP-1559 by default, legacy via `--gas-price`);
> broadcast is opt-in via `--broadcast` using `$HORCRUX_RPC_URL` (default Sepolia).
> The `k256` zeroize caveat noted at the end of Phase 1 was resolved: `SecretKey` and
> `ecdsa::SigningKey` zeroize the scalar on drop unconditionally, and the alloy signer
> wrapper is compiled with its `zeroize` feature.

---

## Phase 3 — Access Logging + Anomaly Detection (Week 3)

**Build:** log every decryption attempt (timestamp, shard id, success/failure) to a local
append-only log. Score each new signing attempt against historical pattern; flag/block
before signing if anomalous (e.g., odd hour, repeated failures, unfamiliar shard
combination).

**Open decision (pick one before you start):**
- **Python bridge:** a small `scikit-learn` `IsolationForest` script the Rust CLI shells
  out to over stdin/stdout JSON. Matches your report's stated algorithm exactly, lowest
  implementation risk, easy to defend in viva ("we integrated a standard, well-validated
  anomaly model").
- **Rust-native:** a simple statistical/rule-based scorer (z-score on access timing +
  hard rule on failed-attempt count) written directly in Rust. No FFI/subprocess
  complexity, but less literally "Isolation Forest" if you want to keep that exact claim
  in your report.

Given your timeline, the Python-bridge route is the lower-risk default — flag if you'd
rather go Rust-native and we'll scope that version instead.

**AI agent fit:** high for the log plumbing; medium for the scoring logic.

**Visible outcome:** two side-by-side demo runs — a normal access pattern gets approved,
an injected anomalous pattern (e.g., 5 failed attempts, then a 3 AM signing attempt) gets
flagged and blocked before signing proceeds.

---

## Phase 4 — Mode B: True MPC via `cggmp21` (Weeks 4–6)

**This is the highest-risk phase — budget 3 weeks, not 1.**

**Build:** integrate the `cggmp21` crate for threshold key generation (DKG) and threshold
signing across N simulated guardian processes (start on localhost with different ports;
LAN-across-machines is a stretch goal, not a requirement). Each guardian holds one shard
as its key-share input; the coordinator combines partial signatures into a final valid
ECDSA signature. Verify the resulting signature against the derived public key. Broadcast
to Sepolia if time allows.

**Fallback if DKG/networking eats week 5:** degrade to a "trusted dealer" simplification
— use the crate's signing protocol with pre-distributed key shares (skip live
peer-to-peer DKG). You still demonstrate the core claim — key never assembled on one
machine — which is what actually matters for your objectives.

**AI agent fit:** low-to-medium. This is the part you need to understand deeply — expect
to read the crate's docs and the CGGMP21 spec summary yourself, and use AI agents mainly
for the networking/message-passing scaffolding around the crate's API, not the protocol
logic itself.

**Visible outcome:** a 3-terminal demo — guardian 1, guardian 2, coordinator — producing
a valid signature, plus a log/memory inspection showing the full private key never
appears on any single process at any point.

---

## Phase 5 — Integration & Hardening (Week 7)

**Build:** unify the CLI surface (`init` / `sign` / `mpc-sign`), consistent error
handling, a real test suite (unit + integration), update your architecture and sequence
diagrams to match what you actually built (vs. what the report originally sketched),
record a demo script/video as a viva fallback if live demo has issues.

**Visible outcome:** `cargo test` passes end-to-end; a rehearsed demo script that runs
clean twice in a row.

---

## Phase 6 — Buffer + Report Alignment (Week 8)

**Build:** fix whatever broke during rehearsal, reconcile your written report with the
actual implementation (your report already reads like a spec for exactly this system —
good sign, just update anything that changed, like the `alloy` swap), prepare viva
answers for the "why this crate/algorithm" questions (Argon2id vs PBKDF2, why an audited
MPC crate instead of from-scratch, what the anomaly detector actually catches).

**Visible outcome:** final report matches final code; you can answer "why" for every
major choice without hesitating.

---

## Not yet specified (resolve these as you go — don't block on them now)

- Anomaly detection: Python bridge vs Rust-native (decide at start of Phase 3)
- Exact RPC provider (Alchemy / Infura / public Sepolia RPC — any works)
- Mode B demo topology: all-localhost vs. genuinely separate machines on LAN
- Whether to pin `cggmp21` at a stable release vs. tracking the `cggmp24` rename —
  check this right before Phase 4 starts, not now (crate is actively evolving)

## Out of scope (your own report already calls these "Future Scope" — leave them there)

- GUI (Tauri desktop app)
- Multi-chain signing (Bitcoin Schnorr, Solana Ed25519, Cosmos)
- Proactive secret sharing / periodic key refresh
- QR-code based air-gapped Mode B
- Formal third-party security audit

---

## How to keep using this

Come back and tell me which phase you're on or what's blocking you, and we'll work that
one thing through rather than re-planning from the top. If a phase turns out bigger or
smaller than scoped here, we adjust the map, not scrap it.