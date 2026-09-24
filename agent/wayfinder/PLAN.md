# HORCRUX — Build Plan
### Final-year major project · 8-week timeline 

---

## Destination

A working Rust CLI (`horcrux`) that: splits a private key via Shamir's Secret Sharing,
binds encrypted shards to USB-simulated paths, signs and broadcasts a **Solana**
transaction in two modes (Mode A: air-gapped RAM reconstruction, Mode B: true
threshold-signing MPC where the key is never assembled anywhere), and flags anomalous
access attempts via a lightweight anomaly-detection layer. Every phase below ends in
something you can literally run and show someone.

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
| Key seed & field | `k256` (pure Rust) | Still the SSS field (math-only choice, Phase 1 unchanged) |
| Ed25519 signing | `ed25519-dalek` | Solana's native signature scheme; `verify_strict` rejects malleated sigs |
| Chain interaction | `solana-rpc-client` (3.x split crates) | The supported post-`solana-sdk`-umbrella layout for RPC |
| Testnet | **localnet** (`solana-test-validator`) default; devnet via `--rpc-url` | Runs locally with pre-funded keypair, no faucet needed |
| Threshold MPC (Mode B) | `frost-ed25519` (Zcash Foundation) | NCC-audited FROST over Ed25519; produces a plain Ed25519 signature |
| Anomaly detection | **Open — see Phase 3** | See fog section below |

---

## Phase 0 — Scaffolding [Done]

**Build:** Rust workspace, `cargo` project skeleton, `horcrux` binary with stub
subcommands (`init`, `sign`, `mpc-sign`), git repo, CI running `cargo test` +
`cargo clippy` on push, local Solana RPC access sorted (`solana-test-validator` on
localhost, default port 8899).

**AI agent fit:** near-total — this is pure boilerplate.

**Visible outcome:** `cargo run -- --help` prints the full command structure; one green
CI check; you can hit the local validator RPC and get back the latest blockhash.

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

> **Field note (locked in):** the Shamir field stays secp256k1/`k256` even though signing
> is now Ed25519. The field is a math-only choice — `vsss-rs` interpolates over any prime
> field, and any 32 bytes is a valid Ed25519 seed. A random 32-byte seed has ~2⁻¹²⁸
> probability of falling outside the k256 field (rejected with a clean error); generated
> keys always succeed because `k256` samples within the field. Swap-in candidate only if a
> reviewer objects; not planned.

---

## Phase 2 — Mode A Signing + Broadcast (Week 2) [Done]

**Build:** `horcrux sign` — take t shard paths + passwords, decrypt, reconstruct key in
memory, derive the Ed25519/Solana address, build an unsigned Solana `Message`
(`system_program` transfer), sign it with `ed25519-dalek`, immediately `zeroize` the key,
output the base58 address + signature, broadcast via `solana-rpc-client` to localnet.

**AI agent fit:** medium — the Solana 3.x split-crate API (`solana-message`,
`solana-transaction`, `solana-system-interface`, `solana-rpc-client`) has boilerplate an
agent can draft, but you should personally verify the memory-wipe timing and that the
signature is over `message.hash()` — not the bincode message bytes.

**Visible outcome — your real milestone:** live demo. Two of three shard files +
passwords → reconstructed key in RAM → signed tx → broadcast → tx confirmed on the
`solana-test-validator` log. **At this point you have a complete, defensible project**
even if nothing below gets built.

> **Status — done.** Phase 2 is fully implemented and verified against a live local
> validator (commits `7382351`, `d826bec`): offline sign + broadcast, fee-aware errors,
> and a confirmed on-chain transfer. Key deltas from the original EVM plan:
> no gas/nonce/chain-id — the only network input is a recent blockhash
> (`get_latest_blockhash()`), fetched live or supplied offline via `--blockhash`. Address
> space: seed bytes (0..32) = signing key, (32..64) = derived pubkey, (64..) = system
> program id `11111111111111111111111111111111`. `zeroize` story: `k256::SecretKey`
> zeroizes the scalar on drop; `ed25519-dalek`'s `SigningKey` zeroizes via its `zeroize`
> feature.

---

## Phase 3 — Access Logging + Anomaly Detection (Week 3) [Done]

**Build:** log every decryption attempt (timestamp, shard id, success/failure) to a local
append-only log. Score each new signing attempt against historical pattern; flag/block
before signing if anomalous (e.g., odd hour, repeated failures, unfamiliar shard
combination).

**Decision made: Rust-native scorer** (no Python/IsolationForest bridge). Rationale: one
binary, no FFI/subprocess, deterministic unit tests, and an easy viva story — a standard
statistical/rule-based model (z-scores + hard rules) rather than a thin wrapper.

