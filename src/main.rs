mod tui;
mod web;

use clap::{Parser, Subcommand, ValueEnum};
use horcrux::error::Error;
use horcrux::tx::{TxParams, derive_address};
use horcrux::{init_shards, reconstruct_with_audit};
use k256::SecretKey;
use rand::rngs::OsRng;
use std::path::PathBuf;

/// The chain a transaction is built for.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ChainKind {
    /// Solana (Ed25519) — the original chain.
    Solana,
    /// Bitcoin (Taproot/BIP340-342).
    Bitcoin,
    /// Cosmos SDK (bank.MsgSend, SIGN_MODE_DIRECT).
    Cosmos,
}

/// How a `qr-*` command moves a message across the air gap.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TransportArg {
    /// One scannable QR-code PNG per frame.
    Qr,
    /// A single `.hx3` frame-collection file.
    Hx3,
}

impl From<TransportArg> for horcrux::qr::Transport {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Qr => horcrux::qr::Transport::Qr,
            TransportArg::Hx3 => horcrux::qr::Transport::Hx3,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "horcrux",
    version,
    about = "Split, encrypt, and reconstruct a private key via Shamir's Secret Sharing; sign Solana, Bitcoin, and Cosmos transactions."
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
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
    },
    /// Sign a transaction offline with a key reconstructed from shards,
    /// optionally broadcasting to a Solana cluster. Choose the chain with
    /// `--chain`; the default (`solana`) preserves the original behavior.
    Sign {
        /// Paths of the shard files to combine.
        #[arg(required = true)]
        shards: Vec<PathBuf>,
        /// Chain to build the transaction for.
        #[arg(long, value_enum, default_value_t = ChainKind::Solana)]
        chain: ChainKind,
        /// Use this password for every shard (else prompt per shard).
        #[arg(long)]
        password: Option<String>,
        /// Recipient address (base58 Solana pubkey, or bech32 for Bitcoin and
        /// Cosmos).
        #[arg(long)]
        to: String,
        /// Amount to send, in lamports (1 SOL = 1_000_000_000 lamports).
        /// Solana only.
        #[arg(long)]
        lamports: Option<u64>,
        /// Recent blockhash (base58). Required offline; fetched from the
        /// cluster when broadcasting. Solana only.
        #[arg(long)]
        blockhash: Option<String>,
        /// RPC endpoint (overrides $HORCRUX_RPC_URL for Solana,
        /// $HORCRUX_BTC_RPC_URL for Bitcoin, $HORCRUX_COSMOS_RPC_URL for
        /// Cosmos).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Broadcast the signed transaction (Solana: wait for confirmation;
        /// Bitcoin: `sendrawtransaction`; Cosmos: `broadcast_tx_commit`).
        #[arg(long)]
        broadcast: bool,
        /// Bitcoin Core RPC username, if the node requires authentication.
        /// Bitcoin only.
        #[arg(long)]
        rpc_user: Option<String>,
        /// Bitcoin Core RPC password, if the node requires authentication.
        /// Bitcoin only.
        #[arg(long)]
        rpc_password: Option<String>,
        /// Spend a UTXO as `<txid>:<vout>:<amount-sat>`, repeating as needed.
        /// Bitcoin only.
        #[arg(long)]
        utxo: Vec<String>,
        /// Amount to send to the recipient, in satoshis. Bitcoin only.
        #[arg(long)]
        amount_sat: Option<u64>,
        /// Change destination (bech32, mainnet). Defaults to the sender's own
        /// derived P2TR address. Bitcoin only.
        #[arg(long)]
        change_address: Option<String>,
        /// Explicit miner fee, in satoshis. Bitcoin only.
        #[arg(long)]
        fee_sat: Option<u64>,
        /// Chain id (e.g. cosmoshub-4). Cosmos only.
        #[arg(long)]
        chain_id: Option<String>,
        /// On-chain account number. Cosmos only.
        #[arg(long)]
        account_number: Option<u64>,
        /// Account sequence (number of previously committed transactions).
        /// Cosmos only.
        #[arg(long)]
        sequence: Option<u64>,
        /// Amount to send, in base denomination units. Cosmos only.
        #[arg(long)]
        amount: Option<u128>,
        /// Denomination of --amount (e.g. uatom). Cosmos only.
        #[arg(long)]
        denom: Option<String>,
        /// Gas limit. Cosmos only.
        #[arg(long)]
        gas: Option<u64>,
        /// Fee amount, in base denomination units. Cosmos only.
        #[arg(long)]
        fee: Option<u128>,
        /// Denomination of --fee (defaults to --denom). Cosmos only.
        #[arg(long)]
        fee_denom: Option<String>,
        /// Transaction memo. Cosmos only.
        #[arg(long, default_value = "")]
        memo: String,
        /// Reject if the chain has advanced past this height (0 disables).
        /// Cosmos only.
        #[arg(long)]
        timeout_height: Option<u64>,
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
    },
    /// Dealer-split a key into encrypted FROST key shares (Mode B). Signing
    /// later combines a threshold subset of these shares without ever
    /// reconstructing the key. `--chain cosmos` is rejected: Cosmos uses
    /// plain ECDSA/secp256k1, which needs a different threshold protocol
    /// (GG18/GG20/CGGMP21) than FROST — see the Mode B section of the plan.
    MpcSplit {
        /// Chain to split the key for (selects the FROST curve/ciphersuite
        /// and share file naming). `cosmos` is not supported.
        #[arg(long, value_enum, default_value_t = ChainKind::Solana)]
        chain: ChainKind,
        /// Shares required to sign.
        #[arg(long, default_value_t = 2)]
        threshold: u8,
        /// Total number of key shares to create.
        #[arg(long, default_value_t = 3)]
        shares: u8,
        /// The private key as hex (64 hex chars, optional 0x prefix).
        #[arg(long, conflicts_with = "generate")]
        key_hex: Option<String>,
        /// Generate a random disposable test key and print it once.
        #[arg(long)]
        generate: bool,
        /// Directory to write share files and the group public key package
        /// into.
        #[arg(long, default_value = "mpc")]
        out_dir: PathBuf,
        /// Use this password for every share (else prompt per share).
        #[arg(long)]
        password: Option<String>,
    },
    /// Sign a transaction with a threshold FROST subset of key shares,
    /// optionally broadcasting (Mode B). Choose the chain with `--chain`;
    /// `cosmos` is not supported (see `mpc-split`).
    MpcSign {
        /// Paths of the FROST share files to combine.
        #[arg(required = true)]
        shares: Vec<PathBuf>,
        /// Chain to build the transaction for.
        #[arg(long, value_enum, default_value_t = ChainKind::Solana)]
        chain: ChainKind,
        /// Directory containing the group public key package.
        #[arg(long, default_value = "mpc")]
        group_dir: PathBuf,
        /// Use this password for every share (else prompt per share).
        #[arg(long)]
        password: Option<String>,
        /// Recipient address (base58 Solana pubkey, or bech32 for Bitcoin).
        #[arg(long)]
        to: String,
        /// Amount to send, in lamports (1 SOL = 1_000_000_000 lamports).
        /// Solana only.
        #[arg(long)]
        lamports: Option<u64>,
        /// Recent blockhash (base58). Required offline; fetched from the
        /// cluster when broadcasting. Solana only.
        #[arg(long)]
        blockhash: Option<String>,
        /// RPC endpoint (overrides $HORCRUX_RPC_URL for Solana,
        /// $HORCRUX_BTC_RPC_URL for Bitcoin).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Broadcast the signed transaction (Solana: wait for confirmation;
        /// Bitcoin: `sendrawtransaction`).
        #[arg(long)]
        broadcast: bool,
        /// Bitcoin Core RPC username, if the node requires authentication.
        /// Bitcoin only.
        #[arg(long)]
        rpc_user: Option<String>,
        /// Bitcoin Core RPC password, if the node requires authentication.
        /// Bitcoin only.
        #[arg(long)]
        rpc_password: Option<String>,
        /// Spend a UTXO as `<txid>:<vout>:<amount-sat>`, repeating as needed.
        /// Bitcoin only.
        #[arg(long)]
        utxo: Vec<String>,
        /// Amount to send to the recipient, in satoshis. Bitcoin only.
        #[arg(long)]
        amount_sat: Option<u64>,
        /// Change destination (bech32, mainnet). Defaults to the group's own
        /// P2TR address. Bitcoin only.
        #[arg(long)]
        change_address: Option<String>,
        /// Explicit miner fee, in satoshis. Bitcoin only.
        #[arg(long)]
        fee_sat: Option<u64>,
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
    },
    /// Show the access log.
    Log {
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Only print the last N entries.
        #[arg(long)]
        tail: Option<usize>,
        /// Print raw JSON-lines instead of a human-readable table.
        #[arg(long)]
        json: bool,
    },
    /// Air-gapped Mode B, step 1 (coordinator): build a Solana transfer and
    /// emit it as a signing request, in `--dir`, for participants to accept.
    /// Uses Solana FROST key shares from `horcrux mpc-split` (Ed25519).
    QrRequest {
        /// Directory containing the group public key package (group.pub).
        #[arg(long, default_value = "mpc")]
        group_dir: PathBuf,
        /// Recipient address (base58).
        #[arg(long)]
        to: String,
        /// Amount to send, in lamports (1 SOL = 1_000_000_000 lamports).
        #[arg(long)]
        lamports: u64,
        /// Recent blockhash (base58). The QR flow is fully offline, so this
        /// must be supplied (no cluster fetch).
        #[arg(long)]
        blockhash: String,
        /// Air-gap transport directory (stands in for the physical QR
        /// codes/files carried between machines).
        #[arg(long, default_value = "qr")]
        dir: PathBuf,
        /// Transport format: real QR-code PNGs, or `.hx3` files.
        #[arg(long, value_enum, default_value_t = TransportArg::Qr)]
        format: TransportArg,
    },
    /// Air-gapped Mode B, step 2 (participant): accept a signing request,
    /// decrypt this guardian's share, and emit a round-1 commitment.
    QrCommit {
        /// Path of this guardian's FROST share file.
        share: PathBuf,
        /// Guardian password for the share (else prompt).
        #[arg(long)]
        password: Option<String>,
        /// Air-gap transport directory (same one the coordinator used).
        #[arg(long, default_value = "qr")]
        dir: PathBuf,
        /// Where to keep this participant's round-1 nonces until `qr-share`
        /// runs (local device state; never written into `--dir`). Defaults
        /// to a file next to the share.
        #[arg(long)]
        nonce_file: Option<PathBuf>,
        /// Transport format for the emitted commitment.
        #[arg(long, value_enum, default_value_t = TransportArg::Qr)]
        format: TransportArg,
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
    },
    /// Air-gapped Mode B, step 3 (coordinator): assemble the signing package
    /// from the commitments collected so far in `--dir`.
    QrPackage {
        /// Air-gap transport directory.
        #[arg(long, default_value = "qr")]
        dir: PathBuf,
        /// Transport format for the emitted signing package.
        #[arg(long, value_enum, default_value_t = TransportArg::Qr)]
        format: TransportArg,
    },
    /// Air-gapped Mode B, step 4 (participant): accept the signing package
    /// and emit this guardian's signature share.
    QrShare {
        /// Path of this guardian's FROST share file (same as `qr-commit`).
        share: PathBuf,
        /// Guardian password for the share (else prompt).
        #[arg(long)]
        password: Option<String>,
        /// Air-gap transport directory.
        #[arg(long, default_value = "qr")]
        dir: PathBuf,
        /// Where this participant's round-1 nonces were kept by `qr-commit`.
        /// Deleted after producing the signature share.
        #[arg(long)]
        nonce_file: Option<PathBuf>,
        /// Transport format for the emitted signature share.
        #[arg(long, value_enum, default_value_t = TransportArg::Qr)]
        format: TransportArg,
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
    },
    /// Air-gapped Mode B, step 5 (coordinator): aggregate the collected
    /// signature shares into the final signed transaction, optionally
    /// broadcasting it.
    QrFinalize {
        /// Air-gap transport directory.
        #[arg(long, default_value = "qr")]
        dir: PathBuf,
        /// Solana JSON-RPC endpoint (overrides $HORCRUX_RPC_URL).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Broadcast the signed transaction and wait for confirmation.
        #[arg(long)]
        broadcast: bool,
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
    },
    /// Check shard/share files for structural integrity (magic, version,
    /// length, split consistency) and, with a password, the AES-GCM
    /// authentication tag. Read-only: never decrypts into the clear and never
    /// touches the access log.
    Verify {
        /// Paths of the shard or share files to check.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Optionally check each file's AES-GCM auth tag with this password.
        #[arg(long)]
        password: Option<String>,
    },
    /// Launch the interactive terminal UI.
    Tui,
    /// Launch a local, loopback-only web UI covering the same Solana MVP
    /// flows as the TUI (Access log, Verify, Init, Sign, MPC split/sign), for
    /// guardians who would rather use a browser. Binds 127.0.0.1 only and
    /// requires a per-run token printed at startup; nothing off this machine
    /// can reach it.
    Web {
        /// Port to listen on.
        #[arg(long, default_value_t = 7420)]
        port: u16,
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
        Command::MpcSplit {
            chain,
            threshold,
            shares,
            key_hex,
            generate,
            out_dir,
            password,
        } => {
            if chain == ChainKind::Cosmos {
                anyhow::bail!(
                    "Mode B (FROST) is not supported for Cosmos: it uses plain ECDSA/secp256k1, \
                     which needs a different threshold protocol (GG18/GG20/CGGMP21) than FROST, \
                     and no audited Rust crate for that exists yet"
                );
            }
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
                &|i, n| format!("Password for share {} of {n}: ", i + 1),
                true,
            )?;

            let (paths, group_path) = match chain {
                ChainKind::Solana => {
                    horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)?
                }
                ChainKind::Bitcoin => {
                    horcrux::btc_mpc::btc_mpc_split(&key, threshold, shares, &out_dir, &passwords)?
                }
                ChainKind::Cosmos => unreachable!("rejected above"),
            };
            println!(
                "Wrote {n} FROST key shares to {dir} (threshold {t}):",
                n = paths.len(),
                dir = out_dir.display(),
                t = threshold
            );
            for path in &paths {
                println!("  {}", path.display());
            }
            println!("Group public package: {}", group_path.display());
            println!(
                "Note: signing combines a threshold subset of these shares and never reconstructs the key."
            );
        }
        Command::Reconstruct {
            shards,
            password,
            log_file,
            force,
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

            let (access_log, attempt) =
                audit_preflight(horcrux::audit::shard_ids(&shards)?, log_file, force)?;
            let key = reconstruct_with_audit(&shards, &passwords, &access_log, attempt)?;
            println!("Reconstructed key: 0x{}", hex::encode(key.to_bytes()));
        }
        Command::Sign {
            shards,
            chain,
            password,
            to,
            lamports,
            blockhash,
            rpc_url,
            broadcast,
            rpc_user,
            rpc_password,
            utxo,
            amount_sat,
            change_address,
            fee_sat,
            chain_id,
            account_number,
            sequence,
            amount,
            denom,
            gas,
            fee,
            fee_denom,
            memo,
            timeout_height,
            log_file,
            force,
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

            match chain {
                ChainKind::Solana => {
                    let to: solana_pubkey::Pubkey = to
                        .parse()
                        .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;
                    let lamports = lamports.ok_or_else(|| {
                        anyhow::anyhow!("--lamports is required for the solana chain")
                    })?;

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

                    let (access_log, attempt) =
                        audit_preflight(horcrux::audit::shard_ids(&shards)?, log_file, force)?;
                    let key = reconstruct_with_audit(&shards, &passwords, &access_log, attempt)?;
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
                ChainKind::Bitcoin => {
                    let recipient = horcrux::bitcoin::parse_address(&to)?;
                    let utxos = utxo
                        .iter()
                        .map(|s| parse_utxo(s))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let amount_sat = amount_sat.ok_or_else(|| {
                        anyhow::anyhow!("--amount-sat is required for the bitcoin chain")
                    })?;
                    let fee_sat = fee_sat.ok_or_else(|| {
                        anyhow::anyhow!("--fee-sat is required for the bitcoin chain")
                    })?;
                    let change_address = change_address
                        .as_deref()
                        .map(horcrux::bitcoin::parse_address)
                        .transpose()?;

                    let (access_log, attempt) =
                        audit_preflight(horcrux::audit::shard_ids(&shards)?, log_file, force)?;
                    let key = reconstruct_with_audit(&shards, &passwords, &access_log, attempt)?;
                    let seed = horcrux::key_seed(&key);
                    let recipient_str = recipient.to_string();
                    let params = horcrux::bitcoin::BitcoinParams {
                        utxos,
                        recipient,
                        amount_sat,
                        change_address,
                        fee_sat,
                    };
                    let signed = horcrux::bitcoin::sign_transaction(*seed, params)?;
                    println!("From:      {}", signed.sender());
                    println!("Recipient: {recipient_str}");
                    println!("Txid:      {}", signed.txid());
                    println!("Fee:       {} sat", signed.fee_sat());
                    println!("Change:    {} sat", signed.change_sat());
                    println!("Signature: {}", hex::encode(signed.signature()));
                    println!("Raw:       {}", signed.raw_hex());

                    if broadcast {
                        let rpc_url = rpc_url.unwrap_or_else(horcrux::bitcoin::default_rpc_url);
                        println!("Broadcasting via {rpc_url}");
                        let txid = horcrux::bitcoin::broadcast(
                            &rpc_url,
                            rpc_user.as_deref(),
                            rpc_password.as_deref(),
                            signed.tx(),
                        )?;
                        println!("Mined:     {txid} (accepted by mempool)");
                    }
                }
                ChainKind::Cosmos => {
                    if rpc_user.is_some() || rpc_password.is_some() {
                        anyhow::bail!(
                            "--rpc-user/--rpc-password are only supported for the bitcoin chain"
                        );
                    }
                    let to = horcrux::cosmos::parse_address(&to)?;
                    let chain_id = chain_id.ok_or_else(|| {
                        anyhow::anyhow!("--chain-id is required for the cosmos chain")
                    })?;
                    let account_number = account_number.ok_or_else(|| {
                        anyhow::anyhow!("--account-number is required for the cosmos chain")
                    })?;
                    let sequence = sequence.ok_or_else(|| {
                        anyhow::anyhow!("--sequence is required for the cosmos chain")
                    })?;
                    let amount = amount.ok_or_else(|| {
                        anyhow::anyhow!("--amount is required for the cosmos chain")
                    })?;
                    let denom = denom.ok_or_else(|| {
                        anyhow::anyhow!("--denom is required for the cosmos chain")
                    })?;
                    let gas = gas
                        .ok_or_else(|| anyhow::anyhow!("--gas is required for the cosmos chain"))?;
                    let fee = fee
                        .ok_or_else(|| anyhow::anyhow!("--fee is required for the cosmos chain"))?;

                    let (access_log, attempt) =
                        audit_preflight(horcrux::audit::shard_ids(&shards)?, log_file, force)?;
                    let key = reconstruct_with_audit(&shards, &passwords, &access_log, attempt)?;
                    let seed = horcrux::key_seed(&key);
                    let params = horcrux::cosmos::CosmosParams {
                        chain_id,
                        account_number,
                        sequence,
                        to,
                        amount,
                        denom,
                        memo,
                        timeout_height: timeout_height.unwrap_or(0),
                        gas,
                        fee_amount: fee,
                        fee_denom: fee_denom.unwrap_or_default(),
                    };
                    let signed = horcrux::cosmos::sign_transaction(*seed, params)?;
                    println!("From:      {}", signed.from());
                    println!("Raw:       {}", signed.raw_hex());

                    if broadcast {
                        let rpc_url = rpc_url.unwrap_or_else(horcrux::cosmos::default_rpc_url);
                        println!("Broadcasting via {rpc_url}");
                        let tx_hash = horcrux::cosmos::broadcast(&rpc_url, &signed).await?;
                        println!("Mined:     {tx_hash} (committed)");
                    }
                }
            }
        }
        Command::MpcSign {
            shares,
            chain: chain_kind,
            group_dir,
            password,
            to,
            lamports,
            blockhash,
            rpc_url,
            broadcast,
            rpc_user,
            rpc_password,
            utxo,
            amount_sat,
            change_address,
            fee_sat,
            log_file,
            force,
        } => {
            if chain_kind == ChainKind::Cosmos {
                anyhow::bail!(
                    "Mode B (FROST) is not supported for Cosmos: it uses plain ECDSA/secp256k1, \
                     which needs a different threshold protocol (GG18/GG20/CGGMP21) than FROST, \
                     and no audited Rust crate for that exists yet"
                );
            }
            let passwords = collect_passwords(
                shares.len(),
                password,
                &|i, _| {
                    let name = shares[i]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Password for {name}: ")
                },
                false,
            )?;

            if chain_kind == ChainKind::Bitcoin {
                let recipient = horcrux::bitcoin::parse_address(&to)?;
                let utxos = utxo
                    .iter()
                    .map(|s| parse_utxo(s))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let amount_sat = amount_sat.ok_or_else(|| {
                    anyhow::anyhow!("--amount-sat is required for the bitcoin chain")
                })?;
                let fee_sat = fee_sat.ok_or_else(|| {
                    anyhow::anyhow!("--fee-sat is required for the bitcoin chain")
                })?;
                let change_address = change_address
                    .as_deref()
                    .map(horcrux::bitcoin::parse_address)
                    .transpose()?;

                let group_pub = group_dir.join(horcrux::btc_mpc::BTC_GROUP_PUB_FILENAME);
                let output_xonly = horcrux::btc_mpc::group_output_xonly(&group_pub)?;

                let (access_log, attempt) =
                    audit_preflight(horcrux::btc_mpc::shard_ids(&shares)?, log_file, force)?;

                let recipient_str = recipient.to_string();
                let params = horcrux::bitcoin::BitcoinParams {
                    utxos,
                    recipient,
                    amount_sat,
                    change_address,
                    fee_sat,
                };
                let sighash = horcrux::bitcoin::unsigned_sighash(&output_xonly, &params)?;
                let sig = horcrux::btc_mpc::btc_mpc_sign_with_audit(
                    &shares,
                    &passwords,
                    &group_pub,
                    &sighash,
                    &access_log,
                    attempt,
                )?;
                let signed = horcrux::bitcoin::assemble_signed_transaction(
                    sig.output_xonly,
                    params,
                    sig.signature,
                )?;
                println!("From:      {}", signed.sender());
                println!("Recipient: {recipient_str}");
                println!("Txid:      {}", signed.txid());
                println!("Fee:       {} sat", signed.fee_sat());
                println!("Change:    {} sat", signed.change_sat());
                println!("Signature: {}", hex::encode(signed.signature()));
                println!("Raw:       {}", signed.raw_hex());

                if broadcast {
                    let rpc_url = rpc_url.unwrap_or_else(horcrux::bitcoin::default_rpc_url);
                    println!("Broadcasting via {rpc_url}");
                    let txid = horcrux::bitcoin::broadcast(
                        &rpc_url,
                        rpc_user.as_deref(),
                        rpc_password.as_deref(),
                        signed.tx(),
                    )?;
                    println!("Mined:     {txid} (accepted by mempool)");
                }
                return Ok(());
            }

            let to: solana_pubkey::Pubkey = to
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;
            let lamports = lamports
                .ok_or_else(|| anyhow::anyhow!("--lamports is required for the solana chain"))?;

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

            let group_pub = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
            let verifying_key: [u8; 32] = horcrux::mpc::group_verifying_key(&group_pub)?;
            let from = solana_pubkey::Pubkey::from(verifying_key);

            let (access_log, attempt) =
                audit_preflight(horcrux::mpc::shard_ids(&shares)?, log_file, force)?;

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
            let message = horcrux::tx::transaction_message(&params);
            let message_bytes = message.serialize();
            let sig = horcrux::mpc::mpc_sign_with_audit(
                &shares,
                &passwords,
                &group_pub,
                &message_bytes,
                &access_log,
                attempt,
            )?;
            let signed = horcrux::tx::sign_transaction_with_signature(
                params,
                sig.signature,
                sig.verifying_key,
            )?;
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
        Command::Log {
            log_file,
            tail,
            json,
        } => {
            let log = horcrux::audit::AccessLog::open(access_log_path(log_file));
            let entries = match tail {
                Some(n) => log.tail(n)?,
                None => log.read_all()?,
            };
            if entries.is_empty() {
                println!("No access log entries at {}", log.path().display());
            }
            for e in &entries {
                if json {
                    println!("{}", serde_json::to_string(e)?);
                } else {
                    let kind = match e.kind {
                        horcrux::audit::EntryKind::DecryptOk => "ok",
                        horcrux::audit::EntryKind::DecryptFail => "fail",
                        horcrux::audit::EntryKind::Blocked => "blocked",
                        horcrux::audit::EntryKind::Signed => "signed",
                    };
                    println!(
                        "{}  {kind:<7}  shard {:>2}",
                        horcrux::audit::format_utc(e.ts),
                        e.shard_id
                    );
                }
            }
        }
        Command::QrRequest {
            group_dir,
            to,
            lamports,
            blockhash,
            dir,
            format,
        } => {
            let to: solana_pubkey::Pubkey = to
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;
            let blockhash: solana_hash::Hash = blockhash
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --blockhash: {e}"))?;

            let group_pub_path = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
            let group_pub_bytes = std::fs::read(&group_pub_path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", group_pub_path.display()))?;
            let verifying_key = horcrux::mpc::group_verifying_key(&group_pub_path)?;
            let from = solana_pubkey::Pubkey::from(verifying_key);

            let params = TxParams {
                from,
                to,
                lamports,
                blockhash,
            };
            let message = horcrux::tx::transaction_message(&params);
            let message_bytes = message.serialize();

            let request = horcrux::qr_mpc::mpc_sign_request(&group_pub_bytes, &message_bytes)?;
            let paths = horcrux::qr::write_message_transport(
                &dir,
                "request",
                format.into(),
                horcrux::qr::MessageType::Request,
                request.session,
                &request.to_payload(),
            )?;
            println!("Signing request: {from} -> {to} ({lamports} lamports)");
            println!("Session: {}", hex::encode(request.session));
            for p in &paths {
                println!("  {}", p.display());
            }
            println!(
                "Carry these file(s) across the air gap to each guardian, then run `horcrux qr-commit`."
            );
        }
        Command::QrCommit {
            share,
            password,
            dir,
            nonce_file,
            format,
            log_file,
            force,
        } => {
            let password = match password {
                Some(pw) => pw,
                None => rpassword::prompt_password(format!("Password for {}: ", share.display()))?,
            };

            let (msg_type, session, payload) =
                horcrux::qr::read_message_transport(&dir, "request")?;
            if msg_type != horcrux::qr::MessageType::Request {
                anyhow::bail!("expected a signing request in {}", dir.display());
            }
            let request = horcrux::qr_mpc::RequestData::from_payload(session, &payload)?;

            let share_id = horcrux::mpc::FrostShare::read(&share)?.id;
            let (access_log, attempt) = audit_preflight(vec![share_id], log_file, force)?;
            let ts = horcrux::audit::now_ms();
            let participant =
                horcrux::qr_mpc::load_participant(&request, &share, &password, |id, ok| {
                    let entry = if ok {
                        horcrux::audit::Entry::ok(ts, attempt, id)
                    } else {
                        horcrux::audit::Entry::fail(ts, attempt, id)
                    };
                    let _ = access_log.append(&entry);
                })?;

            let mut rng = rand::rngs::OsRng;
            let (commitment, nonces) =
                horcrux::qr_mpc::produce_commitment(&participant.key_package, &mut rng)?;

            let nonce_path = nonce_file.unwrap_or_else(|| default_nonce_path(&share));
            horcrux::qr_mpc::save_nonces(&nonce_path, &password, &session, &nonces)?;

            let prefix = format!("commit-{}", commitment.participant_id);
            let paths = horcrux::qr::write_message_transport(
                &dir,
                &prefix,
                format.into(),
                horcrux::qr::MessageType::Commitment,
                session,
                &commitment.to_payload(),
            )?;
            println!("Committed as participant {}", commitment.participant_id);
            for p in &paths {
                println!("  {}", p.display());
            }
            println!(
                "Nonces kept locally at {} until `qr-share` runs; do not copy this file across the air gap.",
                nonce_path.display()
            );
        }
        Command::QrPackage { dir, format } => {
            let (msg_type, session, payload) =
                horcrux::qr::read_message_transport(&dir, "request")?;
            if msg_type != horcrux::qr::MessageType::Request {
                anyhow::bail!("expected a signing request in {}", dir.display());
            }
            let request = horcrux::qr_mpc::RequestData::from_payload(session, &payload)?;

            let ids = horcrux::qr::list_participant_ids(&dir, "commit")?;
            if ids.is_empty() {
                anyhow::bail!("no commitments found in {}", dir.display());
            }
            let mut commitments = Vec::with_capacity(ids.len());
            for id in &ids {
                let prefix = format!("commit-{id}");
                let (t, s, payload) = horcrux::qr::read_message_transport(&dir, &prefix)?;
                if t != horcrux::qr::MessageType::Commitment || s != session {
                    anyhow::bail!("{prefix} does not belong to this signing session");
                }
                commitments.push(horcrux::qr_mpc::CommitmentData::from_payload(&payload)?);
            }

            let package_bytes = horcrux::qr_mpc::mpc_sign_package(&request, &commitments)?;
            let paths = horcrux::qr::write_message_transport(
                &dir,
                "package",
                format.into(),
                horcrux::qr::MessageType::SigningPackage,
                session,
                &package_bytes,
            )?;
            println!(
                "Signing package built from {} commitment(s): {ids:?}",
                ids.len()
            );
            for p in &paths {
                println!("  {}", p.display());
            }
        }
        Command::QrShare {
            share,
            password,
            dir,
            nonce_file,
            format,
            log_file,
            force,
        } => {
            let password = match password {
                Some(pw) => pw,
                None => rpassword::prompt_password(format!("Password for {}: ", share.display()))?,
            };

            let (msg_type, session, payload) =
                horcrux::qr::read_message_transport(&dir, "request")?;
            if msg_type != horcrux::qr::MessageType::Request {
                anyhow::bail!("expected a signing request in {}", dir.display());
            }
            let request = horcrux::qr_mpc::RequestData::from_payload(session, &payload)?;

            let (pkg_type, pkg_session, package_bytes) =
                horcrux::qr::read_message_transport(&dir, "package")?;
            if pkg_type != horcrux::qr::MessageType::SigningPackage || pkg_session != session {
                anyhow::bail!(
                    "signing package in {} does not match this request",
                    dir.display()
                );
            }

            let share_id = horcrux::mpc::FrostShare::read(&share)?.id;
            let (access_log, attempt) = audit_preflight(vec![share_id], log_file, force)?;
            let ts = horcrux::audit::now_ms();
            let participant =
                horcrux::qr_mpc::load_participant(&request, &share, &password, |id, ok| {
                    let entry = if ok {
                        horcrux::audit::Entry::ok(ts, attempt, id)
                    } else {
                        horcrux::audit::Entry::fail(ts, attempt, id)
                    };
                    let _ = access_log.append(&entry);
                })?;

            let nonce_path = nonce_file.unwrap_or_else(|| default_nonce_path(&share));
            let nonces = horcrux::qr_mpc::load_nonces(&nonce_path, &password, &session)?;

            let sig_share = horcrux::qr_mpc::produce_signature_share(
                &request,
                &package_bytes,
                &participant.key_package,
                nonces,
            )?;
            // Nonces are single-use; remove the local file once consumed
            // (best-effort — a leftover file is inert without the password,
            // but must never be reused for another signature).
            let _ = std::fs::remove_file(&nonce_path);
            let _ = access_log.append(&horcrux::audit::Entry::signed(
                horcrux::audit::now_ms(),
                attempt,
            ));

            let prefix = format!("share-{}", sig_share.participant_id);
            let paths = horcrux::qr::write_message_transport(
                &dir,
                &prefix,
                format.into(),
                horcrux::qr::MessageType::SignatureShare,
                session,
                &sig_share.to_payload(),
            )?;
            println!(
                "Signature share produced for participant {}",
                sig_share.participant_id
            );
            for p in &paths {
                println!("  {}", p.display());
            }
        }
        Command::QrFinalize {
            dir,
            rpc_url,
            broadcast,
            log_file: _,
        } => {
            let (msg_type, session, req_payload) =
                horcrux::qr::read_message_transport(&dir, "request")?;
            if msg_type != horcrux::qr::MessageType::Request {
                anyhow::bail!("expected a signing request in {}", dir.display());
            }
            let request = horcrux::qr_mpc::RequestData::from_payload(session, &req_payload)?;

            let (pkg_type, pkg_session, package_bytes) =
                horcrux::qr::read_message_transport(&dir, "package")?;
            if pkg_type != horcrux::qr::MessageType::SigningPackage || pkg_session != session {
                anyhow::bail!(
                    "signing package in {} does not match this request",
                    dir.display()
                );
            }

            let ids = horcrux::qr::list_participant_ids(&dir, "share")?;
            if ids.is_empty() {
                anyhow::bail!("no signature shares found in {}", dir.display());
            }
            let mut shares = Vec::with_capacity(ids.len());
            for id in &ids {
                let prefix = format!("share-{id}");
                let (t, s, payload) = horcrux::qr::read_message_transport(&dir, &prefix)?;
                if t != horcrux::qr::MessageType::SignatureShare || s != session {
                    anyhow::bail!("{prefix} does not belong to this signing session");
                }
                shares.push(horcrux::qr_mpc::SignatureShareData::from_payload(&payload)?);
            }

            let sig =
                horcrux::qr_mpc::finalize_signature(&package_bytes, &shares, &request.group_pub)?;

            let message: solana_message::Message = bincode::deserialize(&request.message)
                .map_err(|e| anyhow::anyhow!("failed to decode the signed message: {e}"))?;
            let signed = horcrux::tx::assemble_signed_transaction(
                message,
                sig.signature,
                sig.verifying_key,
            )?;

            println!("From:      {}", signed.from());
            println!("Signature: {}", signed.signature());
            println!("Raw:       {}", signed.raw_base58());

            if broadcast {
                let rpc_url = rpc_url.unwrap_or_else(horcrux::chain::default_rpc_url);
                println!("Broadcasting via {rpc_url}");
                let chain = horcrux::chain::Chain::connect(&rpc_url);
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
        Command::Verify { files, password } => {
            use horcrux::verify::{Kind, consistency_error, verify_files};

            let reports = verify_files(&files, password.as_deref());
            let kind = match reports.first().and_then(|r| r.kind) {
                Some(Kind::Sss) => "SSS shard",
                Some(Kind::Frost) => "FROST share",
                None => "unknown",
            };
            for r in &reports {
                let label = match r.kind {
                    Some(Kind::Sss) => "SSS shard",
                    Some(Kind::Frost) => "FROST share",
                    None => "invalid",
                };
                let params = r
                    .params
                    .map(|(t, n)| format!(" (t={t}, n={n})"))
                    .unwrap_or_default();
                println!(
                    "{:<8} {}{}  {}",
                    if r.ok { "ok" } else { "FAIL" },
                    label,
                    params,
                    r.path
                );
            }
            if let Some(reason) = consistency_error(&reports) {
                println!("Inconsistent set: {reason}");
                std::process::exit(1);
            }
            if reports.iter().all(|r| r.ok) {
                println!("All {} file(s) verified as {kind}.", reports.len());
            } else {
                println!("Verification failed for one or more files.");
                std::process::exit(1);
            }
        }
        Command::Tui => tui::run().await?,
        Command::Web { port } => web::run(port).await?,
    }
    Ok(())
}

/// Run the audit pre-flight check: score the proposed attempt against the
/// access log and either refuse (unless `--force`), warn, or allow it.
///
/// `ids` are the share/participant ids involved in the attempt, `log_file` the
/// access log path (or `None` for the default), and `force` whether to override
/// a blocking verdict. Returns the opened [`horcrux::audit::AccessLog`] and the
/// attempt id shared by every entry logged for this invocation.
fn audit_preflight(
    ids: Vec<u8>,
    log_file: Option<PathBuf>,
    force: bool,
) -> anyhow::Result<(horcrux::audit::AccessLog, u64)> {
    use horcrux::audit::{Entry, Scorer, Verdict};

    let log = horcrux::audit::AccessLog::open(access_log_path(log_file));
    let history = log.read_all()?;
    let now = horcrux::audit::now_ms();
    let attempt: u64 = rand::random();

    match Scorer::new().assess(&history, &ids, now) {
        Verdict::Block(reasons) => {
            log.append(&Entry::blocked(now, attempt))?;
            let msg = reasons.join("; ");
            if !force {
                anyhow::bail!(horcrux::error::Error::Blocked(msg));
            }
            println!("Audit: BLOCKED (overridden by --force) — {msg}");
        }
        Verdict::Warn(reasons) => println!("Audit: WARN — {}", reasons.join("; ")),
        Verdict::Allow => {}
    }
    Ok((log, attempt))
}

/// Resolve the access log path: `--log-file`, else `$HORCRUX_ACCESS_LOG`,
/// else the default `./horcrux-access.log`.
fn access_log_path(flag: Option<PathBuf>) -> PathBuf {
    horcrux::audit::resolve_log_path(flag)
}

/// Default location for a `qr-commit`/`qr-share` participant's local nonce
/// state: next to the share file, never inside the `--dir` that crosses the
/// air gap.
fn default_nonce_path(share: &std::path::Path) -> PathBuf {
    let mut name = share.as_os_str().to_owned();
    name.push(".qr-nonce");
    PathBuf::from(name)
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

/// Parse a `--utxo` value of the form `<txid>:<vout>:<amount-sat>`.
fn parse_utxo(s: &str) -> anyhow::Result<horcrux::bitcoin::Utxo> {
    let mut parts = s.split(':');
    let txid = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid --utxo {s:?}: missing txid"))?;
    let vout = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid --utxo {s:?}: missing vout"))?;
    let value = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid --utxo {s:?}: missing amount-sat"))?;
    if parts.next().is_some() {
        anyhow::bail!("invalid --utxo {s:?}: expected <txid>:<vout>:<amount-sat>");
    }
    let outpoint: bitcoin::OutPoint = format!("{txid}:{vout}")
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --utxo {s:?}: {e}"))?;
    let value_sat = value
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --utxo amount in {s:?}: {e}"))?;
    Ok(horcrux::bitcoin::Utxo {
        outpoint,
        value_sat,
    })
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
