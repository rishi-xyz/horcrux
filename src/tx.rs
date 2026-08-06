//! Offline EVM transaction building and signing (Mode A).
//!
//! This module builds and signs a standard EVM transaction entirely in memory:
//! no RPC access and no network I/O. The caller supplies a fully-specified
//! [`TxParams`]; [`sign_transaction`] derives the sender address from the key,
//! hashes and signs the transaction, and returns a [`TxEnvelope`] ready to be
//! RLP-encoded for broadcast.
//!
//! The private key is moved into the signer and never cloned: the scalar is
//! zeroized on drop by `k256` (and, with the `zeroize` feature enabled, by the
//! alloy signer wrapper as well).

use crate::error::Error;
use alloy::consensus::transaction::SignerRecoverable;
use alloy::consensus::{SignableTransaction, TxEnvelope, TxType};
use alloy::eips::eip2718::Encodable2718;
use alloy::network::{NetworkTransactionBuilder, TransactionBuilder};
use alloy::primitives::{Address, B256, Bytes, ChainId, Signature, U256};
use alloy::rpc::types::TransactionRequest;
use k256::SecretKey;
use k256::ecdsa::SigningKey;
use k256::ecdsa::signature::hazmat::PrehashSigner;

/// Default chain id used when none is supplied (Sepolia).
pub const DEFAULT_CHAIN_ID: ChainId = 11155111;

/// Fee model for a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fee {
    /// Legacy EIP-155 transaction priced by a single `gas_price`.
    Legacy { gas_price: u128 },
    /// EIP-1559 fee-market transaction.
    Eip1559 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    },
}

/// A fully-specified EVM transaction, ready to sign.
#[derive(Debug, Clone)]
pub struct TxParams {
    /// Recipient address.
    pub to: Address,
    /// Value in wei.
    pub value: U256,
    /// Calldata (empty for a plain transfer).
    pub data: Bytes,
    /// Chain id, used for EIP-155 replay protection.
    pub chain_id: ChainId,
    /// Account nonce.
    pub nonce: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Fee model.
    pub fee: Fee,
}

impl TxParams {
    /// Build a plain-value-transfer parameter set with an EIP-1559 fee model.
    pub fn eip1559_transfer(
        to: Address,
        value: U256,
        chain_id: ChainId,
        nonce: u64,
        gas_limit: u64,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Self {
        Self {
            to,
            value,
            data: Bytes::new(),
            chain_id,
            nonce,
            gas_limit,
            fee: Fee::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            },
        }
    }
}

/// A signed, ready-to-broadcast transaction.
#[derive(Debug, Clone)]
pub struct SignedTx {
    from: Address,
    envelope: TxEnvelope,
}

impl SignedTx {
    /// The sender address (derived from the signing key).
    pub fn from(&self) -> Address {
        self.from
    }

    /// The signed transaction envelope.
    pub fn envelope(&self) -> &TxEnvelope {
        &self.envelope
    }

    /// The transaction hash (keccak of the EIP-2718 encoding).
    pub fn tx_hash(&self) -> B256 {
        *self.envelope.tx_hash()
    }

    /// The raw EIP-2718 encoding, ready to hand to `eth_sendRawTransaction`.
    pub fn encoded(&self) -> Vec<u8> {
        self.envelope.encoded_2718()
    }

    /// The raw EIP-2718 encoding as a `0x`-prefixed lowercase hex string.
    pub fn raw_hex(&self) -> String {
        alloy::primitives::hex::encode_prefixed(self.encoded())
    }

    /// Recover the signer address from the signature, cross-checking `from`.
    pub fn recover_signer(&self) -> Option<Address> {
        self.envelope.recover_signer().ok()
    }
}

/// Derive the Ethereum address (EIP-55 checksummed) for a private key.
pub fn derive_address(key: &SecretKey) -> Address {
    alloy::signers::utils::secret_key_to_address(&SigningKey::from(key))
}

/// Build the transaction request for the given sender and parameters.
fn build_request(from: Address, params: &TxParams) -> TransactionRequest {
    let mut request = TransactionRequest::default()
        .from(from)
        .to(params.to)
        .value(params.value)
        .with_input(params.data.clone())
        .nonce(params.nonce)
        .gas_limit(params.gas_limit)
        .with_chain_id(params.chain_id);
    match params.fee {
        Fee::Legacy { gas_price } => {
            request = request
                .transaction_type(TxType::Legacy as u8)
                .gas_price(gas_price);
        }
        Fee::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } => {
            request = request
                .transaction_type(TxType::Eip1559 as u8)
                .max_fee_per_gas(max_fee_per_gas)
                .max_priority_fee_per_gas(max_priority_fee_per_gas);
        }
    }
    request
}