**What shipped (commits `820d762`):**
- `src/audit.rs` — JSON-lines `AccessLog` (append-only, `./horcrux-access.log` default,
  `--log-file`/`$HORCRUX_ACCESS_LOG` override) recording `decrypt_ok`/`decrypt_fail`/
  `blocked` per shard; `Scorer` with `Verdict::{Allow,Warn,Block}`.
- Rules: **Block** after 3 trailing decrypt failures within the last hour (success resets
  the run); **Warn** on odd UTC hour (00:00–06:00, hand-rolled conversion — no `chrono`),
  unfamiliar shard-id combination, and inter-attempt gap z-score > 3. Blocked attempts are
  logged too, so a refused attempt cannot hide.
- CLI: `sign`/`reconstruct` run a pre-flight check before any key material is handled;
  `--force` overrides a block (still logged); `horcrux log [--tail N] [--json]` views the
  log. `reconstruct_with_audit` records each shard decrypt outcome (best-effort logging).
- Tests: 14 scorer/log unit tests + `tests/audit.rs` (block/allow/log integration).
  `cargo test` = 48 unit + 8 roundtrip + 3 sign + 4 audit, all green.

**Visible outcome:** two side-by-side demo runs — a normal access pattern gets approved,
an injected anomalous pattern (e.g., 5 failed attempts, then a 3 AM signing attempt) gets
flagged and blocked before signing proceeds (see demo.sh step 11).

---

## Phase 4 — Mode B: True Threshold MPC via FROST (Weeks 4–6) [Done]

**This is the highest-risk phase — budget 3 weeks, not 1.**

**Build:** integrate the `frost-ed25519` crate (Zcash Foundation, NCC-audited, v3.0.0,
published 2026-04-23) for threshold signing across N simulated guardian processes (start
on localhost with different ports; LAN-across-machines is a stretch goal, not a
requirement). Each guardian holds one key share; the coordinator combines partial
signatures into a final valid Ed25519 signature — verifiable as a plain Ed25519 signature
against the derived public key. Sign the same Solana `Message` built in Phase 2 and
broadcast to localnet if time allows.

**Fallback if DKG/networking eats week 5:** degrade to a "trusted dealer" simplification
— use the crate's signing protocol with pre-distributed key shares (skip live peer-to-peer
DKG). You still demonstrate the core claim — key never assembled on one machine — which
is what actually matters for your objectives.

**AI agent fit:** low-to-medium. This is the part you need to understand deeply — expect
to read the crate's docs and the FROST spec summary yourself, and use AI agents mainly
for the networking/message-passing scaffolding around the crate's API, not the protocol
logic itself.

**Visible outcome:** a 3-terminal demo — guardian 1, guardian 2, coordinator — producing
a valid signature, plus a log/memory inspection showing the full private key never
appears on any single process at any point.

> **Status — done.** The **trusted-dealer fallback** was chosen (per the plan's own
> contingency): `horcrux mpc-split` dealer-splits the Mode A seed with `frost-ed25519`
> into t-of-n encrypted key shares (default 2-of-3, `mpc-{id}.hx` + non-secret
> `group.pub`), and `horcrux mpc-sign` runs the two-round protocol with one in-process
> participant per share file — the full key is never assembled on any machine. Because
> the split derives the FROST group from the same 32-byte seed, the group address equals
> the Mode A wallet address, and the aggregated signature is an ordinary RFC 8032 Ed25519
> signature verifiable by any verifier (RFC 9591 compatibility), so it flows through the
> Phase 2 Solana sign/broadcast path unchanged. Security properties verified by tests:
> any threshold subset signs and verifies under plain Ed25519, one-share and
> wrong-password attempts fail cleanly, shares from different groups are rejected, and
> SSS shards cannot be misused as FROST shares (distinct `HX1`/`HX2` file magics). The
> audit layer logs each participant decrypt and a final `signed` entry. Gates green:
> `cargo fmt`, `cargo clippy --all-targets --offline -- -D warnings`, `cargo test`
> (58 unit + 22 integration = 80 tests), and the full `./demo.sh --auto` run.

---

## Phase 5 — Integration & Hardening (Week 7)

**Build:** unify the CLI surface (`init` / `sign` / `mpc-sign`), consistent error
handling, a real test suite (unit + integration), update your architecture and sequence
diagrams to match what you actually built (vs. what the report originally sketched),
record a demo script/video as a viva fallback if live demo has issues.

**Visible outcome:** `cargo test` passes end-to-end; a rehearsed demo script that runs
clean twice in a row.

