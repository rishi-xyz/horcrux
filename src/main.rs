use clap::{Parser, Subcommand};
use horcrux::error::Error;
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
}

fn main() -> anyhow::Result<()> {
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
            println!("Wrote {n} shards to {dir} (threshold {t}):", n = paths.len(), dir = out_dir.display(), t = threshold);
            for path in &paths {
                println!("  {}", path.display());
            }
        }
        Command::Reconstruct { shards, password } => {
            let passwords = collect_passwords(
                shards.len(),
                password,
                &|i, _| {
                    let name = shards[i].file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    format!("Password for {name}: ")
                },
                false,
            )?;

            let key = reconstruct(&shards, &passwords)?;
            println!("Reconstructed key: 0x{}", hex::encode(key.to_bytes()));
        }
    }
    Ok(())
}

/// Parse a hex-encoded secp256k1 private key (32 bytes), tolerating a 0x
/// prefix.
fn parse_key(hex_key: &str) -> Result<SecretKey, Error> {
    let stripped = hex_key.strip_prefix("0x").unwrap_or(hex_key);
    let bytes = hex::decode(stripped)
        .map_err(|e| Error::InvalidKey(format!("not valid hex: {e}")))?;
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
