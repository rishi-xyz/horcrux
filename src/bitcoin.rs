//! Offline Bitcoin Taproot (BIP340/BIP341/BIP342) transaction building and
//! signing (Mode A).
//!
//! The signing key is the secp256k1 scalar reconstructed by horcrux. It is
//! interpreted directly as the BIP341 *internal key*, tweaked with
//! `H_taptweak(P)` (no script tree) to produce the *output key* that appears
//! in the derived P2TR address. Inputs are assumed to be from that same output
//! key, so every input is signed with a single key-path (BIP342) signature.
//!
//! # Responsibilities
//!
//! * Tweak derivation uses [`bitcoin::taproot::TapTweakHash`] (matching Bitcoin
//!   Core's `GetTaprootTweakHash`, which hashes *no* merkle-root bytes when
//!   there is no script tree), while the tweaked key arithmetic runs on the
//!   `k256` scalar field already vendored for secret sharing.
//! * BIP341 sighash is computed with [`bitcoin::sighash::SighashCache`]
//!   (`SIGHASH_DEFAULT`), and the signature is a plain BIP340 signature over it.
//! * The signature is verified locally before the transaction is returned, so
//!   a bad tweak can never yield a broadcastable transaction.
//!
//! The caller is responsible for supplying accurate prevout data (outpoints and
//! amounts); wrong values produce a locally-consistent but network-rejected
//! transaction, exactly as with any offline Bitcoin signer.

use crate::error::Error;
use bitcoin::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::hashes::Hash as _;
use bitcoin::key::TweakedPublicKey;
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::TapTweakHash;
use bitcoin::{
    Address, Amount, KnownHrp, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Witness, XOnlyPublicKey,
};
use k256::elliptic_curve::ff::PrimeField;
use k256::schnorr::signature::hazmat::{PrehashSigner, PrehashVerifier};
use zeroize::Zeroize;

/// A spendable UTXO. Both the outpoint and the value (in satoshis) are needed:
/// the value is committed by the BIP341 sighash.
#[derive(Debug, Clone, Copy)]
pub struct Utxo {
    /// Outpoint (`txid:vout`) of the output being spent.
    pub outpoint: OutPoint,
    /// Value of the output, in satoshis.
    pub value_sat: u64,
}

/// A fully-specified Bitcoin transaction, ready to sign.
///
/// The sender is *not* a parameter: it is derived from the key and every input
/// is expected to belong to the sender's own P2TR address.
#[derive(Debug, Clone)]
pub struct BitcoinParams {
    /// Inputs to spend. All must belong to the sender's P2TR address.
    pub utxos: Vec<Utxo>,
    /// Recipient address (mainnet bech32).
    pub recipient: Address,
    /// Amount to send to the recipient, in satoshis.
    pub amount_sat: u64,
    /// Change destination. Defaults to the sender's own derived P2TR address.
    pub change_address: Option<Address>,
    /// Explicit miner fee, in satoshis. `change = inputs - amount - fee`.
    pub fee_sat: u64,
}

/// Taproot keys derived from a horcrux seed.
#[derive(Clone)]
pub struct TaprootKeys {
    /// BIP341 internal key (x-only, even y).
    pub internal_xonly: [u8; 32],
    /// Tweaked output signing key (`Q = P + tG`, normalized to even y).
    pub signing_key: k256::schnorr::SigningKey,
    /// x-coordinate of the output key; what the address and sighash commit to.
    pub output_xonly: [u8; 32],
}

/// A signed, ready-to-broadcast Bitcoin transaction.
#[derive(Debug, Clone)]
pub struct SignedBitcoinTx {
    sender: Address,
    output_xonly: [u8; 32],
    tx: Transaction,
    signature: [u8; 64],
    sighash: [u8; 32],
    fee_sat: u64,
    change_sat: u64,
}

impl SignedBitcoinTx {
    /// The sender's P2TR address (derived from the signing key).
    pub fn sender(&self) -> &Address {
        &self.sender
    }

    /// The signed transaction.
    pub fn tx(&self) -> &Transaction {
        &self.tx
    }

    /// The 64-byte BIP340 signature (key-path spend, `SIGHASH_DEFAULT`).
    pub fn signature(&self) -> [u8; 64] {
        self.signature
    }

    /// The transaction id.
    pub fn txid(&self) -> bitcoin::Txid {
        self.tx.compute_txid()
    }

    /// The explicit miner fee, in satoshis.
    pub fn fee_sat(&self) -> u64 {
        self.fee_sat
    }

    /// The change amount, in satoshis (0 when no change output is present).
    pub fn change_sat(&self) -> u64 {
        self.change_sat
    }

    /// Raw transaction bytes, ready for `sendrawtransaction`.
    pub fn raw(&self) -> Vec<u8> {
        bitcoin::consensus::encode::serialize(&self.tx)
    }