> **Status — done.** Added the `horcrux verify` subcommand (HX1 shards / HX2 shares):
> passive structural checks — magic, format version, exact length, metadata, cross-file
> split consistency, and (with `--password`) each file's AES-GCM auth tag — exiting
> non-zero on any failure and never touching the access log. Removed the one unused
> dependency (`tempfile`) and dropped the LICENSE until a license is chosen. Rewrote
> `Architecture.md` to match the shipped system (Ed25519/FROST, no EVM, no Isolation
> Forest) and fixed README drift: real repo structure, prerequisites, updated CLI tree,
> corrected test counts, and a rewritten Configuration section (env vars + CLI flags
> instead of a TOML file that never existed). `demo.sh` gained two steps — passive
> `verify` (step 14) and a live broadcast to `solana-test-validator` (step 15) that
> funds the derived sender, broadcasts Mode A and Mode B transfers, and confirms both
> on-chain. Two real bugs surfaced by the new broadcast step and fixed: the RPC client
> read balances at `finalized` commitment (invisible 0-balance on a fresh localnet —
> now `confirmed`), and transfers to a non-existent recipient failed rent simulation
> (demo now funds the recipient too). Gates green: `cargo fmt`, `cargo clippy
> --all-targets --offline -- -D warnings`, `cargo test` (66 unit + 25 integration =
> 91 tests), and `./demo.sh --auto` runs clean twice in a row, including the live
> on-chain broadcast.

---

## Phase 6 — Buffer + Report Alignment (Week 8)

**Build:** fix whatever broke during rehearsal, reconcile your written report with the
actual implementation (your report already reads like a spec for exactly this system —
good sign, just update anything that changed, like the EVM→Solana swap and the
CGGMP21→FROST swap), prepare viva answers for the "why this crate/algorithm" questions
(Argon2id vs PBKDF2, why an audited MPC crate instead of from-scratch, what the anomaly
detector actually catches).

**Visible outcome:** final report matches final code; you can answer "why" for every
major choice without hesitating.

---

## Phase 7 — Bonus: Multi-Chain Signing, Bitcoin Mode B, & QR Air-Gapped Mode B (out-of-plan)

**Not scoped by the original 8-week plan** — everything in this phase was explicitly
listed under "Out of scope" in an earlier revision of this doc. A collaborator
(`Ftabs305`) first added Bitcoin/Cosmos Mode A signing and a library-only QR/HX3
protocol; that work was then completed (CLI wiring, tests, Bitcoin Mode B, broadcast
support) so the plan matches what `main` actually claims. The report should note this
phase as an extension beyond the original objectives, not a core deliverable — Mode A
(Phase 2) and Solana Mode B (Phase 4) alone already satisfy the project's stated
objectives.

**Multi-chain Mode A signing (commit `834a082`, then extended) — done, wired,
tested, with broadcast.** `horcrux sign --chain bitcoin|cosmos` (default `solana`)
signs with the same reconstructed secp256k1/Ed25519 seed on two more chains
(`src/bitcoin.rs`, `src/cosmos.rs`):
- **Bitcoin:** offline Taproot (BIP340/341/342) key-path signing, as originally
  shipped (tweaked P2TR output key via `H_taptweak(P)`, no script tree).
  **Broadcast added:** `--broadcast` calls `sendrawtransaction` against a Bitcoin
  Core node (`bitcoincore-rpc`, default `http://127.0.0.1:18443` — regtest —
  override with `--rpc-url`/`$HORCRUX_BTC_RPC_URL`; `--rpc-user`/`--rpc-password`
  for RPC auth).
- **Cosmos:** offline `bank.MsgSend`/`SIGN_MODE_DIRECT` ECDSA signing, as
  originally shipped. **Broadcast added:** `--broadcast` calls
  `/broadcast_tx_commit` on a Tendermint RPC node (`cosmrs::rpc` /
  `tendermint-rpc`, default `http://127.0.0.1:26657` — override with
  `--rpc-url`/`$HORCRUX_COSMOS_RPC_URL`) and treats a non-OK `CheckTx`/delivery
  result as an error rather than a silent "success".
- Verified: unit tests for both modules, plus the full CLI path exercised live in
  `demo.sh` (Bitcoin Mode A/B signing, and — when `bitcoind`/`bitcoin-cli` are
  installed — a real regtest broadcast). **Not verified in this environment:**
  neither `bitcoind` nor a Cosmos node (`gaiad`/`simd`) was available in the
  sandbox this was built in, so Bitcoin broadcast was checked against
  `cargo build`/unit tests only (the regtest `demo.sh` step is written and
  gated exactly like the existing Solana step, but not run end-to-end here), and
  Cosmos broadcast has no automated coverage at all beyond compiling — both are
  worth a manual pass against a real node before relying on them.

