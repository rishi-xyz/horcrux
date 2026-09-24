//! Mode B for Bitcoin: FROST threshold signatures over secp256k1-Taproot
//! (BIP340/BIP341), using `frost-secp256k1-tr` (Zcash Foundation).
//!
//! Architecture mirrors [`crate::mpc`] (the Solana/Ed25519 Mode B): the Mode A
//! seed is dealer-split into t-of-n encrypted key shares, and signing runs the
//! two-round FROST protocol with one in-process participant per share file.
//! The full signing key is never reconstructed on any machine.
//!
//! The one Bitcoin-specific wrinkle is the BIP341 Taproot tweak. The dealer
//! split and the on-disk key shares hold the *untweaked* (`internal`) FROST
//! group key, exactly like [`crate::bitcoin::taproot_keys`] holds an untweaked
//! internal key for Mode A. The tweak (`Q = P + hashTapTweak(P)*G`, no script
//! tree) is applied at sign time via `frost_secp256k1_tr`'s own
//! `round2::sign_with_tweak` / `aggregate_with_tweak` helpers, and at
//! address-derivation time via its `keys::Tweak` trait — the same audited
//! crate logic in both cases, never hand-rolled. Because both Mode A and Mode
//! B tweak the same seed the same way, the Bitcoin Mode B group address
//! equals the Bitcoin Mode A address for the same seed (asserted by
//! [`tests::frost_group_address_matches_mode_a_address`]).

use crate::crypto;
use crate::error::Error;
use crate::{NONCE_LEN, SALT_LEN, TAG_LEN};
use frost_secp256k1_tr::keys::Tweak;
use frost_secp256k1_tr::{self as frost};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// File magic distinguishing Bitcoin FROST share files from Solana FROST
/// shares (`HX2`) and SSS shards (`HX1`), so files from different
/// protocols/curves can never be cross-used.
pub const BTC_FROST_MAGIC: &[u8; 3] = b"HX4";
/// Share file format version.
pub const BTC_FROST_VERSION: u8 = 1;
/// File name of the (non-secret) group public key package written by
/// [`btc_mpc_split`]. Distinct from Solana's `group.pub` so the same
/// `--out-dir` can never mix the two.
pub const BTC_GROUP_PUB_FILENAME: &str = "group-btc.pub";

/// Fixed header length of a Bitcoin FROST share file (excluding the sealed
/// payload). Identical layout to [`crate::mpc::FrostShare`], just a different
/// magic.
pub const BTC_FROST_HEADER_LEN: usize = BTC_FROST_MAGIC.len()
    + 1 // version
    + 1 // min signers (threshold)
    + 1 // max signers (share count)
    + 1 // participant id
    + SALT_LEN
    + NONCE_LEN
    + 2; // sealed payload length, little-endian u16

/// An encrypted Bitcoin FROST key share on disk.
///
/// Layout: magic | version | min_signers | max_signers | id | salt | nonce |
/// sealed_len(u16 LE) | sealed (`KeyPackage` serialization || AES-GCM tag).
/// The key package held here is the *untweaked* internal key; the BIP341
/// tweak is applied at sign time, never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtcFrostShare {
    /// Shares required to sign (threshold).
    pub min_signers: u8,
    /// Total number of shares in the group.
    pub max_signers: u8,
    /// Participant identifier (1-based, matching the default FROST split).
    pub id: u8,
    /// Argon2id salt.
    pub salt: [u8; SALT_LEN],
    /// AES-256-GCM nonce.
    pub nonce: [u8; NONCE_LEN],
    /// Encrypted [`frost::keys::KeyPackage`] with appended authentication tag.
    pub sealed: Vec<u8>,
}

