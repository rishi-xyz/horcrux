//! Offline Solana transaction building and signing (Mode A).
//!
//! This module builds and signs a Solana transaction entirely in memory:
//! no RPC access and no network I/O. The caller supplies a fully-specified
//! [`TxParams`]; [`sign_transaction`] derives the Ed25519/Solana address from
//! the seed, signs the message, and returns a [`SignedTx`] ready to broadcast.
//!
//! The signing key is an Ed25519 [`SigningKey`](ed25519_dalek::SigningKey)
//! built from the seed; with the crate's `zeroize` feature it wipes its scalar
//! on drop, and the caller's seed buffer is zeroized before returning.
//!
//! # Why we sign `message.serialize()` and not `message.hash()`
//!
//! The bytes Ed25519 signs are the bincode serialization of the legacy
//! [`Message`] (`message.serialize()`). This matches the wire encoding that
//! `solana-rpc-client` hands to the cluster for `sendTransaction`, so a
//! signature produced here verifies exactly what the node will see.

use crate::error::Error;
use ed25519_dalek::Signer as _;
use solana_hash::Hash;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use zeroize::Zeroize;

/// A fully-specified Solana transaction, ready to sign.
#[derive(Debug, Clone)]
pub struct TxParams {
    /// Sender address (fee payer). Must match the address derived from the key.
    pub from: Pubkey,
    /// Recipient address.
    pub to: Pubkey,
    /// Amount to transfer, in lamports (1 SOL = 1_000_000_000 lamports).
    pub lamports: u64,
    /// Recent blockhash (fetched live, or supplied offline via `--blockhash`).
    pub blockhash: Hash,
}

/// A signed, ready-to-broadcast transaction.
#[derive(Debug, Clone)]
pub struct SignedTx {
    from: Pubkey,
    tx: Transaction,
    signature: Signature,
}

impl SignedTx {
    /// The sender address (derived from the signing key).
    pub fn from(&self) -> Pubkey {
        self.from
    }

    /// The signed transaction.
    pub fn tx(&self) -> &Transaction {
        &self.tx
    }

    /// The Ed25519 signature (also serves as the Solana transaction id).
    pub fn signature(&self) -> Signature {
        self.signature
    }

    /// The raw bincode serialization of the transaction, ready for
    /// `sendTransaction`.
    pub fn raw(&self) -> Vec<u8> {
        bincode::serialize(&self.tx).expect("transaction is serializable")
    }

    /// The raw transaction as a base58 string (what `solana rpc sendTransaction`
    /// expects).
    pub fn raw_base58(&self) -> String {
        bs58::encode(self.raw()).into_string()
    }

    /// Verify the Ed25519 signature against the serialized message offline.
    pub fn verify(&self) -> bool {
        self.signature
            .verify(&self.from.to_bytes(), &self.tx.message.serialize())
    }
}

/// Derive the Solana address (base58 pubkey) for an Ed25519 seed.
pub fn derive_address(seed: &[u8; 32]) -> Pubkey {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(seed);
    Pubkey::from(signing_key.verifying_key().to_bytes())
}

/// Sign a Solana transfer for `seed` entirely offline.
///
/// The seed is consumed; the only in-memory copy of the key lives inside the
/// [`SigningKey`](ed25519_dalek::SigningKey), which zeroizes it on drop, and
/// the caller's buffer is zeroized before returning.
pub fn sign_transaction(mut seed: [u8; 32], params: TxParams) -> Result<SignedTx, Error> {
    let from = derive_address(&seed);
    if params.from != from {
        seed.zeroize();
        return Err(Error::Tx(format!(
            "derived address {from} does not match --from {}",
            params.from
        )));
    }

    let instruction =
        solana_system_interface::instruction::transfer(&params.from, &params.to, params.lamports);
    let message =
        Message::new_with_blockhash(&[instruction], Some(&params.from), &params.blockhash);
    let sign_bytes = message.serialize();

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let dalek_sig = signing_key.sign(&sign_bytes);
    let signature = Signature::from(dalek_sig.to_bytes());

    if !signature.verify(&params.from.to_bytes(), &sign_bytes) {
        seed.zeroize();
        return Err(Error::Tx("signature failed local verification".into()));
    }

    seed.zeroize();

    Ok(SignedTx {
        from,
        tx: Transaction {
            signatures: vec![signature],
            message,
        },
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Foundry's well-known test private key, reused as a fixed Ed25519 seed.
    /// Never use outside tests.
    const TEST_SEED: [u8; 32] = [
        0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38, 0xff,
        0x94, 0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2,
        0xff, 0x80,
    ];

    fn params() -> TxParams {
        TxParams {
            from: derive_address(&TEST_SEED),
            to: derive_address(&[0x1b; 32]),
            lamports: 1_000_000,
            blockhash: Hash::new_from_array([0x5a; 32]),
        }
    }

    #[test]
    fn derives_address_from_verifying_key() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&TEST_SEED);
        let expected = Pubkey::from(signing_key.verifying_key().to_bytes());
        assert_eq!(derive_address(&TEST_SEED), expected);
    }

    #[test]
    fn sign_transaction_verifies_and_records_sender() {
        let signed = sign_transaction(TEST_SEED, params()).expect("sign");
        assert_eq!(signed.from(), derive_address(&TEST_SEED));
        assert!(signed.verify());
        assert_eq!(signed.tx().signatures.len(), 1);
        assert_eq!(signed.tx().signatures[0], signed.signature());
    }

    #[test]
    fn rejects_from_mismatch() {
        let mut p = params();
        p.from = derive_address(&[7u8; 32]);
        let err = sign_transaction(TEST_SEED, p).expect_err("from mismatch must error");
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn signature_is_over_serialized_message() {
        let signed = sign_transaction(TEST_SEED, params()).expect("sign");
        let sign_bytes = signed.tx().message.serialize();
        assert!(
            signed
                .signature()
                .verify(&signed.from().to_bytes(), &sign_bytes)
        );
    }

    #[test]
    fn raw_base58_round_trips_through_bincode() {
        let signed = sign_transaction(TEST_SEED, params()).expect("sign");
        let raw = signed.raw();
        let decoded: Transaction = bincode::deserialize(&raw).expect("deserializes");
        assert_eq!(bincode::serialize(&decoded).expect("reserializes"), raw);
        assert_eq!(decoded.signatures, signed.tx().signatures);
        assert_eq!(decoded.message.serialize(), signed.tx().message.serialize());

        let from_b58 = bs58::decode(signed.raw_base58())
            .into_vec()
            .expect("valid base58 raw");
        let decoded_from_b58: Transaction = bincode::deserialize(&from_b58).expect("deserializes");
        assert_eq!(decoded_from_b58.signatures, signed.tx().signatures);
    }

    #[test]
    fn different_blockhash_produces_different_signature() {
        let mut p2 = params();
        p2.blockhash = Hash::new_from_array([0x6b; 32]);
        let a = sign_transaction(TEST_SEED, params()).expect("sign a");
        let b = sign_transaction(TEST_SEED, p2).expect("sign b");
        assert_ne!(a.signature(), b.signature());
    }

    #[test]
    fn different_lamports_produce_different_signature() {
        let mut p2 = params();
        p2.lamports += 1;
        let a = sign_transaction(TEST_SEED, params()).expect("sign a");
        let b = sign_transaction(TEST_SEED, p2).expect("sign b");
        assert_ne!(a.signature(), b.signature());
    }
}
