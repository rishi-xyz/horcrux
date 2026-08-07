# HORCRUX

> **Offline Threshold Key Management System for Blockchain Private Keys**

HORCRUX is a Rust-based offline threshold key management system that eliminates the single point of failure associated with traditional cryptocurrency wallets. Instead of storing an entire private key on one device, HORCRUX distributes cryptographic control across multiple trusted guardians using **Shamir's Secret Sharing (SSS)** and supports **FROST threshold Multi-Party Computation (MPC)** for secure transaction signing.

Designed for self-custody, security-critical environments, and blockchain infrastructure, HORCRUX enables users to securely split, store, recover, and sign blockchain transactions without relying on cloud providers or centralized custody services.

Unlike traditional wallets, HORCRUX treats key management as an entire lifecycle rather than a single storage problem.

---

## Table of Contents

- Introduction
- Why HORCRUX?
- Features
- Project Objectives
- System Overview
- Core Concepts
- System Workflow
- Project Architecture
- Technology Stack
- Repository Structure
- Installation
- Building
- Usage
- CLI Commands
- Cryptographic Design
- Security Model
- Threat Model
- Project Modules
- Future Roadmap
- References
- License

---

# Why HORCRUX?

Private keys remain the weakest point in blockchain security.

Existing wallet architectures generally suffer from one or more of the following issues:

- Single device failure
- Plaintext backups
- Vendor dependence
- Internet dependency
- Expensive hardware requirements
- Limited recovery options

HORCRUX addresses these limitations by introducing a completely offline threshold key management workflow.

Instead of protecting one secret, HORCRUX protects multiple encrypted fragments of that secret.

Each fragment is:

- individually encrypted
- password protected
- stored on a dedicated USB drive
- owned by a different guardian

Only when the required threshold of guardians cooperate can a transaction be signed.

---

# Features

## Offline First

Every critical cryptographic operation can be performed completely offline.

Private keys never need internet connectivity.

---

## Threshold Cryptography

Supports configurable threshold schemes including

- 2-of-3
- 3-of-5
- 5-of-7
- N-of-M

using Shamir's Secret Sharing.

---

## USB Bound Storage

Each shard is physically written to a guardian USB device.

Stealing one USB does not compromise the key.

---

## Password Hardened Encryption

Every shard is encrypted using

- AES-256-GCM
- Argon2id
- Random Salt
- Authentication Tag

before leaving memory.

---

## Dual Signing Modes

### Mode A

Air-gapped reconstruction.

Private key exists only inside locked RAM for a few milliseconds before being securely erased.

### Mode B

True Threshold MPC (FROST).

The private key never exists on any machine.

Guardians each hold one encrypted key share and collaboratively produce a
valid Ed25519 signature without the key ever being reconstructed.

---

## Solana Compatible

Uses

- Ed25519 for signing
- secp256k1 field for Shamir secret sharing
- Compatible with Solana (local validator, devnet)

---

## Memory Safety

Implemented in Rust.

Sensitive memory is wiped immediately after use using secure zeroization.

---

## AI-Based Behavioral Security

Beyond cryptography, HORCRUX includes behavioral anomaly detection capable of identifying:

- unusual signing times
- repeated password failures
- abnormal guardian participation
- suspicious access patterns

---

# Project Objectives

HORCRUX aims to solve the complete lifecycle of blockchain private key management.

The primary objectives are:

- Eliminate single-point key storage.
- Securely split keys using threshold cryptography.
- Bind shards to physical USB devices.
- Encrypt every shard independently.
- Support offline signing.
- Support threshold MPC signing.
- Preserve compatibility with Solana (Ed25519).
- Detect suspicious access behavior.
- Maintain complete self custody.

---

# System Overview

HORCRUX consists of three primary operational phases.

```
                Setup
                   │
                   ▼
         Split Private Key
                   │
                   ▼
          Encrypt Each Shard
                   │
                   ▼
          Write To USB Drives
                   │
      ─────────────────────────
                   │
             Distribution
                   │
                   ▼
        Guardian Storage Phase
                   │
      ─────────────────────────
                   │
              Signing Phase
          ┌────────────────┐
          │                │
          ▼                ▼
      Mode A           Mode B
      Air Gap            MPC
```

