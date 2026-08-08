//! Solana RPC access for the optional broadcast step of Mode A signing.
//!
//! Signing always happens offline in [`crate::tx`]. This module only resolves a
//! recent blockhash when the caller did not supply one, and submits a signed
//! transaction to the cluster. Tests run against the crate's mock transport
//! ([`solana_rpc_client::mock_sender`]) so no node is required.

use crate::error::Error;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_signature::Signature;
use solana_transaction::Transaction;
use std::time::Duration;

/// Default local validator RPC endpoint (`solana-test-validator`).
pub const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8899";

/// RPC endpoint to use: the `HORCRUX_RPC_URL` environment variable if set,
/// otherwise [`DEFAULT_RPC_URL`].
pub fn default_rpc_url() -> String {
    std::env::var("HORCRUX_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string())
}

/// A connection to a Solana cluster.
pub struct Chain {
    client: RpcClient,
}

impl Chain {
    /// Connect to a cluster over HTTP.
    ///
    /// Uses the `confirmed` commitment so that balances and blockhashes are
    /// read after gossip confirmation rather than only after finalization
    /// (which can lag on `solana-test-validator`).
    pub fn connect(rpc_url: &str) -> Self {
        Self {
            client: RpcClient::new_with_commitment(
                rpc_url.to_string(),
                CommitmentConfig::confirmed(),
            ),
        }
    }

    /// The underlying RPC client.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }

    /// Fetch the most recent blockhash from the cluster.
    pub async fn latest_blockhash(&self) -> Result<Hash, Error> {
        self.client.get_latest_blockhash().await.map_err(rpc_err)
    }

    /// Current balance of `address` in lamports.
    pub async fn balance(&self, address: &Pubkey) -> Result<u64, Error> {
        self.client.get_balance(address).await.map_err(rpc_err)
    }
}

/// Broadcast a signed transaction and wait until it is confirmed, returning its
/// signature (the Solana transaction id).
pub async fn broadcast(
    client: &RpcClient,
    tx: &Transaction,
    poll_interval: Duration,
    max_polls: usize,
) -> Result<Signature, Error> {
    let signature = client.send_transaction(tx).await.map_err(rpc_err)?;
    for _ in 0..max_polls {
        if client
            .confirm_transaction(&signature)
            .await
            .map_err(rpc_err)?
        {
            return Ok(signature);
        }
        tokio::time::sleep(poll_interval).await;
    }
    Err(Error::Tx(format!(
        "transaction {signature} not confirmed after {max_polls} polls"
    )))
}

/// Map any RPC error onto the crate error type without naming the concrete
/// client error type.
fn rpc_err(e: impl std::fmt::Display) -> Error {
    Error::Tx(format!("RPC error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_rpc_client::mock_sender::MocksMap;
    use solana_rpc_client_api::request::RpcRequest;
    use std::collections::HashMap;

    fn mock_client(mocks: MocksMap) -> RpcClient {
        RpcClient::new_mock_with_mocks_map("mock".to_string(), mocks)
    }

    #[test]
    fn default_rpc_url_uses_env_override() {
        // SAFETY: single-threaded test; no other test reads this variable.
        unsafe { std::env::set_var("HORCRUX_RPC_URL", "http://example.test:9999") };
        assert_eq!(default_rpc_url(), "http://example.test:9999");
        unsafe { std::env::remove_var("HORCRUX_RPC_URL") };
        assert_eq!(default_rpc_url(), DEFAULT_RPC_URL);
    }

    #[tokio::test]
    async fn latest_blockhash_reads_from_mock() {
        let expected = Hash::new_from_array([0x42; 32]);
        let response = serde_json::json!({
            "context": { "slot": 1, "apiVersion": "2.0.3" },
            "value": { "blockhash": expected.to_string(), "lastValidBlockHeight": 1 },
        });
        let mocks = MocksMap(HashMap::from([(
            RpcRequest::GetLatestBlockhash,
            std::collections::VecDeque::from([response]),
        )]));
        let chain = Chain {
            client: mock_client(mocks),
        };
        assert_eq!(chain.latest_blockhash().await.expect("blockhash"), expected);
    }

    #[tokio::test]
    async fn broadcast_times_out_when_never_confirmed() {
        let tx = crate::tx::sign_transaction(
            [0x5a; 32],
            crate::tx::TxParams {
                from: crate::tx::derive_address(&[0x5a; 32]),
                to: crate::tx::derive_address(&[0x1b; 32]),
                lamports: 1_000_000,
                blockhash: Hash::new_from_array([0x42; 32]),
            },
        )
        .expect("sign")
        .tx()
        .clone();

        let statuses = serde_json::json!({
            "context": { "slot": 1, "apiVersion": "2.0.3" },
            "value": [null],
        });
        let mocks = MocksMap(HashMap::from([
            (
                RpcRequest::SendTransaction,
                std::collections::VecDeque::from([serde_json::json!(tx.signatures[0].to_string())]),
            ),
            (
                RpcRequest::GetSignatureStatuses,
                std::collections::VecDeque::from([statuses.clone(), statuses]),
            ),
        ]));
        let client = mock_client(mocks);

        let result = broadcast(&client, &tx, Duration::from_millis(1), 2)
            .await
            .expect_err("must time out");
        assert!(
            matches!(result, Error::Tx(ref msg) if msg.contains("not confirmed")),
            "got: {result:?}"
        );
    }

    #[tokio::test]
    async fn rpc_errors_are_mapped() {
        let mocks = MocksMap(HashMap::from([(
            RpcRequest::GetLatestBlockhash,
            std::collections::VecDeque::from([serde_json::json!({
                "code": -32601,
                "message": "Method not found: getLatestBlockhash",
            })]),
        )]));
        let client = mock_client(mocks);
        let err = client
            .get_latest_blockhash()
            .await
            .map_err(|e| Error::Tx(format!("RPC error: {e}")))
            .expect_err("mock returns an RPC error");
        assert!(err.to_string().contains("RPC error"), "got: {err}");
    }
}