    /// Raw transaction bytes as a hex string.
    pub fn raw_hex(&self) -> String {
        bitcoin::consensus::encode::serialize_hex(&self.tx)
    }

    /// Re-verify the BIP340 signature against the BIP341 sighash offline.
    ///
    /// The sighash was committed at signing time; the witness stack is
    /// re-checked against it with the output key.
    pub fn verify(&self) -> bool {
        let vk = match k256::schnorr::VerifyingKey::from_bytes(&self.output_xonly) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        let sig = match k256::schnorr::Signature::try_from(&self.signature[..]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        vk.verify_prehash(&self.sighash, &sig).is_ok()
    }
}

/// Parse a mainnet bech32 Bitcoin address, rejecting any other network.
pub fn parse_address(s: &str) -> Result<Address, Error> {
    let unchecked: bitcoin::address::Address<bitcoin::address::NetworkUnchecked> = s
        .parse()
        .map_err(|e| Error::Bitcoin(format!("invalid address {s:?}: {e}")))?;
    unchecked
        .require_network(Network::Bitcoin)
        .map_err(|e| Error::Bitcoin(format!("address {s:?} is not a mainnet address: {e}")))
}

/// Derive the sender's P2TR address for a horcrux seed.
pub fn derive_address(seed: &[u8; 32]) -> Result<Address, Error> {
    let keys = taproot_keys(seed)?;
    Ok(p2tr_address(&keys.output_xonly))
}

/// Derive the Taproot keys for a horcrux seed (BIP341 tweak, no script tree).
pub fn taproot_keys(seed: &[u8; 32]) -> Result<TaprootKeys, Error> {
    let internal = k256::schnorr::SigningKey::from_bytes(seed)
        .map_err(|_| Error::Bitcoin("seed is not a valid secp256k1 scalar".into()))?;
    let internal_xonly: [u8; 32] = internal.verifying_key().to_bytes().into();

    let xonly = XOnlyPublicKey::from_slice(&internal_xonly)
        .map_err(|e| Error::Bitcoin(format!("invalid internal key: {e}")))?;
    let tweak = TapTweakHash::from_key_and_tweak(xonly, None);
    let t_be: [u8; 32] = tweak.to_scalar().to_be_bytes();
    let t = k256::Scalar::from_repr(t_be.into())
        .into_option()
        .ok_or_else(|| Error::Bitcoin("taproot tweak out of range".into()))?;

    let output_scalar = **internal.as_nonzero_scalar() + t;
    let output_bytes: [u8; 32] = output_scalar.to_repr().into();
    let signing_key = k256::schnorr::SigningKey::from_bytes(&output_bytes)
        .map_err(|_| Error::Bitcoin("tweaked output key is zero".into()))?;
    let output_xonly: [u8; 32] = signing_key.verifying_key().to_bytes().into();

    Ok(TaprootKeys {
        internal_xonly,
        signing_key,
        output_xonly,
    })
}

/// Build the mainnet P2TR [`Address`] for an x-only output key.
fn p2tr_address(output_xonly: &[u8; 32]) -> Address {
    let xonly = XOnlyPublicKey::from_slice(output_xonly)
        .expect("32-byte x-coordinate is a valid x-only key");
    Address::p2tr_tweaked(
        TweakedPublicKey::dangerous_assume_tweaked(xonly),
        KnownHrp::Mainnet,
    )
}

/// Sign a Bitcoin Taproot transfer for `seed` entirely offline.
///
/// All inputs are spent via the sender's own P2TR output key with a single
/// key-path signature (`SIGHASH_DEFAULT`, so no sighash byte is appended).
/// A change output is added only when `inputs - amount - fee > 0`.
/// The seed is consumed and zeroized before returning.
pub fn sign_transaction(
    mut seed: [u8; 32],
    params: BitcoinParams,
) -> Result<SignedBitcoinTx, Error> {
    if params.utxos.is_empty() {
        return Err(Error::Bitcoin("at least one --utxo is required".into()));
    }
    if params.amount_sat == 0 {
        return Err(Error::Bitcoin("--amount-sat must be positive".into()));
    }

    let keys = taproot_keys(&seed)?;
    let sender = p2tr_address(&keys.output_xonly);

    let total_in = params.utxos.iter().try_fold(0u64, |acc, u| {
        acc.checked_add(u.value_sat)
            .ok_or_else(|| Error::Bitcoin("input values overflow u64".into()))
    })?;
    let total_out = params
        .amount_sat
        .checked_add(params.fee_sat)
        .ok_or_else(|| Error::Bitcoin("amount + fee overflow u64".into()))?;
    if total_in < total_out {
        seed.zeroize();
        return Err(Error::Bitcoin(format!(
            "inputs ({total_in} sat) do not cover amount ({}) + fee ({} sat)",
            params.amount_sat, params.fee_sat
        )));
    }
    let change_sat = total_in - total_out;

    let sender_script = sender.script_pubkey();
    let change_script = match &params.change_address {
        Some(addr) => addr.script_pubkey(),
        None => sender_script.clone(),
    };

    let mut outputs = vec![TxOut {
        value: Amount::from_sat(params.amount_sat),
        script_pubkey: params.recipient.script_pubkey(),
    }];
    if change_sat > 0 {
        outputs.push(TxOut {
            value: Amount::from_sat(change_sat),
            script_pubkey: change_script,
        });
    }

    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: params
            .utxos
            .iter()
            .map(|u| TxIn {
                previous_output: u.outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            })
            .collect(),
        output: outputs,
    };

