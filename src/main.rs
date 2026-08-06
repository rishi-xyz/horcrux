use clap::{Parser, Subcommand};
use horcrux::error::Error;
use horcrux::tx::{DEFAULT_CHAIN_ID, Fee, TxParams};
use horcrux::{init_shards, reconstruct};
use k256::SecretKey;
use rand::rngs::OsRng;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "horcrux",
    version,
    about = "Split, encrypt, and reconstruct a secp256k1 private key via Shamir's Secret Sharing."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)] // clap subcommand variants are inherently large
enum Command {
    /// Split a private key into encrypted shard files.
    Init {
        /// Shares required to reconstruct the key.
        #[arg(long, default_value_t = 2)]
        threshold: u8,
        /// Total number of shares to create.
        #[arg(long, default_value_t = 3)]
        shares: u8,
        /// The private key as hex (64 hex chars, optional 0x prefix).
        #[arg(long, conflicts_with = "generate")]
        key_hex: Option<String>,
        /// Generate a random disposable test key and print it once.
        #[arg(long)]
        generate: bool,
        /// Directory to write shard files into.
        #[arg(long, default_value = "shards")]
        out_dir: PathBuf,
        /// Use this password for every shard (else prompt per shard).
        #[arg(long)]
        password: Option<String>,
    },
    /// Reconstruct a private key from shard files.
    Reconstruct {
        /// Paths of the shard files to combine.
        #[arg(required = true)]
        shards: Vec<PathBuf>,
        /// Use this password for every shard (else prompt per shard).
        #[arg(long)]
        password: Option<String>,
    },
    /// Sign an EVM transaction offline with a key reconstructed from shards,
    /// optionally filling missing fields from and broadcasting to an EVM RPC.
    Sign {
        /// Paths of the shard files to combine.
        #[arg(required = true)]
        shards: Vec<PathBuf>,
        /// Use this password for every shard (else prompt per shard).
        #[arg(long)]
        password: Option<String>,
        /// Recipient address (0x-prefixed).
        #[arg(long)]
        to: String,
        /// Value to send, in wei.
        #[arg(long)]
        value: String,
        /// Calldata as hex (0x-prefixed).
        #[arg(long, default_value = "0x")]
        data: String,
        /// Chain id for EIP-155 replay protection (defaults to 11155111
        /// offline, or the node's chain id when broadcasting).
        #[arg(long)]
        chain_id: Option<u64>,
        /// Account nonce of the sender. Required offline; fetched from the RPC
        /// when broadcasting.
        #[arg(long)]
        nonce: Option<u64>,
        /// Gas limit. Required offline; estimated via the RPC when
        /// broadcasting.
        #[arg(long)]
        gas: Option<u64>,
        /// Legacy gas price in wei (makes the transaction legacy-typed).
        #[arg(long, conflicts_with_all = ["max_fee_per_gas", "max_priority_fee_per_gas"])]
        gas_price: Option<u128>,
        /// EIP-1559 max fee per gas in wei. Required offline; estimated when
        /// broadcasting.
        #[arg(long)]
        max_fee_per_gas: Option<u128>,
        /// EIP-1559 max priority fee per gas in wei.
        #[arg(long, requires = "max_fee_per_gas")]
        max_priority_fee_per_gas: Option<u128>,
        /// EVM JSON-RPC endpoint (overrides $HORCRUX_RPC_URL).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Broadcast the signed transaction and wait for its receipt.
        #[arg(long)]
        broadcast: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            threshold,
            shares,
            key_hex,
            generate,
            out_dir,
            password,
        } => {
            let key = match (&key_hex, generate) {
                (Some(hex_key), _) => parse_key(hex_key)?,
                (None, true) => {
                    let key = SecretKey::random(&mut OsRng);
                    println!("Generated test key: 0x{}", hex::encode(key.to_bytes()));
                    key
                }
                (None, false) => {
                    anyhow::bail!("provide either --key-hex or --generate");
                }
            };

            let passwords = collect_passwords(
                shares as usize,
                password,
                &|i, n| format!("Password for shard {} of {n}: ", i + 1),
                true,
            )?;

            let paths = init_shards(&key, threshold, shares, &out_dir, &passwords)?;
            println!(
                "Wrote {n} shards to {dir} (threshold {t}):",
                n = paths.len(),
                dir = out_dir.display(),
                t = threshold
            );
            for path in &paths {
                println!("  {}", path.display());
            }
        }
        Command::Reconstruct { shards, password } => {
            let passwords = collect_passwords(
                shards.len(),
                password,
                &|i, _| {
                    let name = shards[i]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Password for {name}: ")
                },
                false,
            )?;

            let key = reconstruct(&shards, &passwords)?;
            println!("Reconstructed key: 0x{}", hex::encode(key.to_bytes()));
        }
        Command::Sign {
            shards,
            password,
            to,
            value,
            data,
            chain_id,
            nonce,
            gas,
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            rpc_url,
            broadcast,
        } => {
            let passwords = collect_passwords(
                shards.len(),
                password,
                &|i, _| {
                    let name = shards[i]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Password for {name}: ")
                },
                false,
            )?;

            let to: alloy::primitives::Address = to
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;
            let value: alloy::primitives::U256 = value
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --value: {e}"))?;
            let data = parse_data(&data)?;

            let explicit_fee = match (gas_price, max_fee_per_gas, max_priority_fee_per_gas) {
                (Some(gas_price), None, None) => Some(Fee::Legacy { gas_price }),
                (None, Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
                    Some(Fee::Eip1559 {
                        max_fee_per_gas,
                        max_priority_fee_per_gas,
                    })
                }
                (None, None, None) => None,
                _ => unreachable!("clap conflicts prevent a mixed fee model"),
            };

            let rpc_url = rpc_url.unwrap_or_else(horcrux::chain::default_rpc_url);
            let key = reconstruct(&shards, &passwords)?;
            let from = horcrux::tx::derive_address(&key);

            let params;
            let provider;
            if broadcast {
                let connected = horcrux::chain::http_provider(&rpc_url).await?;
                println!("Broadcasting via {rpc_url}");
                let populated = horcrux::chain::populate(
                    &connected,
                    from,
                    to,
                    value,
                    data.clone(),
                    horcrux::chain::FieldHints {
                        chain_id,
                        nonce,
                        gas_limit: gas,
                        fee: explicit_fee,
                    },
                )
                .await?;
                println!(
                    "Resolved: chain {} · nonce {} · gas {} · {}",
                    populated.chain_id,
                    populated.nonce,
                    populated.gas_limit,
                    match populated.fee {
                        Fee::Legacy { gas_price } => format!("legacy {gas_price} wei/gas"),
                        Fee::Eip1559 {
                            max_fee_per_gas,
                            max_priority_fee_per_gas,
                        } => format!(
                            "EIP-1559 max {max_fee_per_gas} + tip {max_priority_fee_per_gas} wei/gas"
                        ),
                    },
                );
                params = TxParams {
                    to,
                    value,
                    data,
                    chain_id: populated.chain_id,
                    nonce: populated.nonce,
                    gas_limit: populated.gas_limit,
                    fee: populated.fee,
                };
                provider = Some(connected);
            } else {
                let chain_id = chain_id.unwrap_or(DEFAULT_CHAIN_ID);
                let nonce = nonce.ok_or_else(|| {
                    anyhow::anyhow!(
                        "offline signing requires --nonce; use --broadcast to fetch it from the RPC"
                    )
                })?;
                let gas_limit = gas.ok_or_else(|| {
                    anyhow::anyhow!(
                        "offline signing requires --gas; use --broadcast to estimate it from the RPC"
                    )
                })?;
                let fee = explicit_fee.ok_or_else(|| {
                    anyhow::anyhow!(
                        "offline signing requires a fee model \
                         (--gas-price, or --max-fee-per-gas with --max-priority-fee-per-gas); \
                         use --broadcast to estimate one"
                    )
                })?;
                params = TxParams {
                    to,
                    value,
                    data,
                    chain_id,
                    nonce,
                    gas_limit,
                    fee,
                };
                provider = None;
            }

            let signed = horcrux::tx::sign_transaction(key, params)?;
            println!("From:    {:#x}", signed.from());
            println!("Tx hash: {:#x}", signed.tx_hash());
            println!("Raw:     {}", signed.raw_hex());

            if let Some(provider) = provider {
                let (tx_hash, receipt) = horcrux::chain::broadcast(
                    &provider,
                    &signed.encoded(),
                    std::time::Duration::from_secs(2),
                    60,
                )
                .await?;
                let status = if receipt.inner.status() {
                    "success"
                } else {
                    "reverted"
                };
                println!("Mined:   {tx_hash:#x} ({status})");
            }
        }
    }
    Ok(())
}

