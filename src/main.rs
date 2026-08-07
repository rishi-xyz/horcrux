use clap::{Parser, Subcommand};
use horcrux::error::Error;
use horcrux::tx::{TxParams, derive_address};
use horcrux::{init_shards, reconstruct};
use k256::SecretKey;
use rand::rngs::OsRng;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "horcrux",
    version,
    about = "Split, encrypt, and reconstruct a private key via Shamir's Secret Sharing; sign Solana transactions."
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
    /// Sign a Solana transaction offline with a key reconstructed from shards,
    /// optionally broadcasting to a cluster.
    Sign {
        /// Paths of the shard files to combine.
        #[arg(required = true)]
        shards: Vec<PathBuf>,
        /// Use this password for every shard (else prompt per shard).
        #[arg(long)]
        password: Option<String>,
        /// Recipient address (base58).
        #[arg(long)]
        to: String,
        /// Amount to send, in lamports (1 SOL = 1_000_000_000 lamports).
        #[arg(long)]
        lamports: u64,
        /// Recent blockhash (base58). Required offline; fetched from the
        /// cluster when broadcasting.
        #[arg(long)]
        blockhash: Option<String>,
        /// Solana JSON-RPC endpoint (overrides $HORCRUX_RPC_URL).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Broadcast the signed transaction and wait for confirmation.
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
            lamports,
            blockhash,
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

            let to: solana_pubkey::Pubkey = to
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;

            let rpc_url = rpc_url.unwrap_or_else(horcrux::chain::default_rpc_url);
            let chain = if broadcast {
                println!("Broadcasting via {rpc_url}");
                Some(horcrux::chain::Chain::connect(&rpc_url))
            } else {
                None
            };

            let blockhash: solana_hash::Hash = match blockhash {
                Some(bh) => bh
                    .parse()
                    .map_err(|e| anyhow::anyhow!("invalid --blockhash: {e}"))?,
                None => {
                    let chain = chain.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "offline signing requires --blockhash; \
                             use --broadcast to fetch it from the cluster"
                        )
                    })?;
                    let blockhash = chain.latest_blockhash().await?;
                    println!("Resolved: latest blockhash {blockhash}");
                    blockhash
                }
            };

            let key = reconstruct(&shards, &passwords)?;
            let seed = horcrux::key_seed(&key);
            let from = derive_address(&seed);

            if let Some(chain) = &chain {
                let balance = chain.balance(&from).await?;
                println!("Balance:  {balance} lamports");
                if balance == 0 {
                    anyhow::bail!(
                        "sender {from} is unfunded; airdrop lamports first \
                         (localnet/devnet: `solana airdrop 1 {from}`)"
                    );
                }
            }

            let params = TxParams {
                from,
                to,
                lamports,
                blockhash,
            };
            let seed_bytes: [u8; 32] = *seed;
            let signed = horcrux::tx::sign_transaction(seed_bytes, params)?;
            println!("From:      {}", signed.from());
            println!("Signature: {}", signed.signature());
            println!("Raw:       {}", signed.raw_base58());

            if let Some(chain) = chain {
                let signature = horcrux::chain::broadcast(
                    chain.client(),
                    signed.tx(),
                    std::time::Duration::from_secs(1),
                    60,
                )
                .await?;
                println!("Mined:     {signature} (confirmed)");
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
