# HORCRUX — Viva Preparation (Q&A)

Short, defensible answers for the "why this crate / why this algorithm" questions.
Each answer is grounded in the actual code — cite the file so you can jump to it
during the demo. Read each once out loud; the point is that you can *explain*, not
memorize.

---

## 1. Why did you write it in Rust?

- Memory safety without a garbage collector: key material is `&[u8]` on the heap and
  stack, and I control exactly when it exists and when it is wiped (`zeroize`).
- Strong type system caught cross-module mistakes at compile time (e.g. the Solana
  split-crate types `Pubkey`/`Signature`/`Hash` are distinct types, not strings).
- The crates I wanted (see §3) are Rust-native.
- `Cargo` gives a reproducible build and a test story (`cargo test`) I could point at.

## 2. Why Argon2id instead of PBKDF2 / bcrypt / scrypt?

- Argon2 is the Password Hashing Competition winner (2015) and the OWASP-recommended
  choice for new code.
- **Argon2id** = the hybrid variant: data-independent passes (resistant to timing
  side-channels that could leak password length/entropy) + a data-dependent pass
  (resists GPU/ASIC cracking).
- **Memory-hardness**: unlike PBKDF2 (cheap to parallelize on GPUs) and bcrypt (small
  memory footprint), Argon2 needs a tunable amount of RAM per guess — 19 MiB in this
  project (`ARGON2_M_COST` in `src/lib.rs`) — which multiplies the cost of brute force.
- Parameters follow the OWASP "interactive login" recommendation (m=19 MiB, t=2, p=1),
  and the salt is 16 fresh random bytes per shard (`crypto::random_salt`).
- Trade-off acknowledged: Argon2 was optimized for interactive use; for an offline,
  USB-bound key you could argue for even larger m. I picked the standard default so
  the viva answer is "documented, defensible parameters" rather than "arbitrary ones".

## 3. Why use audited crates instead of implementing the crypto yourself?

- The plan's ground rule: integrate mature, audited libraries and explain them, rather
  than reproduce decades of research. A panel respects correct integration of a
  reviewed library far more than a shaky from-scratch implementation.
- Specifically:
  - `vsss-rs` — Shamir's Secret Sharing over any prime field (here the secp256k1
    scalar field, `k256`). Audited/used in production contexts; I verified the round
    trip with my own tests (`src/sss.rs`).
  - `aes-gcm` — AES-256-GCM, the RFC 5288 construction.
  - `argon2` — the reference Rust binding of the Argon2 PHC winner.
  - `frost-ed25519` — Zcash Foundation, **NCC-audited**, implements RFC 9591 FROST
    over Ed25519. This is the one that matters for Mode B — threshold crypto is where
    hand-rolled code dies.
  - `ed25519-dalek` — the standard pure-Rust Ed25519 implementation (matches Solana's
    native scheme).
- What I *did* write and test myself: the file formats, the AAD binding, the
  threshold-consistency checks, the audit scorer, and the orchestration glue between
  these libraries — the security-critical integration surface.

## 4. Why is the Shamir field secp256k1 while signing is Ed25519?

- The SSS field is a **math-only choice**: `vsss-rs` interpolates over any prime
  field, and the only requirement is that the secret (the 32-byte seed) maps into the
  field.
- `k256`'s `Scalar` is a 32-byte prime field element, so a random seed is a valid
  secret with overwhelming probability (~2⁻¹²⁸ to be out of range; rejected with a
  clean error — `src/sss.rs`).
- Signing is Ed25519 because **Solana natively uses Ed25519** — address derivation and
  signature verification are plain Ed25519 (RFC 8032). The original report sketched
  secp256k1/ECDSA because it assumed Ethereum; once the chain swapped to Solana, the
  signing scheme swapped, and only the SSS field stayed k256. Documented in
  `Architecture.md` and `agent/wayfinder/PLAN.md`.

## 5. What does the 83-byte shard format actually contain, and why the AAD?

- `HX1` SSS shard (`src/shard.rs::SHARD_LEN` = 83 bytes):

  ```
  magic(3) | version(1) | threshold(1) | share_count(1) | id(1) | salt(16) | nonce(12) | sealed(32 + 16 tag)
  ```