---

# Core Concepts

## Shamir Secret Sharing

The original private key is mathematically divided into **N** independent shares.

A configurable threshold **T** determines the minimum number of shares required for reconstruction.

Example:

```
Secret

↓

Split

↓

Shard A
Shard B
Shard C

Threshold = 2

A+B ✔
A+C ✔
B+C ✔

Only A ✘
Only B ✘
Only C ✘
```

Any subset smaller than the threshold reveals no information about the original secret.

---

## Guardian Model

HORCRUX introduces the concept of **Guardians**.

Each guardian owns:

- one encrypted shard
- one USB drive
- one password

No guardian possesses the complete key.

---

## Two-Factor Custody

Every shard requires:

Physical possession

AND

Knowledge of the guardian password.

Stealing only the USB drive is insufficient.

Knowing only the password is insufficient.

---

## Air-Gapped Security

Mode A enables completely offline signing.

Network connectivity is unnecessary.

Suitable for:

- cold wallets
- treasury management
- institutional custody
- high value transactions

---

## Threshold MPC

Mode B uses FROST threshold Ed25519 signing (RFC 9591).

Instead of reconstructing the private key:

```
Guardian A

+

Guardian B

+

Guardian C

↓

Signature Shares

↓

Coordinator

↓

Valid Ed25519 Signature
```

The private key never appears anywhere.

---

# System Workflow

## Phase 1

Initialization

```
Private Key

↓

HORCRUX CLI

↓

SSS Module

↓

Generate N Shards
```

---

## Phase 2

Shard Protection

```
Guardian Password

↓

Argon2id

↓

AES-256 Key

↓

AES-GCM Encryption

↓

Encrypted Shard
```

---

## Phase 3

Distribution

```
Encrypted Shard

↓

USB Drive

↓

Guardian
```

---

## Phase 4

Signing

Two execution modes exist.

### Mode A

```
USB

↓

Decrypt

↓

Reconstruct Key

↓

Sign Transaction

↓

Zeroize Memory

↓

Output Signed Transaction
```

---

### Mode B

```
Guardian 1

↓

Guardian 2

↓

Guardian 3

↓

FROST Threshold Protocol

↓

Ed25519 Signature

↓

Broadcast
```

---

# Project Architecture

The project is divided into multiple independent cryptographic modules.

```
HORCRUX
│
├── CLI
├── Config
├── Authentication
├── SSS Module
├── Encryption Module
├── USB Storage
├── MPC Module
├── Signing Module
├── Blockchain Module
├── AI Module
└── Logging Module
```

Each module has a clearly defined responsibility to simplify auditing, testing, and future extension.

---

# Technology Stack

| Layer | Technology |
|----------|------------|
| Language | Rust |
| CLI | clap |
| Secret Sharing | vsss-rs |
| Encryption | AES-256-GCM |
| Password KDF | Argon2id |
| Zeroization | zeroize |
| Elliptic Curve | Ed25519 (signing), secp256k1 (Shamir field) |
| Chain Integration | solana-rpc-client (3.x split crates) |
| MPC | FROST (frost-ed25519) |
| AI | Isolation Forest |
| Serialization | serde |
| Storage | Binary Shard Files |
| USB Medium | FAT32 / exFAT Drives |

---

# Repository Structure

```
horcrux/
│
├── src/
│   ├── cli/
│   ├── crypto/
│   ├── sss/
│   ├── encryption/
│   ├── authentication/
│   ├── storage/
│   ├── signing/
│   ├── mpc/
│   ├── blockchain/
│   ├── ai/
│   ├── logging/
│   └── utils/
│
├── tests/
│
├── examples/
│
├── docs/
│
├── assets/
│
├── README.md
├── ARCHITECTURE.md
├── PLAN.md
├── Cargo.toml
└── LICENSE
```
---

# Installation

## Prerequisites

Before building HORCRUX, ensure the following tools are installed.

| Requirement | Version |
|-------------|---------|
| Rust | Stable (latest) |
| Cargo | Latest |
| Git | Latest |
| OpenSSL | Latest |
| USB Drive(s) | Recommended |
| Linux / macOS / Windows | Supported |

