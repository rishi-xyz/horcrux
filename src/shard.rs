use crate::crypto;
use crate::error::Error;
use crate::{NONCE_LEN, SALT_LEN, SHARD_MAGIC, SHARD_VERSION, SHARE_VALUE_LEN, TAG_LEN};
use std::fs;
use std::path::Path;
use zeroize::Zeroizing;

/// Total on-disk size of a shard file, in bytes.
pub const SHARD_LEN: usize = SHARD_MAGIC.len()
    + 1 // version
    + 1 // threshold
    + 1 // share count
    + 1 // share id
    + SALT_LEN
    + NONCE_LEN
    + SHARE_VALUE_LEN
    + TAG_LEN;

/// An encrypted, on-disk shard.
///
/// Layout (83 bytes): magic | version | threshold | share_count | id |
/// Argon2id salt | AES-GCM nonce | sealed (ciphertext || tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    /// Number of shares required to reconstruct the key.
    pub threshold: u8,
    /// Total number of shares this shard was split into.
    pub share_count: u8,
    /// This share's x-coordinate (identifier).
    pub id: u8,
    /// Argon2id salt.
    pub salt: [u8; SALT_LEN],
    /// AES-256-GCM nonce.
    pub nonce: [u8; NONCE_LEN],
    /// Encrypted share value with appended authentication tag.
    pub sealed: [u8; SHARE_VALUE_LEN + TAG_LEN],
}

/// Associated data bound to the ciphertext: a shard from a different split
/// (different threshold/share-count) or a different share id will not decrypt.
pub fn aad(threshold: u8, share_count: u8, id: u8) -> [u8; 3] {
    [threshold, share_count, id]
}

impl Shard {
    /// Build a shard from already-sealed bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        threshold: u8,
        share_count: u8,
        id: u8,
        salt: [u8; SALT_LEN],
        nonce: [u8; NONCE_LEN],
        sealed: [u8; SHARE_VALUE_LEN + TAG_LEN],
    ) -> Self {
        Self {
            threshold,
            share_count,
            id,
            salt,
            nonce,
            sealed,
        }
    }

    /// Serialize to the fixed binary layout.
    pub fn to_bytes(&self) -> [u8; SHARD_LEN] {
        let mut out = [0u8; SHARD_LEN];
        out[..3].copy_from_slice(SHARD_MAGIC);
        out[3] = SHARD_VERSION;
        out[4] = self.threshold;
        out[5] = self.share_count;
        out[6] = self.id;
        out[7..7 + SALT_LEN].copy_from_slice(&self.salt);
        out[7 + SALT_LEN..7 + SALT_LEN + NONCE_LEN].copy_from_slice(&self.nonce);
        out[7 + SALT_LEN + NONCE_LEN..].copy_from_slice(&self.sealed);
        out
    }

    /// Deserialize from the fixed binary layout, validating magic/version/len.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != SHARD_LEN {
            return Err(Error::InvalidShardFile(format!(
                "expected {SHARD_LEN} bytes, got {}",
                bytes.len()
            )));
        }
        if &bytes[..3] != SHARD_MAGIC {
            return Err(Error::InvalidShardFile(
                "bad magic bytes; not a horcrux shard".to_string(),
            ));
        }
        if bytes[3] != SHARD_VERSION {
            return Err(Error::InvalidShardFile(format!(
                "unsupported version {}",
                bytes[3]
            )));
        }

        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        let mut sealed = [0u8; SHARE_VALUE_LEN + TAG_LEN];
        salt.copy_from_slice(&bytes[7..7 + SALT_LEN]);
        nonce.copy_from_slice(&bytes[7 + SALT_LEN..7 + SALT_LEN + NONCE_LEN]);
        sealed.copy_from_slice(&bytes[7 + SALT_LEN + NONCE_LEN..]);

        Ok(Self {
            threshold: bytes[4],
            share_count: bytes[5],
            id: bytes[6],
            salt,
            nonce,
            sealed,
        })
    }

    /// Write the shard to `path`.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        fs::write(path, self.to_bytes()).map_err(Error::Io)
    }

    /// Read a shard from `path`.
    pub fn read(path: &Path) -> Result<Self, Error> {
        let bytes = fs::read(path).map_err(Error::Io)?;
        Self::from_bytes(&bytes)
    }

    /// Decrypt the share value with a guardian password.
    ///
    /// The password is bound to this exact shard via the AAD; a wrong
    /// password (or tampered file) is rejected by the GCM tag check.
    pub fn decrypt(&self, password: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
        let aad = aad(self.threshold, self.share_count, self.id);
        crypto::open(&self.sealed, password, &self.salt, &self.nonce, &aad).map_err(|e| {
            Error::Decrypt {
                id: self.id,
                reason: e.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    fn sample() -> Shard {
        Shard::new(
            2,
            3,
            1,
            [0x11; SALT_LEN],
            [0x22; NONCE_LEN],
            [0x33; SHARE_VALUE_LEN + TAG_LEN],
        )
    }

    #[test]
    fn layout_round_trip() {
        let s = sample();
        let bytes = s.to_bytes();
        assert_eq!(bytes.len(), SHARD_LEN);
        assert_eq!(Shard::from_bytes(&bytes).expect("parse"), s);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().to_bytes();
        bytes[0] = b'X';
        assert!(matches!(
            Shard::from_bytes(&bytes),
            Err(Error::InvalidShardFile(_))
        ));
    }

    #[test]
    fn rejects_bad_version() {
        let mut bytes = sample().to_bytes();
        bytes[3] = 99;
        assert!(matches!(
            Shard::from_bytes(&bytes),
            Err(Error::InvalidShardFile(_))
        ));
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(matches!(
            Shard::from_bytes(&[0u8; SHARD_LEN - 1]),
            Err(Error::InvalidShardFile(_))
        ));
    }

    #[test]
    fn write_read_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("shard-1.hx");
        sample().write(&path).expect("write");
        let read = Shard::read(&path).expect("read");
        assert_eq!(read, sample());
    }

    #[test]
    fn decrypt_with_right_password() {
        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let aad = aad(2, 3, 1);
        let sealed = crypto::seal(&[7u8; 32], "pw", &salt, &nonce, &aad).expect("seal");
        let mut sealed_arr = [0u8; SHARE_VALUE_LEN + TAG_LEN];
        sealed_arr.copy_from_slice(&sealed);
        let shard = Shard::new(2, 3, 1, salt, nonce, sealed_arr);

        let value = shard.decrypt("pw").expect("decrypt");
        assert_eq!(&value[..], &[7u8; 32]);
    }

    #[test]
    fn wrong_password_maps_to_decrypt_error() {
        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let aad = aad(2, 3, 1);
        let sealed = crypto::seal(&[7u8; 32], "pw", &salt, &nonce, &aad).expect("seal");
        let mut sealed_arr = [0u8; SHARE_VALUE_LEN + TAG_LEN];
        sealed_arr.copy_from_slice(&sealed);
        let shard = Shard::new(2, 3, 1, salt, nonce, sealed_arr);

        assert!(matches!(
            shard.decrypt("nope"),
            Err(Error::Decrypt { id: 1, .. })
        ));
    }
}