impl BtcFrostShare {
    /// Write the share to `path`.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        fs::write(path, self.to_bytes()).map_err(Error::Io)
    }

    /// Read a share from `path`.
    pub fn read(path: &Path) -> Result<Self, Error> {
        let bytes = fs::read(path).map_err(Error::Io)?;
        Self::from_bytes(&bytes)
    }

    /// Serialize to the binary layout.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BTC_FROST_HEADER_LEN + self.sealed.len());
        out.extend_from_slice(BTC_FROST_MAGIC);
        out.push(BTC_FROST_VERSION);
        out.push(self.min_signers);
        out.push(self.max_signers);
        out.push(self.id);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&(self.sealed.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.sealed);
        out
    }

    /// Deserialize from the binary layout, validating magic/version/length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < BTC_FROST_HEADER_LEN {
            return Err(Error::InvalidShardFile(format!(
                "expected at least {BTC_FROST_HEADER_LEN} bytes, got {}",
                bytes.len()
            )));
        }
        if &bytes[..3] != BTC_FROST_MAGIC {
            return Err(Error::InvalidShardFile(
                "bad magic bytes; not a horcrux Bitcoin FROST share".to_string(),
            ));
        }
        if bytes[3] != BTC_FROST_VERSION {
            return Err(Error::InvalidShardFile(format!(
                "unsupported version {}",
                bytes[3]
            )));
        }

        let sealed_len = u16::from_le_bytes([
            bytes[BTC_FROST_HEADER_LEN - 2],
            bytes[BTC_FROST_HEADER_LEN - 1],
        ]) as usize;
        if sealed_len < TAG_LEN || bytes.len() != BTC_FROST_HEADER_LEN + sealed_len {
            return Err(Error::InvalidShardFile(format!(
                "sealed payload is {sealed_len} bytes but file is {} bytes",
                bytes.len()
            )));
        }

        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        salt.copy_from_slice(&bytes[7..7 + SALT_LEN]);
        nonce.copy_from_slice(&bytes[7 + SALT_LEN..7 + SALT_LEN + NONCE_LEN]);

        Ok(Self {
            min_signers: bytes[4],
            max_signers: bytes[5],
            id: bytes[6],
            salt,
            nonce,
            sealed: bytes[BTC_FROST_HEADER_LEN..].to_vec(),
        })
    }

    /// Decrypt and deserialize the participant's untweaked
    /// [`frost::keys::KeyPackage`].
    pub fn decrypt(&self, password: &str) -> Result<frost::keys::KeyPackage, Error> {
        let aad = [self.min_signers, self.max_signers, self.id];
        let payload =
            crypto::open(&self.sealed, password, &self.salt, &self.nonce, &aad).map_err(|e| {
                Error::Decrypt {
                    id: self.id,
                    reason: e.to_string(),
                }
            })?;
        frost::keys::KeyPackage::deserialize(&payload)
            .map_err(|e| Error::Mpc(format!("invalid share payload: {e}")))
    }
}

/// A BIP340 Schnorr signature produced by aggregating FROST signature shares
/// under the BIP341 Taproot tweak, plus the tweaked x-only output key that
/// verifies it (the same 32 bytes embedded in the P2TR address).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtcMpcSignature {
    /// The 64-byte compact BIP340 signature (`r || s`, x-only `R`).
    pub signature: [u8; 64],
    /// The 32-byte x-only tweaked group output key (the Mode A address key).
    pub output_xonly: [u8; 32],
}

/// Derive the FROST signing key for a Mode A secp256k1 seed. Unlike Ed25519
/// (which needs a curve conversion), the seed *is* already a secp256k1
/// scalar, so this is a direct load.
pub fn frost_signing_key(seed: &k256::SecretKey) -> Result<frost::SigningKey, Error> {
    let scalar = *seed.to_nonzero_scalar();
    frost::SigningKey::from_scalar(scalar)
        .map_err(|e| Error::Mpc(format!("invalid signing scalar: {e}")))
}

/// Dealer-split `key` into `share_count` encrypted Bitcoin FROST key shares
/// written to `out_dir`, requiring `threshold` of them to sign. Mirrors
/// [`crate::mpc::mpc_split`]; shares hold the *untweaked* internal key.
pub fn btc_mpc_split(
    key: &k256::SecretKey,
    threshold: u8,
    share_count: u8,
    out_dir: &Path,
    passwords: &[String],
) -> Result<(Vec<PathBuf>, PathBuf), Error> {
    if threshold < 2 {
        return Err(Error::InvalidParams(
            "FROST requires a threshold of at least 2".to_string(),
        ));
    }
    if threshold > share_count {
        return Err(Error::InvalidParams(format!(
            "threshold ({threshold}) must be at most share count ({share_count})"
        )));
    }
    if passwords.len() != share_count as usize {
        return Err(Error::InvalidParams(format!(
            "expected {share_count} passwords, got {}",
            passwords.len()
        )));
    }

    let signing_key = frost_signing_key(key)?;

    let mut rng = rand::rngs::OsRng;
    let (secret_shares, pubkey_package) = frost::keys::split(
        &signing_key,
        share_count as u16,
        threshold as u16,
        frost::keys::IdentifierList::Default,
        &mut rng,
    )
    .map_err(|e| Error::Mpc(e.to_string()))?;

    fs::create_dir_all(out_dir)?;

    let mut paths = Vec::with_capacity(secret_shares.len());
    for (i, (identifier, secret_share)) in secret_shares.iter().enumerate() {
        let id = share_id(identifier);
        let key_package = frost::keys::KeyPackage::try_from(secret_share.clone())
            .map_err(|e| Error::Mpc(e.to_string()))?;
        let payload = key_package
            .serialize()
            .map_err(|e| Error::Mpc(e.to_string()))?;
        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let aad = [threshold, share_count, id];
        let sealed = crypto::seal(&payload, &passwords[i], &salt, &nonce, &aad)?;

        let share = BtcFrostShare {
            min_signers: threshold,
            max_signers: share_count,
            id,
            salt,
            nonce,
            sealed,
        };
        let path = out_dir.join(format!("btc-mpc-{id}.hx"));
        share.write(&path)?;
        paths.push(path);
    }

    let group_path = out_dir.join(BTC_GROUP_PUB_FILENAME);
    let group_bytes = pubkey_package
        .serialize()
        .map_err(|e| Error::Mpc(e.to_string()))?;
    fs::write(&group_path, group_bytes).map_err(Error::Io)?;

    Ok((paths, group_path))
}