/// Parse a hex-encoded secp256k1 private key (32 bytes), tolerating a 0x
/// prefix.
fn parse_key(hex_key: &str) -> Result<SecretKey, Error> {
    let stripped = hex_key.strip_prefix("0x").unwrap_or(hex_key);
    let bytes =
        hex::decode(stripped).map_err(|e| Error::InvalidKey(format!("not valid hex: {e}")))?;
    if bytes.len() != 32 {
        return Err(Error::InvalidKey(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    SecretKey::from_slice(&bytes).map_err(|e| Error::InvalidKey(e.to_string()))
}

/// Parse a `0x`-prefixed hex calldata string.
fn parse_data(hex_data: &str) -> anyhow::Result<alloy::primitives::Bytes> {
    let bytes = alloy::primitives::hex::decode(hex_data)
        .map_err(|e| anyhow::anyhow!("invalid --data hex: {e}"))?;
    Ok(alloy::primitives::Bytes::from(bytes))
}

/// Gather guardian passwords: either a single shared `--password`, or an
/// interactive prompt per shard (with confirmation when `confirm` is set).
fn collect_passwords(
    count: usize,
    shared: Option<String>,
    prompt: &dyn Fn(usize, usize) -> String,
    confirm: bool,
) -> anyhow::Result<Vec<String>> {
    if let Some(pw) = shared {
        return Ok(vec![pw; count]);
    }
    let mut passwords = Vec::with_capacity(count);
    for i in 0..count {
        let pw = rpassword::prompt_password(prompt(i, count))?;
        if confirm {
            let again = rpassword::prompt_password("  Confirm password: ")?;
            if pw != again {
                anyhow::bail!("passwords do not match");
            }
        }
        passwords.push(pw);
    }
    Ok(passwords)
}