Verify your installation:

```bash
rustc --version
cargo --version
```

---

## Clone Repository

```bash
git clone https://github.com/<username>/horcrux.git

cd horcrux
```

---

## Build

Development

```bash
cargo build
```

Release

```bash
cargo build --release
```

The executable will be available at

```
target/release/horcrux
```

---

## Running Tests

Run all tests

```bash
cargo test
```

Run integration tests

```bash
cargo test --tests
```

Run documentation tests

```bash
cargo test --doc
```

---

## Linting

```bash
cargo fmt

cargo clippy
```

---

# Getting Started

A typical HORCRUX workflow consists of four steps.

```
Generate Wallet

↓

Initialize HORCRUX

↓

Distribute USB Shards

↓

Sign Transactions
```

---

# CLI Commands

HORCRUX exposes a simple command-line interface.

```
horcrux
├── init
├── reconstruct
├── sign
├── mpc-split
├── mpc-sign
├── log
└── help
```

---

## Initialize

Creates encrypted shards from a private key.

```bash
horcrux init
```

Example

```bash
horcrux init \
    --threshold 2 \
    --shares 3 \
    --out-dir ./usb
```

Process

```
Private Key

↓

Split

↓

Encrypt

↓

Write USB Shards

↓

Destroy Original Key
```

---

## Sign

Mode A signing: decrypt a threshold of shards, reconstruct the key in RAM, build and
sign a Solana transaction offline, wipe the key, and output the signed transaction.

```bash
horcrux sign \
    ./usb/shard-1.hx ./usb/shard-2.hx \
    --password guardian \
    --to RecipientAddressBase58 \
    --lamports 1000000000 \
    --blockhash <recent-base58-blockhash>
```

(`--password` applies to every shard; omit it to be prompted per shard.)

- Required: `--to` (base58), `--lamports` (1 SOL = 1_000_000_000 lamports), and
  `--blockhash`. A blockhash must be supplied offline; it is only valid for the
  block window in which it was produced, so fetch a fresh one at signing time
  (e.g. `solana blockhash`).
- Optional: `--rpc-url` (overrides `$HORCRUX_RPC_URL`, default
  `http://127.0.0.1:8899`) and `--broadcast`.
- Audit: `--log-file` (default `./horcrux-access.log` or `$HORCRUX_ACCESS_LOG`)
  records every shard decryption, and `--force` bypasses an audit block.

Broadcast to the cluster (local validator by default), fetching the latest
blockhash and waiting for confirmation:

```bash
HORCRUX_RPC_URL=http://127.0.0.1:8899 \
horcrux sign \
    ./usb/shard-1.hx ./usb/shard-2.hx \
    --password guardian \
    --to RecipientAddressBase58 \
    --lamports 1000000000 \
    --broadcast
```

The sender address is derived from the reconstructed key; when broadcasting,
its balance is checked (airdrop lamports first with `solana airdrop 1 <addr>`).

Output

```
Signed Transaction

↓

Raw Base58 (bincode)

↓

Broadcast (opt-in)
```

---

## MPC Signing (Mode B)

FROST threshold signing: dealer-split a key into encrypted key shares, then
sign with any threshold subset. The full signing key is never reconstructed on
any machine.

Split the key (writes `mpc-{id}.hx` share files plus a non-secret
`group.pub`):

```bash
horcrux mpc-split \
    --threshold 2 \
    --shares 3 \
    --out-dir ./mpc
```

Sign with a threshold subset of share files:

```bash
horcrux mpc-sign \
    ./mpc/mpc-1.hx ./mpc/mpc-2.hx \
    --group-dir ./mpc \
    --password guardian \
    --to RecipientAddressBase58 \
    --lamports 1000000000 \
    --blockhash <recent-base58-blockhash>
```

Each share file is its own in-process participant in the two-round FROST
protocol (round 1: nonce commitments, round 2: signature shares), which the
coordinator aggregates into a single Ed25519 signature. Only nonces,
commitments, and signature shares ever exist in memory.