- `sealed` = the 32-byte share value encrypted with AES-256-GCM under a key derived
  from that shard's guardian password.
- The GCM **additional authenticated data (AAD)** is `[threshold, share_count, id]`.
  Because the AAD is authenticated, a shard cannot be decrypted with the metadata of a
  different split: swapping a shard from a 3-of-3 split into a 2-of-3 set fails at
  decrypt time (`Error::SplitMismatch`, `src/lib.rs`), and a tampered byte flips the
  tag. This is a *cryptographic* binding, not a convention.
- Random salt + random nonce per shard means two `init` runs of the same key produce
  completely different files.

## 6. What does the anomaly detector actually catch?

`src/audit.rs`, a rule-based scorer over the JSON-lines access log. Three signals:

- **Block (hard rule):** 3 trailing `decrypt_fail` entries within the last hour
  (`failed_window = 3`, `fail_lookback_secs = 3600`). A successful decrypt resets the
  run. A blocked attempt is itself logged, so a refusal cannot hide.
- **Warn:** attempt during the "odd hours" window 00:00–06:00 UTC.
- **Warn:** a shard combination never used by a previous completed attempt.
- **Warn:** inter-attempt gap whose z-score exceeds 3.0 against the historical gap
  distribution — i.e. "you always sign every ~2 minutes, now it's been an hour".

Honest limits to state: it is *rule-based*, not a learned model; it detects simple
pattern anomalies, not an attacker who already holds the passwords; and it scores each
attempt *before* any key material is handled (pre-flight in `src/main.rs`), so a
blocked attempt never decrypts anything.

## 7. Why FROST instead of CGGMP21 / threshold-ECDSA?

- **Signature format:** FROST over Ed25519 produces a plain RFC 8032 Ed25519
  signature — exactly what Solana verifies. A threshold-ECDSA (CGGMP21) signature
  would only be useful if the chain were EVM, which it no longer is.
- **Efficiency:** FROST is a two-round, one-shot protocol (no per-message exponent
  multiplication). CGGMP21 is a heavyweight MPC that is overkill for "split a key and
  sign with a threshold of holders".
- **Audited implementation:** `frost-ed25519` (Zcash Foundation, NCC-audited, RFC 9591)
  was available and battle-tested; CGGMP21 crates are fewer and less audited.
- **The output is indistinguishable:** because the group verifying key equals the Mode
  A address (the FROST key is derived from the same seed — `mpc::frost_signing_key`),
  a FROST signature is indistinguishable on-chain from one made by the full key.
  Verified by `frost_group_key_matches_mode_a_address` and cross-checks in the demo.

## 8. Mode A vs Mode B — what is the difference?

- **Mode A (`sign`):** reconstruct the full key in RAM from t shards, sign, wipe. The
  key *exists*, briefly, in one process. Air-gap story: it happens offline; only the
  signed transaction is ever moved to a networked machine.
- **Mode B (`mpc-sign`):** the key is never reconstructed anywhere. Each share file
  contributes round-1 nonces and a round-2 signature share; the coordinator aggregates
  them into a single Ed25519 signature. Only shares, commitments, and signature shares
  exist in memory (`src/mpc.rs::mpc_sign`).
- Both modes produce the *same* sender address and a plain Ed25519 signature, so they
  are interchangeable to the chain.

## 9. The "key never assembled" claim — be precise (trusted dealer).

- True claim: during **signing**, the full key is never assembled on any machine.
- Honest qualifier: this project uses the **trusted-dealer fallback** (explicitly
  allowed in the plan): at `mpc-split` time the dealer does hold the full key to create
  the shares. Live DKG (no single entity ever knowing the key) was planned but dropped
  in favour of a working system — `agent/wayfinder/PLAN.md` documents this as the
  contingency. Say exactly this; the panel will respect the honesty.

## 10. How does offline signing work with Solana?

- A Solana `Message` is fully specified by: the `system_program` transfer instruction,
  the fee payer, and a recent `blockhash` (`src/tx.rs::transaction_message`).