/// Read the participant ids from Bitcoin FROST share files without decrypting
/// anything.
pub fn shard_ids(paths: &[PathBuf]) -> Result<Vec<u8>, Error> {
    paths
        .iter()
        .map(|p| Ok(BtcFrostShare::read(p)?.id))
        .collect()
}

/// Deserialize the (untweaked) group public key package.
fn deserialize_public_key_package(bytes: &[u8]) -> Result<frost::keys::PublicKeyPackage, Error> {
    frost::keys::PublicKeyPackage::deserialize(bytes)
        .map_err(|e| Error::Mpc(format!("invalid group public package: {e}")))
}

/// Read the tweaked x-only group output key (the Mode A P2TR address key)
/// from a public key package file, applying the same BIP341 "no script tree"
/// tweak used by [`crate::bitcoin::taproot_keys`].
pub fn group_output_xonly(group_pub_path: &Path) -> Result<[u8; 32], Error> {
    let bytes = fs::read(group_pub_path).map_err(Error::Io)?;
    let group_pub = deserialize_public_key_package(&bytes)?;
    Ok(tweaked_output_xonly(&group_pub))
}

/// Extract the tweaked x-only output key from an untweaked public key
/// package.
fn tweaked_output_xonly(group_pub: &frost::keys::PublicKeyPackage) -> [u8; 32] {
    use k256::elliptic_curve::sec1::ToEncodedPoint;

    let tweaked = group_pub.clone().tweak::<&[u8]>(None);
    let affine = tweaked.verifying_key().to_element().to_affine();
    let encoded = affine.to_encoded_point(false);
    let mut out = [0u8; 32];
    out.copy_from_slice(encoded.x().expect("affine point has an x coordinate"));
    out
}

