//! End-to-end tests for Mode A signing: reconstruct a key from shards and sign
//! an EVM transaction entirely offline, verifying the sender address and that
//! the signature round-trips.

use alloy::primitives::address;
use horcrux::error::Error;
use horcrux::tx::{DEFAULT_CHAIN_ID, Fee, TxParams};
use horcrux::{init_shards, sign_transaction_from_shards};
use k256::SecretKey;
use rand::rngs::OsRng;

fn key() -> SecretKey {
    SecretKey::random(&mut OsRng)
}

fn passwords(n: usize, seed: &str) -> Vec<String> {
    (0..n).map(|i| format!("{seed}-{i}")).collect()
}

fn params() -> TxParams {
    TxParams {
        to: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
        value: alloy::primitives::U256::from(1_000_000u64),
        data: alloy::primitives::Bytes::new(),
        chain_id: DEFAULT_CHAIN_ID,
        nonce: 0,
        gas_limit: 21_000,
        fee: Fee::Eip1559 {
            max_fee_per_gas: 20_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
        },
    }
}

#[test]
fn sign_from_two_of_three_shards_recovers_sender() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = key();
    let pws = passwords(3, "guardian");

    let paths = init_shards(&key, 2, 3, dir.path(), &pws).expect("init");
    let out = sign_transaction_from_shards(&paths[..2], &pws[..2], params()).expect("sign");

    assert_eq!(
        out.from,
        horcrux::tx::derive_address(&key),
        "signer address must match the reconstructed key"
    );
    assert_eq!(
        out.tx_hash,
        alloy::primitives::keccak256(alloy::primitives::hex::decode(&out.raw_hex).expect("hex")),
        "tx hash must equal keccak of the raw encoding"
    );
}

#[test]
fn signed_output_is_deterministic_for_same_inputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = key();
    let pws = passwords(3, "guardian");
    let paths = init_shards(&key, 2, 3, dir.path(), &pws).expect("init");

    let a = sign_transaction_from_shards(&paths[..2], &pws[..2], params()).expect("sign a");
    let b = sign_transaction_from_shards(&paths[..2], &pws[..2], params()).expect("sign b");
    assert_eq!(a.tx_hash, b.tx_hash);
    assert_eq!(a.raw_hex, b.raw_hex);
    assert_eq!(a.from, b.from);
}

#[test]
fn wrong_password_aborts_before_signing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = key();
    let pws = passwords(3, "guardian");
    let paths = init_shards(&key, 2, 3, dir.path(), &pws).expect("init");

    let bad = vec!["wrong".to_string(), pws[1].clone()];
    assert!(matches!(
        sign_transaction_from_shards(&paths[..2], &bad, params()),
        Err(Error::Decrypt { .. })
    ));
}
