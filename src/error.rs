use std::path::PathBuf;

/// Errors that can occur while splitting, encrypting, decrypting, or
/// reconstructing shards.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The private key bytes do not form a valid secp256k1 scalar.
    #[error("invalid private key: {0}")]
    InvalidKey(String),

    /// Invalid threshold/share-count combination.
    #[error("invalid secret-sharing parameters: {0}")]
    InvalidParams(String),

    /// Not enough valid shards to reconstruct the key.
    #[error("need {0} shards but only {1} were provided")]
    NotEnoughShares(usize, usize),

    /// Decryption failed; typically a wrong password or tampered file.
    #[error("failed to decrypt shard {id}: {reason}")]
    Decrypt { id: u8, reason: String },

    /// The shard file is not a valid horcrux shard.
    #[error("invalid shard file: {0}")]
    InvalidShardFile(String),

    /// Two shards from different splits were mixed together.
    #[error("shard {path:?} has different split parameters (t={t}, n={n})")]
    SplitMismatch { path: PathBuf, t: u8, n: u8 },

    /// The underlying secret-sharing library failed.
    #[error("secret-sharing error: {0}")]
    Vsss(String),

    /// I/O error while reading or writing a shard file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
