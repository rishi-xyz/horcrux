//! End-to-end tests for Bitcoin Mode B signing: dealer-split a key into
//! Bitcoin FROST key shares, sign a Taproot transfer from any threshold
//! subset of share files, and verify the aggregated signature under plain
//! BIP340 — proving the key is never reconstructed and the result is
//! broadcastable, and that the group address matches the Mode A address.

use bitcoin::OutPoint;
use horcrux::bitcoin::{BitcoinParams, Utxo, derive_address};
use horcrux::btc_mpc::{btc_mpc_split, shard_ids};
use horcrux::error::Error;
use horcrux::sign_bitcoin_transaction_from_mpc_shares;
use k256::SecretKey;
use rand::rngs::OsRng;
use std::str::FromStr;

fn key() -> SecretKey {
    SecretKey::random(&mut OsRng)
}

fn passwords(n: usize, seed: &str) -> Vec<String> {
    (0..n).map(|i| format!("{seed}-{i}")).collect()
}

fn seed_of(key: &SecretKey) -> [u8; 32] {
    key.to_bytes().into()
}

fn params() -> BitcoinParams {
    BitcoinParams {
        utxos: vec![Utxo {
            outpoint: OutPoint::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001:0",
            )
            .expect("valid outpoint"),
            value_sat: 100_000,
        }],
        recipient: derive_address(&[0x1b; 32]).expect("derived recipient"),
        amount_sat: 60_000,
        change_address: None,
        fee_sat: 10_000,
    }
}

#[test]
fn mpc_sign_from_any_two_of_three_recovers_sender_and_verifies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = key();
    let seed = seed_of(&key);
    let pws = passwords(3, "guardian");

    let (paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
    assert_eq!(paths.len(), 3);
    assert_eq!(shard_ids(&paths).expect("ids"), vec![1, 2, 3]);

    let expected_sender = derive_address(&seed).expect("mode a address");

    for combo in [&[0usize, 1], &[0usize, 2], &[1usize, 2]] {
        let chosen: Vec<_> = combo.iter().map(|&i| paths[i].clone()).collect();
        let chosen_pws: Vec<_> = combo.iter().map(|&i| pws[i].clone()).collect();
        let signed =
            sign_bitcoin_transaction_from_mpc_shares(&chosen, &chosen_pws, &group, params())
                .expect("sign");
        assert_eq!(
            signed.sender(),
            &expected_sender,
            "Mode B group address must equal the Mode A address"
        );
        assert!(signed.verify(), "broadcastable, BIP340-valid transaction");
    }
}

#[test]
fn one_share_cannot_sign() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = key();
    let pws = passwords(3, "guardian");
    let (paths, group) = btc_mpc_split(&key, 2, 3, dir.path(), &pws).expect("split");
    assert!(matches!(
        sign_bitcoin_transaction_from_mpc_shares(&paths[..1], &pws[..1], &group, params()),
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
        sign_bitcoin_transaction_from_mpc_shares(&paths[..2], &bad, &group, params()),
        Err(Error::Decrypt { .. })
    ));
}
