//! Offline Cosmos SDK (`bank.MsgSend`, `SIGN_MODE_DIRECT`) transaction
//! building and signing (Mode A).
//!
//! The signing key is the secp256k1 scalar reconstructed by horcrux, wrapped
//! in [`cosmrs::crypto::secp256k1::SigningKey`]. The sender address is derived
//! from the key using the bech32 HRP of the recipient address (they are on the
//! same chain, so they share a prefix); Cosmos address derivation is
//! `RIPEMD160(SHA-256(compressed_pubkey))`.
//!
//! The transaction is built with `cosmrs` exactly as its `tx` module documents
//! (protobuf `Body` + `AuthInfo`, `SignDoc` with `SIGN_MODE_DIRECT`) and the
//! signature is an RFC 6979 deterministic ECDSA/secp256k1 signature (low-S)
//! produced by `k256`.

use crate::error::Error;
use cosmrs::bank::MsgSend;
use cosmrs::crypto::secp256k1;
use cosmrs::tx::{Fee, Msg, SignDoc, SignerInfo};
use cosmrs::{AccountId, Coin};
use zeroize::Zeroize;

/// A fully-specified Cosmos `bank.MsgSend`, ready to sign.
///
/// Amounts are in the chain's base denomination units (e.g. `uatom` for one
/// millionth of an ATOM).
#[derive(Debug, Clone)]
pub struct CosmosParams {
    /// Chain id (e.g. `cosmoshub-4`). Bound into the sign doc to prevent
    /// replay across chains.
    pub chain_id: String,
    /// On-chain account number (from chain state or a recent query).
    pub account_number: u64,
    /// Account sequence (number of previously committed transactions).
    pub sequence: u64,
    /// Recipient address (bech32).
    pub to: AccountId,
    /// Amount to send, in base denomination units.
    pub amount: u128,
    /// Denomination of `amount` (e.g. `uatom`).
    pub denom: String,
    /// Transaction memo.
    pub memo: String,
    /// Reject if the chain has advanced past this height (0 disables).
    pub timeout_height: u64,
    /// Gas limit.
    pub gas: u64,
    /// Fee amount, in base denomination units.
    pub fee_amount: u128,
    /// Denomination of `fee_amount` (defaults to `denom`).
    pub fee_denom: String,
}

/// A signed, ready-to-broadcast Cosmos transaction.
#[derive(Debug, Clone)]
pub struct SignedCosmosTx {
    from: AccountId,
    raw_bytes: Vec<u8>,
    signature: Vec<u8>,
}

impl SignedCosmosTx {
    /// The sender address (derived from the signing key).
    pub fn from(&self) -> &AccountId {
        &self.from
    }

    /// The DER-encoded ECDSA signature from the auth info.
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Raw protobuf (`TxRaw`) bytes, ready for `tx broadcast sync`.
    pub fn raw(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Raw transaction bytes as hex.
    pub fn raw_hex(&self) -> String {
        hex::encode(&self.raw_bytes)
    }
}

/// Default Tendermint RPC endpoint (a local `simd`/`gaiad` node), overridden
/// by `--rpc-url` or `$HORCRUX_COSMOS_RPC_URL`.
pub fn default_rpc_url() -> String {
    std::env::var("HORCRUX_COSMOS_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:26657".to_string())
}

/// Broadcast a signed transaction via `/broadcast_tx_commit` and wait for it
/// to be included in a block. Returns the transaction hash on success; a
/// non-OK `CheckTx` or delivery result is reported as an error rather than a
/// silently "successful" broadcast of a rejected transaction.
pub async fn broadcast(rpc_url: &str, tx: &SignedCosmosTx) -> Result<String, Error> {
    use cosmrs::rpc::Client as _;

    let client = cosmrs::rpc::HttpClient::new(rpc_url)
        .map_err(|e| Error::Cosmos(format!("failed to connect to {rpc_url}: {e}")))?;
    let response = client
        .broadcast_tx_commit(tx.raw_bytes.clone())
        .await
        .map_err(|e| Error::Cosmos(format!("broadcast failed: {e}")))?;

    if response.check_tx.code.is_err() {
        return Err(Error::Cosmos(format!(
            "check_tx rejected the transaction: code {:?}: {}",
            response.check_tx.code, response.check_tx.log
        )));
    }
    if response.tx_result.code.is_err() {
        return Err(Error::Cosmos(format!(
            "transaction delivery failed: code {:?}: {}",
            response.tx_result.code, response.tx_result.log
        )));
    }
    Ok(response.hash.to_string())
}

/// Parse a bech32 Cosmos account address.
pub fn parse_address(s: &str) -> Result<AccountId, Error> {
    s.parse()
        .map_err(|e| Error::Cosmos(format!("invalid account address {s:?}: {e}")))
}

/// Derive the sender address for a horcrux seed and an HRP (e.g. `cosmos`).
pub fn derive_address(seed: &[u8; 32], hrp: &str) -> Result<AccountId, Error> {
    let key = secp256k1::SigningKey::from_slice(seed)
        .map_err(|e| Error::Cosmos(format!("invalid secp256k1 key: {e}")))?;
    key.public_key()
        .account_id(hrp)
        .map_err(|e| Error::Cosmos(format!("cannot derive account from hrp {hrp:?}: {e}")))
}

/// Sign a Cosmos `bank.MsgSend` for `seed` entirely offline.
///
/// The sender HRP is taken from the recipient address. The seed is consumed
/// and zeroized before returning.
pub fn sign_transaction(mut seed: [u8; 32], params: CosmosParams) -> Result<SignedCosmosTx, Error> {
    if params.amount == 0 {
        return Err(Error::Cosmos("--amount must be positive".into()));
    }
    if params.gas == 0 {
        return Err(Error::Cosmos("--gas must be positive".into()));
    }

    let key = secp256k1::SigningKey::from_slice(&seed)
        .map_err(|e| Error::Cosmos(format!("invalid secp256k1 key: {e}")))?;
    let public_key = key.public_key();
    let from = public_key
        .account_id(params.to.prefix())
        .map_err(|e| Error::Cosmos(format!("cannot derive sender address: {e}")))?;

    let denom: cosmrs::Denom = params
        .denom
        .parse()
        .map_err(|e| Error::Cosmos(format!("invalid --denom {:?}: {e}", params.denom)))?;
    let fee_denom: cosmrs::Denom = if params.fee_denom.is_empty() {
        denom.clone()
    } else {
        params.fee_denom.parse().map_err(|e| {
            Error::Cosmos(format!("invalid --fee-denom {:?}: {e}", params.fee_denom))
        })?
    };

    let amount = Coin {
        amount: params.amount,
        denom,
    };
    let msg_send = MsgSend {
        from_address: from.clone(),
        to_address: params.to.clone(),
        amount: vec![amount.clone()],
    };

    let timeout_height = u32::try_from(params.timeout_height)
        .map_err(|_| Error::Cosmos("--timeout-height exceeds supported range".into()))?;

    let tx_body = cosmrs::tx::Body::new(
        vec![
            msg_send
                .to_any()
                .map_err(|e| Error::Cosmos(format!("{}", e)))?,
        ],
        params.memo,
        timeout_height,
    );

    let signer_info = SignerInfo::single_direct(Some(public_key), params.sequence);
    let auth_info = signer_info.auth_info(Fee::from_amount_and_gas(
        Coin {
            amount: params.fee_amount,
            denom: fee_denom,
        },
        params.gas,
    ));

    let chain_id = params
        .chain_id
        .parse()
        .map_err(|e| Error::Cosmos(format!("invalid --chain-id {:?}: {e}", params.chain_id)))?;
    let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, params.account_number)
        .map_err(|e| Error::Cosmos(format!("failed to build sign doc: {e}")))?;