- Flags mirror `sign`: `--broadcast` (plus optional `--rpc-url`) fetches the
  blockhash and broadcasts; `--log-file`/`$HORCRUX_ACCESS_LOG` records every
  participant decrypt plus a final `signed` entry; `--force` overrides an audit
  block.
- The group address (the `From` line) equals the Mode A wallet address, so a
  FROST-signed transaction is indistinguishable from one signed with the full
  key — and can be broadcast to Solana unchanged.

Mode B flow:

```
Decrypt Each Key Share
↓
Round 1: Nonce Commitments
↓
Round 2: Signature Shares
↓
Aggregate
↓
Valid Ed25519 Signature
```

---

## Verify

Verify shard integrity.

```bash
horcrux verify
```

Checks

- Authentication tag

- Corruption

- Metadata

- Version

---

## Log

View the access log (JSON-lines, append-only).

```bash
horcrux log
horcrux log --tail 10
horcrux log --json
```

Shows

- timestamp (UTC)

- shard id

- outcome: `ok` / `fail` / `blocked` / `signed`

---

## Access Logging & Anomaly Detection

Every shard decryption attempt is recorded to an **append-only** access log as a
JSON-lines entry:

```json
{"ts":1767225600000,"attempt":12345,"shard_id":1,"kind":"decrypt_ok"}
```

No passwords or key material are ever logged. Before signing (or
reconstructing), the audit layer (src/audit.rs) scores the attempt against the
log and returns a verdict:

| Signal | Effect |
|--------|--------|
| 3+ trailing decrypt failures within the last hour | **Block** |
| Attempt during unusual hours (UTC 00:00–06:00) | Warn |
| Shard combination never used before | Warn |
| Gap since last access has z-score > 3 | Warn |

A **Block** refuses the operation before any key material is handled; the
refused attempt is itself logged. `--force` overrides a block (still logged).

```bash
# normal history -> signing proceeds and is logged
horcrux sign shard-1.hx shard-2.hx --password guardian \
    --to RecipientAddressBase58 --lamports 1000 --blockhash <base58>

# three wrong-password attempts...
horcrux sign shard-1.hx shard-2.hx --password wrong ...
horcrux sign shard-1.hx shard-2.hx --password wrong ...
horcrux sign shard-1.hx shard-2.hx --password wrong ...

# ...then a legitimate attempt is refused before signing
horcrux sign shard-1.hx shard-2.hx --password guardian ...
# access audit blocked the attempt: 3 failed decrypt attempts within the last 3600s
```

---

# Configuration

HORCRUX stores runtime configuration separately from encrypted shard data.

Example

```toml
threshold = 2

shares = 3

curve = "secp256k1"

mode = "offline"

rpc = "http://127.0.0.1:8899"

logging = true

anomaly_detection = true
```

---

# Cryptographic Design

HORCRUX combines several established cryptographic primitives.

```
Private Key

↓

Shamir Secret Sharing

↓

Independent Shares

↓

Argon2id

↓

AES-256-GCM

↓

USB Storage
```

Every primitive solves one specific security problem.

---

## Shamir Secret Sharing

Purpose

Distribute trust.

Input

```
Private Key
```

Output

```
Share 1

Share 2

Share 3
```

Properties

✔ Information theoretic security

✔ Configurable threshold

✔ Random polynomial generation

✔ Lagrange reconstruction

---

## Argon2id

Purpose

Password hardening.

Transforms

```
Password

↓

Salt

↓

Memory Hard KDF

↓

256-bit Encryption Key
```

Benefits

- GPU resistant

- ASIC resistant

- Memory hard

- Recommended by OWASP

---

## AES-256-GCM

Purpose

Encrypt shard files.

Provides

- Confidentiality

- Integrity

- Authentication

Every encrypted shard contains

```
Salt

Nonce

Ciphertext

Authentication Tag
```

Any modification immediately invalidates the shard.

---

## Zeroization

Sensitive memory is erased immediately after use.

Protected data includes

- decrypted shard

- reconstructed private key

- passwords

- derived AES keys

Memory lifecycle

```
Allocate

↓

Use

↓

Overwrite

↓

Release
```

---

## Ed25519

Used for

- Solana (native Ed25519 signatures)

---

## FROST Threshold Ed25519

