//! EVM RPC access for the optional broadcast step of Mode A signing.
//!
//! Signing always happens offline in [`crate::tx`]. This module only fills in
//! fields a caller chose to omit (nonce, gas limit, fees, chain id) from a
//! node, and submits a signed raw transaction to the network. Every function is
//! written against the [`Provider`] trait so it can be exercised against the
//! alloy mock transport in tests.

use crate::error::Error;
use crate::tx::Fee;
use alloy::network::{Ethereum, TransactionBuilder};
use alloy::primitives::{Address, B256, Bytes, ChainId, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionReceipt;
use alloy::rpc::types::request::TransactionRequest;
use std::time::Duration;

/// Default Sepolia public RPC endpoint.
pub const DEFAULT_RPC_URL: &str = "https://rpc.sepolia.org";

/// RPC endpoint to use: the `HORCRUX_RPC_URL` environment variable if set,
/// otherwise [`DEFAULT_RPC_URL`].
pub fn default_rpc_url() -> String {
    std::env::var("HORCRUX_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string())
}

/// Connect to an EVM JSON-RPC endpoint over HTTPS/HTTP.
pub async fn http_provider(rpc_url: &str) -> Result<impl Provider<Ethereum>, Error> {
    ProviderBuilder::new()
        .connect(rpc_url)
        .await
        .map_err(|e| Error::Tx(format!("could not connect to RPC at {rpc_url}: {e}")))
}

/// The transaction fields resolved from a node, for broadcast signing.
#[derive(Debug, Clone)]
pub struct Populated {
    /// Chain id, used for EIP-155 replay protection.
    pub chain_id: ChainId,
    /// Sender nonce.
    pub nonce: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Fee model.
    pub fee: Fee,
}

/// Fields the caller may leave unset; each `None` is resolved from the node.
#[derive(Debug, Clone, Default)]
pub struct FieldHints {
    /// Chain id, used for EIP-155 replay protection.
    pub chain_id: Option<ChainId>,
    /// Sender nonce.
    pub nonce: Option<u64>,
    /// Gas limit.
    pub gas_limit: Option<u64>,
    /// Fee model.
    pub fee: Option<Fee>,
}

/// Fill in any transaction fields the caller did not supply by querying the
/// node. Fields that are already provided are left untouched.
pub async fn populate<P>(
    provider: &P,
    from: Address,
    to: Address,
    value: U256,
    data: Bytes,
    hints: FieldHints,
) -> Result<Populated, Error>
where
    P: Provider<Ethereum>,
{
    let FieldHints {
        chain_id,
        nonce,
        gas_limit,
        fee,
    } = hints;
    let chain_id = match chain_id {
        Some(chain_id) => chain_id,
        None => provider.get_chain_id().await.map_err(rpc_err)?,
    };
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => provider
            .get_transaction_count(from)
            .await
            .map_err(rpc_err)?,
    };
    let gas_limit = match gas_limit {
        Some(gas_limit) => gas_limit,
        None => {
            let request = TransactionRequest::default()
                .from(from)
                .to(to)
                .with_value(value)
                .with_input(data);
            provider.estimate_gas(request).await.map_err(rpc_err)?
        }
    };
    let fee = match fee {
        Some(fee) => fee,
        None => {
            let estimation = provider.estimate_eip1559_fees().await.map_err(rpc_err)?;
            Fee::Eip1559 {
                max_fee_per_gas: estimation.max_fee_per_gas,
                max_priority_fee_per_gas: estimation.max_priority_fee_per_gas,
            }
        }
    };
    Ok(Populated {
        chain_id,
        nonce,
        gas_limit,
        fee,
    })
}

/// Broadcast a signed raw transaction and wait until it is mined, returning its
/// hash and receipt.
///
/// `raw` is the EIP-2718 encoding of a signed transaction (see
/// [`crate::tx::SignedTx::encoded`]).
pub async fn broadcast(
    provider: &impl Provider<Ethereum>,
    raw: &[u8],
    poll_interval: Duration,
    max_polls: usize,
) -> Result<(B256, TransactionReceipt), Error> {
    let pending = provider.send_raw_transaction(raw).await.map_err(rpc_err)?;
    let tx_hash = *pending.tx_hash();

    for _ in 0..max_polls {
        if let Some(receipt) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(rpc_err)?
        {
            return Ok((tx_hash, receipt));
        }
        tokio::time::sleep(poll_interval).await;
    }
    Err(Error::Tx(format!(
        "receipt for {tx_hash:#x} not found after {max_polls} polls"
    )))
}

/// Map a transport error onto the crate error type.
fn rpc_err(e: alloy::transports::TransportError) -> Error {
    Error::Tx(format!("RPC error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::mock::Asserter;

    const FROM: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    const TO: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

    fn fee_history() -> alloy::rpc::types::FeeHistory {
        alloy::rpc::types::FeeHistory {
            base_fee_per_gas: (1u128..=11).collect(),
            gas_used_ratio: vec![0.5; 10],
            base_fee_per_blob_gas: Vec::new(),
            blob_gas_used_ratio: Vec::new(),
            oldest_block: 1000,
            reward: Some(vec![vec![10_000_000]; 10]),
        }
    }

    #[tokio::test]
    async fn populate_fills_all_missing_fields_from_node() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

        asserter.push_success(&11155111u64); // eth_chainId
        asserter.push_success(&5u64); // eth_getTransactionCount
        asserter.push_success(&21_000u64); // eth_estimateGas
        asserter.push_success(&fee_history()); // eth_feeHistory

        let from: Address = FROM.parse().expect("from");
        let to: Address = TO.parse().expect("to");
        let populated = populate(
            &provider,
            from,
            to,
            U256::from(1),
            Bytes::new(),
            FieldHints::default(),
        )
        .await
        .expect("populate");

        assert_eq!(populated.chain_id, 11155111);
        assert_eq!(populated.nonce, 5);
        assert_eq!(populated.gas_limit, 21_000);
        assert_eq!(
            populated.fee,
            Fee::Eip1559 {
                max_fee_per_gas: 10_000_020,
                max_priority_fee_per_gas: 10_000_000,
            }
        );
    }

    #[tokio::test]
    async fn populate_preserves_explicit_values_without_rpc_calls() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

        // No responses pushed: explicit values mean no RPC calls are made.
        let from: Address = FROM.parse().expect("from");
        let to: Address = TO.parse().expect("to");
        let hints = FieldHints {
            chain_id: Some(1),
            nonce: Some(2),
            gas_limit: Some(21_000),
            fee: Some(Fee::Legacy { gas_price: 7 }),
        };
        let populated = populate(&provider, from, to, U256::from(1), Bytes::new(), hints)
            .await
            .expect("populate");

        assert_eq!(populated.chain_id, 1);
        assert_eq!(populated.nonce, 2);
        assert_eq!(populated.gas_limit, 21_000);
        assert_eq!(populated.fee, Fee::Legacy { gas_price: 7 });
    }

    #[tokio::test]
    async fn broadcast_returns_hash_and_mined_receipt() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

        let tx_hash: B256 = "0x9db8dc5f4365292ea2743ae8570217ba3d09330835597b767c5b7e78e21127bc"
            .parse()
            .expect("hash");
        let receipt = receipt_json(&tx_hash);

        asserter.push_success(&tx_hash); // eth_sendRawTransaction
        asserter.push_success(&None::<TransactionReceipt>); // first poll: not mined yet
        asserter.push_success(&Some(receipt)); // second poll: mined

        let (hash, mined) = broadcast(&provider, &[0x02, 0x00], Duration::from_millis(1), 5)
            .await
            .expect("broadcast");

        assert_eq!(hash, tx_hash);
        assert_eq!(mined.transaction_hash, tx_hash);
        assert_eq!(mined.block_number, Some(1));
    }

    #[tokio::test]
    async fn broadcast_times_out_when_receipt_never_appears() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

        let tx_hash: B256 = "0x9db8dc5f4365292ea2743ae8570217ba3d09330835597b767c5b7e78e21127bc"
            .parse()
            .expect("hash");

        asserter.push_success(&tx_hash); // eth_sendRawTransaction
        asserter.push_success(&None::<TransactionReceipt>); // poll 1
        asserter.push_success(&None::<TransactionReceipt>); // poll 2

        let result = broadcast(&provider, &[0x02, 0x00], Duration::from_millis(1), 2)
            .await
            .unwrap_err();
        assert!(matches!(result, Error::Tx(msg) if msg.contains("not found")));
    }

    fn receipt_json(tx_hash: &B256) -> TransactionReceipt {
        serde_json::from_value(serde_json::json!({
            "transactionHash": tx_hash,
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "blockNumber": "0x1",
            "gasUsed": "0x5208",
            "effectiveGasPrice": "0x4a817c800",
            "from": FROM,
            "to": TO,
            "contractAddress": null,
            "cumulativeGasUsed": "0x5208",
            "logs": [],
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "status": "0x1",
            "type": "0x2"
        }))
        .expect("valid receipt json")
    }
}