/// Run the two-round FROST protocol over `sighash` (the BIP341 key-path
/// sighash to sign) with one in-process participant per share file, applying
/// the BIP341 tweak at both the signing and aggregation steps, and aggregate
/// their signature shares into a single verified BIP340 signature.
///
/// `on_participant` is invoked with each share's id and whether its
/// decryption succeeded (used by the audit layer).
pub fn btc_mpc_sign(
    share_paths: &[PathBuf],
    passwords: &[String],
    group_pub_path: &Path,
    sighash: &[u8; 32],
    mut on_participant: impl FnMut(u8, bool),
) -> Result<BtcMpcSignature, Error> {
    if passwords.len() != share_paths.len() {
        return Err(Error::InvalidParams(format!(
            "expected {} passwords, got {}",
            share_paths.len(),
            passwords.len()
        )));
    }

    let group_bytes = fs::read(group_pub_path).map_err(Error::Io)?;
    let group_pub = deserialize_public_key_package(&group_bytes)?;
    let group_vk = *group_pub.verifying_key();

    let mut rng = rand::rngs::OsRng;
    let mut participants: Vec<(frost::keys::KeyPackage, frost::round1::SigningNonces)> = Vec::new();
    let mut commitments: BTreeMap<frost::Identifier, frost::round1::SigningCommitments> =
        BTreeMap::new();

    for (path, password) in share_paths.iter().zip(passwords) {
        let share = BtcFrostShare::read(path)?;
        let key_package = match share.decrypt(password) {
            Ok(kp) => {
                on_participant(share.id, true);
                kp
            }
            Err(e) => {
                on_participant(share.id, false);
                return Err(e);
            }
        };
        if key_package.verifying_key() != &group_vk {
            return Err(Error::MpcGroupMismatch { path: path.clone() });
        }

        let (nonces, commitment) = frost::round1::commit(key_package.signing_share(), &mut rng);
        commitments.insert(*key_package.identifier(), commitment);
        participants.push((key_package, nonces));
    }

    let min = group_pub.min_signers().unwrap_or_default() as usize;
    if participants.len() < min {
        return Err(Error::NotEnoughShares(min, participants.len()));
    }

    let signing_package = frost::SigningPackage::new(commitments, sighash);
    let mut signature_shares: BTreeMap<frost::Identifier, frost::round2::SignatureShare> =
        BTreeMap::new();
    for (key_package, nonces) in &participants {
        let share =
            frost::round2::sign_with_tweak(&signing_package, nonces, key_package, None::<&[u8]>)
                .map_err(|e| Error::Mpc(format!("failed to produce signature share: {e}")))?;
        signature_shares.insert(*key_package.identifier(), share);
    }

    let signature = frost::aggregate_with_tweak(
        &signing_package,
        &signature_shares,
        &group_pub,
        None::<&[u8]>,
    )
    .map_err(|e| Error::Mpc(format!("failed to aggregate signature: {e}")))?;

    let output_xonly = tweaked_output_xonly(&group_pub);
    let tweaked_group = group_pub.clone().tweak::<&[u8]>(None);
    if tweaked_group
        .verifying_key()
        .verify(sighash, &signature)
        .is_err()
    {
        return Err(Error::Mpc(
            "aggregated signature failed FROST verification".to_string(),
        ));
    }

    let sig_bytes: [u8; 64] = signature
        .serialize()
        .map_err(|e| Error::Mpc(e.to_string()))?
        .try_into()
        .map_err(|_| Error::Mpc("signature is not 64 bytes".to_string()))?;

    Ok(BtcMpcSignature {
        signature: sig_bytes,
        output_xonly,
    })
}

/// Like [`btc_mpc_sign`], recording each participant's decryption outcome and
/// a final `signed` entry in the access log.
pub fn btc_mpc_sign_with_audit(
    share_paths: &[PathBuf],
    passwords: &[String],
    group_pub_path: &Path,
    sighash: &[u8; 32],
    log: &crate::audit::AccessLog,
    attempt: u64,
) -> Result<BtcMpcSignature, Error> {
    let ts = crate::audit::now_ms();
    let result = btc_mpc_sign(share_paths, passwords, group_pub_path, sighash, |id, ok| {
        let entry = if ok {
            crate::audit::Entry::ok(ts, attempt, id)
        } else {
            crate::audit::Entry::fail(ts, attempt, id)
        };
        let _ = log.append(&entry);
    });
    if result.is_ok() {
        let _ = log.append(&crate::audit::Entry::signed(
            crate::audit::now_ms(),
            attempt,
        ));
    }
    result
}