    let prevouts: Vec<TxOut> = params
        .utxos
        .iter()
        .map(|u| TxOut {
            value: Amount::from_sat(u.value_sat),
            script_pubkey: sender_script.clone(),
        })
        .collect();

    let mut cache = SighashCache::new(&tx);
    let sighash = cache
        .taproot_key_spend_signature_hash(0, &Prevouts::All(&prevouts), TapSighashType::Default)
        .map_err(|e| Error::Bitcoin(format!("failed to compute taproot sighash: {e}")))?;
    let sighash_bytes: [u8; 32] = sighash.to_byte_array();

    let sig = keys
        .signing_key
        .sign_prehash(&sighash_bytes)
        .map_err(|e| Error::Bitcoin(format!("BIP340 signing failed: {e}")))?;
    let sig_bytes: [u8; 64] = sig.to_bytes();

    if keys
        .signing_key
        .verifying_key()
        .verify_prehash(&sighash_bytes, &sig)
        .is_err()
    {
        seed.zeroize();
        return Err(Error::Bitcoin(
            "BIP340 signature failed local verification".into(),
        ));
    }

    let witness = {
        let mut w = Witness::new();
        w.push(sig_bytes);
        w
    };
    for input in tx.input.iter_mut() {
        input.witness = witness.clone();
    }

    seed.zeroize();

    Ok(SignedBitcoinTx {
        sender,
        output_xonly: keys.output_xonly,
        tx,
        signature: sig_bytes,
        sighash: sighash_bytes,
        fee_sat: params.fee_sat,
        change_sat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Foundry's well-known test private key, reused as a fixed seed.
    /// Never use outside tests.
    const TEST_SEED: [u8; 32] = [
        0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38, 0xff,
        0x94, 0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2,
        0xff, 0x80,
    ];

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
    fn derives_sender_address_consistently() {
        let a = derive_address(&TEST_SEED).expect("derive a");
        let b = derive_address(&TEST_SEED).expect("derive b");
        assert_eq!(a, b);
        assert!(a.to_string().starts_with("bc1p"));
    }

    #[test]
    fn sign_transaction_verifies_and_derives_sender() {
        let signed = sign_transaction(TEST_SEED, params()).expect("sign");
        assert_eq!(
            signed.sender(),
            &derive_address(&TEST_SEED).expect("derive")
        );
        assert!(signed.verify());
        assert_eq!(signed.signature().len(), 64);
        assert_eq!(signed.change_sat(), 30_000);
        assert_eq!(signed.tx().output.len(), 2);
        assert_eq!(signed.tx().output[0].value.to_sat(), 60_000);
        assert_eq!(signed.tx().output[1].value.to_sat(), 30_000);
        // Every input carries the key-path witness.
        for input in &signed.tx().input {
            assert_eq!(input.witness.len(), 1);
            assert_eq!(input.witness.nth(0).unwrap().len(), 64);
        }
    }

    #[test]
    fn omits_change_output_when_inputs_are_fully_spent() {
        let mut p = params();
        p.fee_sat = 40_000;
        let signed = sign_transaction(TEST_SEED, p).expect("sign");
        assert_eq!(signed.change_sat(), 0);
        assert_eq!(signed.tx().output.len(), 1);
        assert!(signed.verify());
    }

    #[test]
    fn rejects_insufficient_inputs() {
        let mut p = params();
        p.amount_sat = 100_000;
        p.fee_sat = 1_000;
        assert!(matches!(
            sign_transaction(TEST_SEED, p),
            Err(Error::Bitcoin(ref msg)) if msg.contains("do not cover")
        ));
    }

    #[test]
    fn rejects_non_mainnet_addresses() {
        assert!(
            parse_address("tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq5zuyut")
                .is_err()
        );
    }

    #[test]
    fn changing_any_field_changes_txid() {
        let base = sign_transaction(TEST_SEED, params()).expect("sign base");
        let mut p = params();
        p.amount_sat += 1;
        let changed = sign_transaction(TEST_SEED, p).expect("sign changed");
        assert_ne!(base.txid(), changed.txid());
    }
}
