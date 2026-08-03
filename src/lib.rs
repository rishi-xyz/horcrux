//! horcrux — split a secp256k1 private key via Shamir's Secret Sharing, encrypt
//! the shares with per-shard guardian passwords (Argon2id + AES-256-GCM), and
//! reconstruct the key from any threshold subset of shard files.

pub mod crypto;
pub mod error;
pub mod shard;
pub mod sss;

pub const SHARD_MAGIC: &[u8; 3] = b"HX1";
pub const SHARD_VERSION: u8 = 1;

/// KDF memory cost in KiB (Argon2id, OWASP interactive-class defaults).
pub const ARGON2_M_COST: u32 = 19 * 1024;
/// KDF time cost.
pub const ARGON2_T_COST: u32 = 2;
/// KDF parallelism.
pub const ARGON2_P_COST: u32 = 1;

/// Length of the per-shard Argon2id salt, in bytes.
pub const SALT_LEN: usize = 16;
/// Length of the AES-256-GCM nonce, in bytes.
pub const NONCE_LEN: usize = 12;
/// Length of the AES-256-GCM authentication tag, in bytes.
pub const TAG_LEN: usize = 16;
/// Length of a share value, in bytes (a secp256k1 scalar).
pub const SHARE_VALUE_LEN: usize = 32;