/// The participant id as a `u8`. The default FROST split assigns the nonzero
/// identifiers `1..=n`; unlike Ed25519's little-endian scalar encoding,
/// secp256k1's scalar serialization is big-endian (SEC1), so the id byte is
/// the *last* byte, not the first.
fn share_id(identifier: &frost::Identifier) -> u8 {
    *identifier
        .serialize()
        .last()
        .expect("scalar serialization is non-empty")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin as btc;

    fn key() -> k256::SecretKey {
        k256::SecretKey::random(&mut rand::rngs::OsRng)
    }

    fn passwords(n: usize, seed: &str) -> Vec<String> {
        (0..n).map(|i| format!("{seed}-{i}")).collect()
    }

    fn seed_of(key: &k256::SecretKey) -> [u8; 32] {
        key.to_bytes().into()
    }

    #[test]
    fn frost_group_address_matches_mode_a_address() {
        let key = key();
        let seed = seed_of(&key);
        let pws = passwords(3, "guardian");
        let dir = tempfile::tempdir().expect("tempdir");

        let (_paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        let mpc_xonly = group_output_xonly(&group).expect("group xonly");

        let mode_a_keys = btc::taproot_keys(&seed).expect("mode a keys");
        assert_eq!(
            mpc_xonly, mode_a_keys.output_xonly,
            "Bitcoin Mode B group address must equal the Mode A address for the same seed"
        );
    }

    #[test]
    fn split_2_of_3_any_pair_signs_and_verifies() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");
        let (paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        assert_eq!(paths.len(), 3);
        let sighash = [0x24u8; 32];

        for combo in [&[0usize, 1], &[0usize, 2], &[1usize, 2]] {
            let chosen: Vec<_> = combo.iter().map(|&i| paths[i].clone()).collect();
            let chosen_pws: Vec<_> = combo.iter().map(|&i| pws[i].clone()).collect();
            let sig =
                btc_mpc_sign(&chosen, &chosen_pws, &group, &sighash, |_, _| {}).expect("sign");
            let vk = k256::schnorr::VerifyingKey::from_bytes(&sig.output_xonly).expect("vk");
            let signature =
                k256::schnorr::Signature::try_from(&sig.signature[..]).expect("signature");
            use k256::schnorr::signature::hazmat::PrehashVerifier;
            assert!(
                vk.verify_prehash(&sighash, &signature).is_ok(),
                "aggregated signature must verify under plain BIP340"
            );
        }
    }

    #[test]
    fn single_share_is_not_enough() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");
        let (paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        assert!(matches!(
            btc_mpc_sign(&paths[..1], &pws[..1], &group, &[0u8; 32], |_, _| {}),
            Err(Error::NotEnoughShares(..))
        ));
    }

    #[test]
    fn wrong_password_fails_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");
        let (paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        let bad = vec!["nope".to_string(), "guardian-1".to_string()];
        assert!(matches!(
            btc_mpc_sign(&paths[..2], &bad, &group, &[0u8; 32], |_, _| {}),
            Err(Error::Decrypt { .. })
        ));
    }

    #[test]
    fn shares_from_different_groups_are_rejected() {
        let dir_a = tempfile::tempdir().expect("tempdir a");
        let dir_b = tempfile::tempdir().expect("tempdir b");
        let pws = passwords(3, "guardian");
        let (paths_a, group_a) = btc_mpc_split(&key(), 2, 3, dir_a.path(), &pws).expect("split a");
        let (paths_b, _group_b) = btc_mpc_split(&key(), 2, 3, dir_b.path(), &pws).expect("split b");

        let mixed = vec![paths_a[0].clone(), paths_b[1].clone()];
        let mixed_pws = vec![pws[0].clone(), pws[1].clone()];
        assert!(matches!(
            btc_mpc_sign(&mixed, &mixed_pws, &group_a, &[0u8; 32], |_, _| {}),
            Err(Error::MpcGroupMismatch { .. })
        ));
    }

    #[test]
    fn share_file_round_trip_and_ids() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");
        let (paths, _group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        for path in &paths {
            let read = BtcFrostShare::read(path).expect("read");
            let bytes = read.to_bytes();
            assert_eq!(BtcFrostShare::from_bytes(&bytes).expect("parse"), read);
        }
        assert_eq!(shard_ids(&paths).expect("ids"), vec![1, 2, 3]);
    }

    #[test]
    fn rejects_wrong_magic_and_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");
        let (paths, _group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
        let mut bytes = fs::read(&paths[0]).expect("read file");
        bytes[0] = b'X';
        assert!(matches!(
            BtcFrostShare::from_bytes(&bytes),
            Err(Error::InvalidShardFile(_))
        ));
        let bytes = fs::read(&paths[0]).expect("read file");
        let mut bad_version = bytes;
        bad_version[3] = 99;
        assert!(matches!(
            BtcFrostShare::from_bytes(&bad_version),
            Err(Error::InvalidShardFile(_))
        ));
    }

    /// Cross-protocol isolation: an SSS shard (HX1), a Solana FROST share
    /// (HX2), and a Bitcoin FROST share (HX4) must never be readable as one
    /// another.
    #[test]
    fn cross_protocol_files_are_mutually_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = key();
        let pws = passwords(3, "guardian");

        let sss_paths = crate::init_shards(&key, 2, 3, dir.path(), &pws).expect("sss");
        let (ed_paths, _ed_group) =
            crate::mpc::mpc_split(&key, 2, 3, dir.path(), &pws).expect("ed25519 split");
        let (btc_paths, _btc_group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");

        assert!(matches!(
            BtcFrostShare::read(&sss_paths[0]),
            Err(Error::InvalidShardFile(_))
        ));
        assert!(matches!(
            BtcFrostShare::read(&ed_paths[0]),
            Err(Error::InvalidShardFile(_))
        ));
        assert!(matches!(
            crate::shard::Shard::read(&btc_paths[0]),
            Err(Error::InvalidShardFile(_))
        ));
        assert!(matches!(
            crate::mpc::FrostShare::read(&btc_paths[0]),
            Err(Error::InvalidShardFile(_))
        ));
    }
}