    let tx_raw = sign_doc
        .sign(&key)
        .map_err(|e| Error::Cosmos(format!("signing failed: {e}")))?;
    let raw_bytes = tx_raw
        .to_bytes()
        .map_err(|e| Error::Cosmos(format!("failed to serialize transaction: {e}")))?;

    let signature = {
        let mut proto: cosmrs::proto::cosmos::tx::v1beta1::TxRaw = tx_raw.into();
        proto
            .signatures
            .pop()
            .ok_or_else(|| Error::Cosmos("signed transaction has no signatures".into()))?
    };

    seed.zeroize();

    Ok(SignedCosmosTx {
        from,
        raw_bytes,
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Foundry's well-known test private key, reused as a fixed seed.
    /// Never use outside tests.
    const TEST_SEED: [u8; 32] = [
        0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38, 0xff,
        0x94, 0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2,
        0xff, 0x80,
    ];

    fn params() -> CosmosParams {
        CosmosParams {
            chain_id: "cosmoshub-4".into(),
            account_number: 1,
            sequence: 0,
            to: parse_address("cosmos19dyl0uyzes4k23lscla02n06fc22h4uqsdwq6z").expect("to"),
            amount: 1_000_000,
            denom: "uatom".into(),
            memo: String::new(),
            timeout_height: 0,
            gas: 100_000,
            fee_amount: 5_000,
            fee_denom: String::new(),
        }
    }

    #[test]
    fn derives_sender_address_from_key_and_hrp() {
        let from = derive_address(&TEST_SEED, "cosmos").expect("derive");
        assert_eq!(
            from.to_string(),
            "cosmos15428vq2uzwhm3taey9sr9x5vm6tk78ewe54lwe"
        );
        assert_eq!(from.prefix(), "cosmos");
    }

    #[test]
    fn sign_transaction_produces_verifiable_bytes() {
        let signed = sign_transaction(TEST_SEED, params()).expect("sign");
        assert_eq!(
            signed.from().to_string(),
            "cosmos15428vq2uzwhm3taey9sr9x5vm6tk78ewe54lwe"
        );
        assert_eq!(signed.signature().len(), 64);
        assert!(!signed.raw().is_empty());

        // The raw bytes are a valid TxRaw protobuf and parse back.
        let tx = cosmrs::tx::Tx::from_bytes(signed.raw()).expect("parses");
        assert_eq!(tx.body.memo, "");
        assert_eq!(tx.auth_info.fee.gas_limit, 100_000);
        assert_eq!(tx.auth_info.signer_infos.len(), 1);
        assert_eq!(tx.auth_info.signer_infos[0].sequence, 0);
    }

    #[test]
    fn signing_is_deterministic_for_fixed_params() {
        let a = sign_transaction(TEST_SEED, params()).expect("sign a");
        let b = sign_transaction(TEST_SEED, params()).expect("sign b");
        assert_eq!(a.signature(), b.signature());
        assert_eq!(a.raw(), b.raw());
    }

    #[test]
    fn different_sequence_changes_signature() {
        let mut p = params();
        p.sequence += 1;
        let a = sign_transaction(TEST_SEED, params()).expect("sign a");
        let b = sign_transaction(TEST_SEED, p).expect("sign b");
        assert_ne!(a.signature(), b.signature());
    }

    #[test]
    fn rejects_zero_amount() {
        let mut p = params();
        p.amount = 0;
        assert!(matches!(
            sign_transaction(TEST_SEED, p),
            Err(Error::Cosmos(ref msg)) if msg.contains("--amount")
        ));
    }
}