Mode B implements threshold signing.

Instead of reconstructing

```
Key

↓

Sign
```

HORCRUX performs

```
Guardian

↓

Partial Signature

↓

Coordinator

↓

Ed25519 Signature
```

Advantages

✔ Key never reconstructed

✔ Standard Ed25519 output

✔ Compatible with Solana signatures

✔ No blockchain modifications

---

# Security Model

HORCRUX follows a layered defense strategy.

```
Physical Security

↓

Password Security

↓

Cryptography

↓

Memory Safety

↓

Behavior Analysis
```

Every layer assumes another layer may eventually fail.

---

## Security Layers

### Layer 1

Guardian separation

No individual possesses the complete key.

---

### Layer 2

USB custody

Physical theft alone cannot recover a shard.

---

### Layer 3

Password protection

Argon2id derived encryption prevents brute-force attacks.

---

### Layer 4

Authenticated encryption

AES-GCM detects tampering automatically.

---

### Layer 5

Threshold cryptography

Multiple guardians must cooperate.

---

### Layer 6

Memory wiping

Private key lifetime is minimized.

---

### Layer 7

Behavior monitoring

AI identifies suspicious activity before signing.

---

# Threat Model

HORCRUX is designed to mitigate

✔ Device theft

✔ Password guessing

✔ Malware stealing shard files

✔ USB duplication

✔ Guardian compromise

✔ Insider attacks

✔ Offline brute force attacks

✔ Key leakage during storage

✔ Memory persistence after signing

✔ Unauthorized recovery attempts

---

HORCRUX does **not** currently protect against

- Compromised operating systems

- Hardware side-channel attacks

- Physical coercion of all guardians

- Nation-state hardware implants

- Quantum attacks

These remain future research areas.

---

# Performance Goals

| Operation | Expected Time |
|------------|--------------|
| Split Key | <100 ms |
| Encrypt Shard | <50 ms |
| Decrypt Shard | <50 ms |
| Reconstruction | <100 ms |
| Mode A Signing | <500 ms |
| MPC Signing | Network dependent |

---

# Design Philosophy

HORCRUX follows several guiding principles.

## Self Custody

Users retain complete ownership of cryptographic material.

---

## Offline First

Internet connectivity should never be a security requirement.

---

## Defense in Depth

Every protection layer assumes another layer may eventually fail.

---

## Open Cryptography

Never invent cryptographic algorithms.

Use audited, peer-reviewed libraries.

---

## Memory Safety

Rust reduces classes of vulnerabilities common in systems programming.

---

# Roadmap

## Current

- CLI
- SSS
- AES Encryption
- USB Storage
- Mode A Signing
- Mode B FROST Signing
- Logging
- AI Detection

---

## Planned

- Tauri Desktop GUI
- QR-based Air Gap MPC
- Proactive Secret Refresh
- Multi-chain Support
- Hardware Wallet Integration
- Hardware Security Module Support
- Secure Firmware Verification

---

# Contributing

Contributions are welcome.

Recommended workflow

```
Fork Repository

↓

Create Feature Branch

↓

Write Tests

↓

Submit Pull Request
```

Please ensure

- Code is formatted

- Tests pass

- Documentation updated

- Security considerations explained

---

# References

The implementation and design are based on established cryptographic research, including:

- Adi Shamir — *How to Share a Secret* (1979)
- Komlo & Goldberg — FROST: Round-Optimal Schnorr Threshold Signatures
- Zcash Foundation — frost-ed25519 (NCC-audited, v3.0.0)
- Park et al. — Cryptocurrency Wallet Security Survey
- Li et al. — Distributed HSM-Based Key Management

For detailed discussion, see the project report included with this repository.

---

# License

This project is released under the MIT License.

See the LICENSE file for details.

---

## Acknowledgements

HORCRUX was developed as a Final Year B.E. Project in Artificial Intelligence & Machine Learning, combining modern cryptography, secure systems programming, and blockchain infrastructure into a unified offline threshold key management platform.

The project demonstrates that enterprise-inspired threshold custody can be achieved using commodity hardware, open-source cryptographic libraries, and memory-safe software without sacrificing user sovereignty.