//! End-to-end tests for Phase 3: the access log records every shard
//! decryption, the scorer blocks repeat-failure patterns, and audited
//! reconstruction writes matching entries.

use horcrux::audit::{AccessLog, Entry, EntryKind, Scorer, Verdict};
use horcrux::{init_shards, reconstruct_with_audit};
use k256::SecretKey;
use rand::rngs::OsRng;

fn passwords(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("guardian-{i}")).collect()
}

fn setup() -> (tempfile::TempDir, Vec<std::path::PathBuf>, Vec<String>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = SecretKey::random(&mut OsRng);
    let pws = passwords(3);
    let paths = init_shards(&key, 2, 3, dir.path(), &pws).expect("init");
    (dir, paths, pws)
}

#[test]
fn repeated_failures_block_a_sign_attempt() {
    let (_dir, paths, _pws) = setup();
    let log = AccessLog::open(paths[0].parent().unwrap().join("access.log"));
    let now = 1_700_000_000_000;
    for i in 0..5 {
        log.append(&Entry::fail(now - (5 - i) * 10_000, 100 + i, 2))
            .expect("append failure");
    }

    let ids = horcrux::audit::shard_ids(&paths[..2]).expect("shard ids");
    let verdict = Scorer::new().assess(&log.read_all().expect("read"), &ids, now);
    assert!(
        matches!(verdict, Verdict::Block(_)),
        "five recent failures must block, got {verdict:?}"
    );
}

#[test]
fn clean_history_allows_and_logs_decrypts() {
    let (_dir, paths, pws) = setup();
    let log = AccessLog::open(paths[0].parent().unwrap().join("access.log"));
    let now = 1_700_000_000_000;
    let ids = horcrux::audit::shard_ids(&paths[..2]).expect("shard ids");
    // A prior successful attempt using the same two shards.
    log.append(&Entry::ok(now - 86_400_000, 1, ids[0]))
        .expect("append");
    log.append(&Entry::ok(now - 86_400_000, 1, ids[1]))
        .expect("append");

    assert!(
        matches!(
            Scorer::new().assess(&log.read_all().expect("read"), &ids, now),
            Verdict::Allow
        ),
        "known combination at a normal hour must be allowed"
    );

    // Reconstruction through the audited path must write decrypt_ok entries.
    let key = reconstruct_with_audit(&paths[..2], &pws[..2], &log, 2).expect("reconstruct");
    assert_eq!(key.to_bytes().len(), 32);

    let all = log.read_all().expect("read");
    let ok_entries: Vec<&Entry> = all
        .iter()
        .filter(|e| e.kind == EntryKind::DecryptOk && e.attempt == 2)
        .collect();
    assert_eq!(ok_entries.len(), 2, "each shard decryption must be logged");
}

#[test]
fn failed_password_is_logged_as_a_failure() {
    let (_dir, paths, pws) = setup();
    let log = AccessLog::open(paths[0].parent().unwrap().join("access.log"));
    let ids = horcrux::audit::shard_ids(&paths[..2]).expect("shard ids");

    let bad = vec!["wrong".to_string(), pws[1].clone()];
    assert!(reconstruct_with_audit(&paths[..2], &bad, &log, 7).is_err());

    let all = log.read_all().expect("read");
    let fails: Vec<&Entry> = all
        .iter()
        .filter(|e| e.attempt == 7 && e.kind == EntryKind::DecryptFail)
        .collect();
    assert_eq!(fails.len(), 1, "the failed shard must be logged");
    assert_eq!(fails[0].shard_id, ids[0]);
}

#[test]
fn blocked_attempt_is_recorded() {
    let (_dir, paths, _pws) = setup();
    let log = AccessLog::open(paths[0].parent().unwrap().join("access.log"));
    let now = 1_700_000_000_000;
    log.append(&Entry::blocked(now, 42)).expect("append");

    let all = log.read_all().expect("read");
    assert!(
        all.iter()
            .any(|e| e.attempt == 42 && e.kind == EntryKind::Blocked)
    );
}
