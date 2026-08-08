//! End-to-end tests for `horcrux verify`: structural integrity checks on SSS
//! shards and FROST key shares, split-consistency reporting, and optional
//! AES-GCM auth-tag verification.

use horcrux::init_shards;
use horcrux::mpc::mpc_split;
use horcrux::verify::{Kind, consistency_error, verify_files};
use k256::SecretKey;
use rand::rngs::OsRng;

fn key() -> SecretKey {
    SecretKey::random(&mut OsRng)
}

fn passwords(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("guardian-{i}")).collect()
}

#[test]
fn verify_sss_shards_with_and_without_password() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pws = vec!["shared-password".to_string(); 3];
    let shards = init_shards(&key(), 2, 3, dir.path(), &pws).expect("init");

    let structural = verify_files(&shards, None);
    assert!(structural.iter().all(|r| r.ok), "{structural:?}");
    assert_eq!(consistency_error(&structural), None);

    let authed = verify_files(&shards, Some(&pws[0]));
    assert!(authed.iter().all(|r| r.ok), "{authed:?}");

    let wrong = verify_files(&shards, Some("wrong-password"));
    assert!(!wrong.iter().all(|r| r.ok));
}

#[test]
fn verify_frost_shares() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pws = passwords(3);
    let (shares, _group) = mpc_split(&key(), 2, 3, dir.path(), &pws).expect("split");

    let reports = verify_files(&shares, None);
    assert!(reports.iter().all(|r| r.ok));
    assert!(reports.iter().all(|r| r.kind == Some(Kind::Frost)));
    assert_eq!(consistency_error(&reports), None);
}

#[test]
fn tampered_shard_is_reported_invalid() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pws = vec!["shared-password".to_string(); 3];
    let shards = init_shards(&key(), 2, 3, dir.path(), &pws).expect("init");

    let mut bytes = std::fs::read(&shards[0]).expect("read");
    let idx = 7 + 16 + 12; // first ciphertext byte
    bytes[idx] ^= 0x01;
    std::fs::write(&shards[0], bytes).expect("rewrite");

    let reports = verify_files(&shards, Some(&pws[0]));
    assert!(!reports[0].ok, "GCM tag must catch tampering");
    assert!(reports[1].ok && reports[2].ok);
}