/// Sign a transaction for `key` entirely offline.
///
/// The key is consumed and the only copy of the scalar lives inside the
/// [`SigningKey`], which zeroizes it on drop.
pub fn sign_transaction(key: SecretKey, params: TxParams) -> Result<SignedTx, Error> {
    let from = derive_address(&key);
    let request = build_request(from, &params);
    let unsigned = request
        .build_unsigned()
        .map_err(|e| Error::Tx(format!("could not build transaction: {e}")))?;

    let hash = unsigned.signature_hash();
    let signing_key = SigningKey::from(key);
    let (signature, recovery_id) = signing_key
        .sign_prehash(hash.as_ref())
        .map_err(|e| Error::Tx(format!("could not sign transaction: {e}")))?;
    let signature: Signature = (signature, recovery_id).into();

    Ok(SignedTx {
        from,
        envelope: unsigned.into_signed(signature).into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::Transaction;
    use alloy::eips::eip2718::Decodable2718;
    use alloy::primitives::address;
    use rand::rngs::OsRng;

    /// Foundry's well-known test private key. Never use outside tests.
    const TEST_KEY_HEX: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const TEST_KEY_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

    fn test_key() -> SecretKey {
        SecretKey::from_slice(&hex::decode(&TEST_KEY_HEX[2..]).expect("valid test key hex"))
            .expect("valid test key")
    }

    fn params() -> TxParams {
        TxParams {
            to: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
            value: U256::from(1_000_000u64),
            data: Bytes::new(),
            chain_id: DEFAULT_CHAIN_ID,
            nonce: 7,
            gas_limit: 21_000,
            fee: Fee::Eip1559 {
                max_fee_per_gas: 20_000_000_000,
                max_priority_fee_per_gas: 1_000_000_000,
            },
        }
    }

    #[test]
    fn derives_expected_address() {
        let key = test_key();
        assert_eq!(derive_address(&key).to_checksum(None), TEST_KEY_ADDRESS);
    }

    #[test]
    fn derived_address_matches_verifying_key() {
        let key = SecretKey::random(&mut OsRng);
        let expected: Address =
            alloy::signers::utils::secret_key_to_address(&SigningKey::from(&key));
        assert_eq!(derive_address(&key), expected);
    }

    #[test]
    fn eip1559_signing_recovers_sender() {
        let key = test_key();
        let signed = sign_transaction(key, params()).expect("sign");
        assert_eq!(signed.from().to_checksum(None), TEST_KEY_ADDRESS);
        assert_eq!(signed.recover_signer(), Some(signed.from()));
        assert!(signed.envelope().as_eip1559().is_some());
    }

    #[test]
    fn legacy_signing_recovers_sender_and_applies_eip155() {
        let key = test_key();
        let mut params = params();
        params.fee = Fee::Legacy {
            gas_price: 15_000_000_000,
        };
        let signed = sign_transaction(key, params).expect("sign");
        assert_eq!(signed.recover_signer(), Some(signed.from()));

        let legacy = signed.envelope().as_legacy().expect("legacy envelope");
        assert_eq!(legacy.tx().chain_id(), Some(DEFAULT_CHAIN_ID));
        assert_eq!(legacy.tx().gas_price(), Some(15_000_000_000));
    }

    #[test]
    fn raw_hex_round_trips_through_decode_2718() {
        let key = test_key();
        let signed = sign_transaction(key, params()).expect("sign");
        let raw = signed.encoded();
        let mut slice = raw.as_slice();

        let decoded = TxEnvelope::decode_2718(&mut slice).expect("decodes");
        assert_eq!(decoded.tx_hash(), &signed.tx_hash());
        assert_eq!(decoded.recover_signer().expect("recovers"), signed.from());
    }

    #[test]
    fn same_params_produce_same_transaction() {
        let a = sign_transaction(test_key(), params()).expect("sign a");
        let b = sign_transaction(test_key(), params()).expect("sign b");
        assert_eq!(a.tx_hash(), b.tx_hash());
        assert_eq!(a.raw_hex(), b.raw_hex());
    }

    #[test]
    fn different_nonce_produces_different_transaction() {
        let mut p2 = params();
        p2.nonce += 1;
        let a = sign_transaction(test_key(), params()).expect("sign a");
        let b = sign_transaction(test_key(), p2).expect("sign b");
        assert_ne!(a.tx_hash(), b.tx_hash());
    }

    #[test]
    fn data_payload_round_trips() {
        let key = test_key();
        let mut params = params();
        let payload = Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]);
        params.data = payload.clone();
        let signed = sign_transaction(key, params).expect("sign");
        let raw = signed.encoded();
        let mut slice = raw.as_slice();

        let decoded = TxEnvelope::decode_2718(&mut slice).expect("decodes");
        let eip1559 = decoded.as_eip1559().expect("eip1559");
        assert_eq!(eip1559.tx().input(), &payload);
        assert_eq!(decoded.recover_signer().expect("recovers"), signed.from());
    }

    #[test]
    fn tx_hash_matches_rlp_digest() {
        let key = test_key();
        let signed = sign_transaction(key, params()).expect("sign");
        let digest = alloy::primitives::keccak256(signed.encoded());
        assert_eq!(digest, signed.tx_hash());
    }
}