**Bitcoin Mode B (new, `src/btc_mpc.rs`) — done, wired, tested.**
`horcrux mpc-split`/`mpc-sign --chain bitcoin` dealer-splits and threshold-signs
using `frost-secp256k1-tr` (Zcash Foundation, same 3.0.0 lineage as
`frost-ed25519`), mirroring `src/mpc.rs`'s architecture exactly: shares hold the
*untweaked* internal FROST key, and the BIP341 tweak (no script tree) is applied
at sign/aggregate time via the crate's own `round2::sign_with_tweak` /
`aggregate_with_tweak` / `keys::Tweak` — never hand-rolled. Bitcoin FROST shares
use their own `HX4` file magic and `group-btc.pub` filename (distinct from
Solana's `HX2`/`group.pub` and the SSS `HX1`), so files from different
chains/protocols can never be cross-used — asserted by a dedicated test.
**Security property verified by test:** the Bitcoin Mode B group's tweaked P2TR
address equals the Bitcoin Mode A address for the same seed
(`btc_mpc::tests::frost_group_address_matches_mode_a_address`), exactly
mirroring the existing Solana Mode A/B parity property. `--chain cosmos` is
rejected on both `mpc-split` and `mpc-sign` with an explicit error: **Cosmos Mode
B is out of scope, not merely unbuilt** — Cosmos uses plain ECDSA/secp256k1,
which needs a fundamentally different threshold protocol (GG18/GG20/CGGMP21:
multiplicative-to-additive share conversion, Paillier encryption, zero-knowledge
proofs) than FROST/Schnorr, and no mature audited Rust crate for threshold ECDSA
exists yet — per this plan's ground rule #1 ("don't implement cryptography from
scratch, use audited crates only"), that makes it a deliberate boundary rather
than missing work.

**QR-based air-gapped Mode B (`src/qr.rs`, `src/qr_mpc.rs`) — done, wired,
tested, with real QR images.** The HX3 frame protocol and FROST air-gap logic
that shipped library-only now has:
- **real QR-code PNG transport** (`qrcode` for encoding, `image` for PNG I/O,
  `rqrr` for pure-Rust decoding) — `write_qr_png`/`read_qr_png` round-trip
  correctly at the maximum single-frame payload (450 bytes), verified by test;
- **five CLI subcommands** (`qr-request`, `qr-commit`, `qr-package`, `qr-share`,
  `qr-finalize`) implementing the full 4-message protocol (request → commitment →
  signing package → signature share → finalize) over files in a shared `--dir`
  (`--format qr` for PNGs, `--format hx3` for a single frame-collection file);
  round-1 nonces are kept in a small encrypted local file next to each share
  (never written into `--dir`) between `qr-commit` and `qr-share`, and deleted
  once consumed;
- **tests**: 11 unit tests in `qr.rs` (frame/base58/multi-frame round trips, QR
  PNG round trip at max payload, transport-format round trips, participant-id
  scanning) and an integration test (`tests/qr.rs`) running the complete
  protocol through real QR PNG files for every 2-of-3 participant combination,
  asserting the aggregated signature verifies under plain Ed25519 and matches
  the in-process Mode B sender address;
- **`demo.sh` step 14b** runs the full five-command flow live and asserts the
  QR-derived sender matches Mode B's in-process sender.
- Scope note: QR Mode B remains Solana-only (it reuses `qr_mpc.rs`'s
  `frost_ed25519` import); a Bitcoin QR flow was not attempted.

---

## Not yet specified (resolve these as you go — don't block on them now)

- Anomaly detection: Python bridge vs Rust-native (decide at start of Phase 3)
- Mode B demo topology: all-localhost vs. genuinely separate machines on LAN
- Solana CLI install path: `solana-install` vs `agave-install` (decide at Phase 0/2
  demo time; `solana-test-validator` from either works)
- Whether to pin `frost-ed25519` at 3.0.x vs. tracking newer releases (pin at 3.0.0;
  re-check right before Phase 4 starts, not now)
- Bitcoin regtest and Cosmos broadcast (Phase 7): re-verify against real
  `bitcoind`/`gaiad`/`simd` nodes — only checked by compilation/unit tests and an
  untested-in-sandbox `demo.sh` step in the environment this was built in

## Out of scope (your own report already calls these "Future Scope" — leave them there)

- GUI (Tauri desktop app)
- Proactive secret sharing / periodic key refresh
- Formal third-party security audit

---

## How to keep using this

Come back and tell me which phase you're on or what's blocking you, and we'll work that
one thing through rather than re-planning from the top. If a phase turns out bigger or
smaller than scoped here, we adjust the map, not scrap it.