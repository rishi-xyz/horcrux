use clap::{Parser, Subcommand};
use horcrux::error::Error;
use horcrux::tx::{TxParams, derive_address};
use horcrux::{init_shards, reconstruct_with_audit};
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
        /// Access log file (default: ./horcrux-access.log or
        /// $HORCRUX_ACCESS_LOG).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Bypass audit blocking (blocked attempts are still logged).
        #[arg(long)]
        force: bool,
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
    /// reconstructing the key.
    MpcSplit {
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
        /// Directory to write share files and group.pub into.
        #[arg(long, default_value = "mpc")]
        out_dir: PathBuf,
        /// Use this password for every share (else prompt per share).
        #[arg(long)]
        password: Option<String>,
    },
    /// Sign a Solana transaction with a threshold FROST subset of key shares,
    /// optionally broadcasting to a cluster (Mode B).
    MpcSign {
        /// Paths of the FROST share files to combine.
        #[arg(required = true)]
        shares: Vec<PathBuf>,
        /// Directory containing the group public key package (group.pub).
        #[arg(long, default_value = "mpc")]
        group_dir: PathBuf,
        /// Use this password for every share (else prompt per share).
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
                &|i, n| format!("Password for share {} of {n}: ", i + 1),
                true,
            )?;

            let (paths, group_path) =
                horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)?;
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
            password,
            to,
            lamports,
            blockhash,
            rpc_url,
            broadcast,
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
        Command::MpcSign {
            shares,
            group_dir,
            password,
            to,
            lamports,
            blockhash,
            rpc_url,
            broadcast,
            log_file,
            force,
        } => {
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
    flag.or_else(|| std::env::var_os("HORCRUX_ACCESS_LOG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("horcrux-access.log"))
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
