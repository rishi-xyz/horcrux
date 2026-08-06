# HORCRUX

> **Offline Threshold Key Management System for Blockchain Private Keys**

HORCRUX is a Rust-based offline threshold key management system that eliminates the single point of failure associated with traditional cryptocurrency wallets. Instead of storing an entire private key on one device, HORCRUX distributes cryptographic control across multiple trusted guardians using **Shamir's Secret Sharing (SSS)** and supports **threshold ECDSA Multi-Party Computation (MPC)** for secure transaction signing.

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

True Threshold MPC.

The private key never exists on any machine.

Guardians collaboratively generate a valid ECDSA signature.

---

## Ethereum Compatible

Uses

- secp256k1
- Standard ECDSA
- Compatible with Ethereum
- Compatible with every EVM chain

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
- Preserve compatibility with Ethereum.
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

Mode B uses threshold ECDSA.

Instead of reconstructing the private key:

```
Guardian A

+

Guardian B

+

Guardian C

↓

Partial Signatures

↓

Coordinator

↓

Valid ECDSA Signature
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

Threshold Protocol

↓

ECDSA Signature

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
| Elliptic Curve | secp256k1 |
| EVM Integration | alloy |
| MPC | CGGMP21 |
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
├── sign
├── mpc-sign
├── verify
├── inspect
├── logs
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
    --output ./usb
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
sign an EVM transaction, wipe the key, and output the signed transaction.

```bash
horcrux sign \
    --shard ./usb/shard-1.hx --password guardian-1 \
    --shard ./usb/shard-2.hx --password guardian-2 \
    --to 0xRecipientAddress \
    --value 0.001 \
    --nonce 0 \
    --gas 21000 \
    --max-fee-per-gas 20 \
    --max-priority-fee-per-gas 1
```

EIP-1559 fees are the default. Legacy transactions use `--gas-price` instead
(conflicts with the EIP-1559 flags):

```bash
horcrux sign \
    --shard ./usb/shard-1.hx --password guardian-1 \
    --shard ./usb/shard-2.hx --password guardian-2 \
    --to 0xRecipientAddress --value 0.001 \
    --nonce 0 --gas 21000 --gas-price 5
```

- Required: `--to`, `--value`, `--nonce`, `--gas`, and one fee model.
- Optional: `--calldata` (hex), `--chain-id` (default Sepolia `11155111`),
  `--from` (skips nonce check), and `--broadcast`.

Broadcast to an EVM RPC (Sepolia by default), fetching any missing fields and
waiting for the receipt:

```bash
HORCRUX_RPC_URL=https://rpc.sepolia.org \
horcrux sign \
    --shard ./usb/shard-1.hx --password guardian-1 \
    --shard ./usb/shard-2.hx --password guardian-2 \
    --to 0xRecipientAddress --value 0.001 \
    --broadcast
```

Output

```
Signed Transaction

↓

Raw Hex

↓

Broadcast (opt-in)
```

---

## MPC Signing

Mode B

```bash
horcrux mpc-sign
```

Coordinator

```
Receive Partial Signatures

↓

Combine

↓

Generate Valid Signature
```

Guardians

```
Decrypt Local Share

↓

Participate In MPC

↓

Never Reveal Key
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

## Logs

View access logs.

```bash
horcrux logs
```

Displays

- timestamp

- guardian

- success

- failure

- anomaly score

---

# Configuration

HORCRUX stores runtime configuration separately from encrypted shard data.

Example

```toml
threshold = 2

shares = 3

curve = "secp256k1"

mode = "offline"

rpc = "https://rpc.sepolia.org"

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

## secp256k1

Used for

- Ethereum

- Polygon

- BNB Chain

- Avalanche C-Chain

- Base

- Arbitrum

- Optimism

- Every EVM compatible chain

---

## CGGMP21 Threshold ECDSA

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

ECDSA Signature
```

Advantages

✔ Key never reconstructed

✔ Standard ECDSA output

✔ Compatible with existing wallets

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
- Gennaro & Goldfeder — Threshold ECDSA (GG18)
- Canetti et al. — CGGMP21 Threshold ECDSA
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