- Offline: the blockhash is supplied via `--blockhash`; the tx is built and signed
  with no network access, and the raw bincode tx is printed as base58.
- Online: `--broadcast` fetches the latest blockhash, signs, sends via
  `solana-rpc-client`, and polls until confirmed (`src/chain.rs::broadcast`).
- There is no nonce/gas/chain-id — Solana only needs the blockhash for replay
  protection. That is the whole network input, which is why air-gapped signing is
  feasible.

## 11. Tell me about a real bug you found and fixed.

- The broadcast step in the demo surfaced this: a freshly-airdropped sender read as
  **0 lamports** through horcrux while the Solana CLI showed 1 SOL. The RPC client used
  the default `finalized` commitment; on a fresh `solana-test-validator` a just-confirmed
  airdrop is not yet *finalized*, so the balance query returned 0 and signing aborted.
  Fix: `Chain::connect` uses `CommitmentConfig::confirmed()` (`src/chain.rs`), matching
  what the Solana CLI does. It demonstrates understanding of Solana's commitment model —
  and that I debug a test failure to the protocol layer, not just patch the symptom.

## 12. Where does zeroization happen?

- `k256::SecretKey` zeroizes its scalar on drop (its `zeroize` impl).
- `ed25519-dalek`'s `SigningKey` zeroizes on drop via its `zeroize` feature (enabled in
  `Cargo.toml`).
- Derived AES keys are wrapped in `zeroize::Zeroizing` (`src/crypto.rs::derive_key`),
  and decrypted share values and the reconstructed seed are zeroized buffers
  (`Zeroizing<Vec<u8>>`, `key_seed`). The seed passed to `tx::sign_transaction` is
  explicitly `zeroize()`d after signing, and the value bytes are zeroized after sealing
  (`src/lib.rs::init_shards`).

## 13. Why is `verify` a separate, passive command?

- `horcrux verify` (`src/verify.rs`) checks magic, version, length, metadata, and — with
  `--password` — the AES-GCM auth tag, without ever producing plaintext and **without
  writing to the access log**. It answers "is my backup intact / did the USB get
  corrupted?" without creating audit noise or touching key material. Cross-file
  consistency is checked (all one kind, one split).

## 14. What are the limitations of the system? (Say these yourself.)

- Security is only as strong as the guardian passwords (Argon2id slows, not stops,
  offline guessing of weak passwords).
- Trusted dealer at split time (see §9); no DKG, no proactive refresh.
- The access log lives on the same machine as the shards in the demo — a real
  deployment would keep the log on a separate observer. The demo runs everything on
  one host to *show* the two-terminal separation conceptually.
- No third-party audit of the whole system; the components are audited, the glue is
  mine, tested but not audited.
- Threshold shares are 32-byte scalars; at n=3, t=2, the math is textbook Shamir —
  the *novelty* is the AAD-bound per-shard encryption + audit + FROST-on-the-same-seed
  integration.

## 15. How is the project tested?

- 66 unit tests across `sss`, `crypto`, `shard`, `chain`, `audit`, `tx`, `mpc`,
  `verify`, `lib` + 25 integration tests (`tests/roundtrip.rs`, `sign.rs`, `audit.rs`,
  `mpc.rs`, `verify.rs`) = **91 tests**.
- Property-flavoured checks: every threshold subset reconstructs, any pair of FROST
  shares signs and verifies, wrong-password/tamper/mixed-split/group-mismatch all fail
  cleanly, block-after-3-failures, GCM tag rejection, signature determinism vs nonce
  freshness.
- End-to-end rehearsal: `./demo.sh --auto` runs the real binary through split →
  reconstruct → failure modes → audit block → FROST → verify → live broadcast to
  `solana-test-validator`, twice in a row.
- CI (`.github/workflows/ci.yml`): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on every push/PR.

---

## One-line elevator pitch

> horcrux splits a Solana private key into encrypted threshold shards that are each
> bound to a guardian password, signs either by briefly reconstructing the key in RAM
> (Mode A) or — never assembling it at all — via FROST threshold signatures (Mode B),
> with an access-log anomaly layer that blocks suspicious attempts before any key
> material is touched.
