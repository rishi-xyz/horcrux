# HORCRUX — Code Map

File-by-file walkthrough for a live viva demo: what each module does, where the key
functions are, and what to point at on screen. Read it top to bottom once, then
practice the walk with `./demo.sh --auto` running.

---

## `src/` — the library

| File | Responsibility | Key items to point at |
|------|----------------|-----------------------|
| `lib.rs` | Top-level orchestration, constants, shard <-> key pipeline | `init_shards`, `reconstruct` / `reconstruct_inner` / `reconstruct_with_audit`, `sign_transaction_from_shards`, `sign_transaction_from_mpc_shares`, `key_seed`, constants `ARGON2_M_COST`, `SHARD_MAGIC` |
| `sss.rs` | Shamir split/combine over the secp256k1 field | `split` (`vsss_rs::shamir::split_secret`), `combine` (Lagrange interpolation) |
| `crypto.rs` | Argon2id KDF + AES-256-GCM seal/open | `derive_key`, `seal`, `open`, `random_salt`, `random_nonce` |
| `shard.rs` | The 83-byte `HX1` file format | `Shard::to_bytes`/`from_bytes`, `aad`, `SHARD_LEN`, `decrypt` |
| `tx.rs` | Offline Solana message build + sign | `transaction_message`, `sign_transaction`, `sign_transaction_with_signature`, `derive_address`, `SignedTx::raw_base58` |
| `chain.rs` | Solana RPC (broadcast only) | `Chain::connect` (confirmed commitment), `broadcast`, `default_rpc_url` |
| `audit.rs` | Append-only JSON access log + rule scorer | `Entry`/`EntryKind`, `AccessLog`, `Scorer::assess`, `Verdict`, `utc_hour`, `format_utc` |
| `mpc.rs` | Mode B: FROST split + sign (`HX2`) | `mpc_split` (dealer), `mpc_sign` (two rounds + aggregate), `frost_signing_key`, `FrostShare`, `MpcSignature`, `GROUP_PUB_FILENAME` |
| `verify.rs` | Passive shard/share verification | `verify_files`, `consistency_error`, `Kind::{Sss,Frost}` |
| `error.rs` | Typed error enum | `Error::Decrypt`, `Error::NotEnoughShares`, `Error::SplitMismatch`, `Error::MpcGroupMismatch`, `Error::Blocked` |

## `src/main.rs` — the CLI

Clap `derive` CLI with subcommands. Show `horcrux --help`.

- `Init` → `init_shards`
- `Reconstruct` → `reconstruct_with_audit`
- `Sign` → `sign_transaction_from_shards` (+ optional `--broadcast`)
- `MpcSplit` → `mpc::mpc_split`
- `MpcSign` → `mpc::mpc_sign_with_audit`
- `Log` → `audit::AccessLog::tail`
- `Verify` → `verify::verify_files` + `consistency_error`

Two functions worth naming in the walk:
- `audit_preflight` (in `main.rs`) — scores the proposed attempt **before** any shard
  is decrypted; `--force` overrides a block but still logs it.
- The shared `--log-file` / `$HORCRUX_ACCESS_LOG` resolution (`access_log_path`).

## `tests/` — integration tests

| File | Covers |
|------|--------|
| `tests/roundtrip.rs` | init → reconstruct for every threshold subset; too-few / wrong-password / mixed-split failures at the public-API level |
| `tests/sign.rs` | offline sign from two of three shards, deterministic output, wrong-password aborts before signing |
| `tests/audit.rs` | block after 3 failures, allow/block verdicts through the real CLI |
| `tests/mpc.rs` | 2-of-3 FROST signs and verifies, non-deterministic signatures, group/split mismatches |
| `tests/verify.rs` | verify SSS + FROST, tampered/truncated files, mixed kinds, auth-tag check |

## `demo.sh` — the rehearsed demo

15 steps, each running the **real binary**:

1. build 2. init/split 3. inspect 83-byte format 4–7. reconstruct (all subsets)
8. wrong password 9. too few shards 10. mixed splits 11. access log + block
12. test suite 13/13b/13c/13d. Mode B FROST + failure modes 14. `verify`
15. live broadcast to `solana-test-validator` (both modes confirmed on-chain).

Run `./demo.sh --auto` (no pauses). `--keep` retains the temp shard files and prints
their path. Note the demo routes its access log into a temp dir so it never pollutes
the repo.

## The two file formats (know these cold)

```
HX1 SSS shard (83 bytes):
  H X 1 | ver | t | n | id | salt(16) | nonce(12) | sealed(32 || tag16)
HX2 FROST share (variable):
  H X 2 | ver | min | max | id | salt(16) | nonce(12) | len(u16) | sealed(payload || tag)
```

`sealed` is AES-256-GCM under an Argon2id-derived key; AAD = `[t, n, id]` for HX1 and
`[min, max, id]` for HX2. That AAD is what cryptographically rejects cross-split mixes.

## Suggested 5-minute walkthrough

1. `./demo.sh --auto` on screen.
2. At step 2–3: pause on `inspect` — explain the 83 bytes, the per-shard salt/nonce,
   the AAD binding, and that each shard has its own guardian password.
3. At step 8: wrong password → GCM tag rejection (`Error::Decrypt`).
4. At step 11: the audit block — "3 failures in the hour, and note the refusal is
   itself logged".
5. At steps 13/13c: Mode B — "same address as Mode A, and look, signatures differ each
   run because FROST uses fresh nonces".
6. At step 15: live broadcast — "both a reconstructed-key signature and a FROST
   signature land on-chain as ordinary Ed25519 transactions".
7. Close with `cargo test` count (91) and the CI file.
