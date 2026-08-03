use crate::error::Error;
use crate::{ARGON2_M_COST, ARGON2_P_COST, ARGON2_T_COST, NONCE_LEN, SALT_LEN};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

const KEY_LEN: usize = 32;

/// Generate a fresh random Argon2id salt.
pub fn random_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a fresh random 96-bit nonce for AES-256-GCM.
pub fn random_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Derive a 256-bit AES key from a guardian password using Argon2id.
///
/// Parameters follow the OWASP "interactive login" recommendation
/// (m=19 MiB, t=2, p=1).
pub fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<Zeroizing<[u8; KEY_LEN]>, Error> {
    let params = Params::new(
        ARGON2_M_COST,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(KEY_LEN),
    )
    .map_err(|e| Error::Kdf(format!("invalid Argon2 parameters: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| Error::Kdf(format!("Argon2id failed: {e}")))?;
    Ok(key)
}

/// Encrypt `plaintext` with AES-256-GCM, authenticating `aad` alongside the
/// ciphertext. Returns the ciphertext with the 16-byte authentication tag
/// appended.
pub fn seal(
    plaintext: &[u8],
    password: &str,
    salt: &[u8; SALT_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
) -> Result<Vec<u8>, Error> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key[..]));
    cipher
        .encrypt(Nonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|e| Error::Aead(format!("encryption failed: {e}")))
}

/// Decrypt a ciphertext (with appended tag) previously produced by [`seal`].
///
/// Fails on a wrong password or any tampering, because AES-GCM rejects the
/// authentication tag.
pub fn open(
    ciphertext: &[u8],
    password: &str,
    salt: &[u8; SALT_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key[..]));
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map_err(|_| Error::Aead("wrong password or tampered shard".to_string()))?;
    Ok(Zeroizing::new(plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let password = "correct horse battery staple";
        let salt = random_salt();
        let nonce = random_nonce();
        let aad = [1u8, 2, 3];
        let plaintext = [7u8; 32];

        let ct = seal(&plaintext, password, &salt, &nonce, &aad).expect("seal");
        assert_eq!(ct.len(), 32 + 16);
        let opened = open(&ct, password, &salt, &nonce, &aad).expect("open");
        assert_eq!(&opened[..], &plaintext[..]);
    }

    #[test]
    fn wrong_password_rejected() {
        let salt = random_salt();
        let nonce = random_nonce();
        let ct = seal(b"secret material", "right", &salt, &nonce, b"").expect("seal");
        assert!(open(&ct, "wrong", &salt, &nonce, b"").is_err());
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let salt = random_salt();
        let nonce = random_nonce();
        let mut ct = seal(b"secret material", "pw", &salt, &nonce, b"").expect("seal");
        ct[0] ^= 0x01;
        assert!(open(&ct, "pw", &salt, &nonce, b"").is_err());
    }

    #[test]
    fn tampered_nonce_rejected() {
        let salt = random_salt();
        let nonce = random_nonce();
        let ct = seal(b"secret material", "pw", &salt, &nonce, b"").expect("seal");
        let mut bad_nonce = nonce;
        bad_nonce[0] ^= 0x01;
        assert!(open(&ct, "pw", &salt, &bad_nonce, b"").is_err());
    }

    #[test]
    fn tampered_aad_rejected() {
        let salt = random_salt();
        let nonce = random_nonce();
        let ct = seal(b"secret material", "pw", &salt, &nonce, b"aad").expect("seal");
        assert!(open(&ct, "pw", &salt, &nonce, b"tampered").is_err());
    }

    #[test]
    fn different_passwords_decrypt_independently() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let salt_a = random_salt();
        let salt_b = random_salt();
        let nonce_a = random_nonce();
        let nonce_b = random_nonce();

        let ct_a = seal(&a, "alpha", &salt_a, &nonce_a, b"").expect("seal a");
        let ct_b = seal(&b, "beta", &salt_b, &nonce_b, b"").expect("seal b");

        assert_eq!(&open(&ct_a, "alpha", &salt_a, &nonce_a, b"").unwrap()[..], &a[..]);
        assert_eq!(&open(&ct_b, "beta", &salt_b, &nonce_b, b"").unwrap()[..], &b[..]);
        assert!(open(&ct_a, "beta", &salt_a, &nonce_a, b"").is_err());
        assert!(open(&ct_b, "alpha", &salt_b, &nonce_b, b"").is_err());
    }

    #[test]
    fn derived_key_is_32_bytes() {
        let salt = random_salt();
        let key = derive_key("pw", &salt).expect("derive");
        assert_eq!(key.len(), 32);
    }
}
