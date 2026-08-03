use clap::{Parser, Subcommand};

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
        /// Number of shares required to reconstruct the key.
        #[arg(long, default_value_t = 2)]
        threshold: u8,
        /// Total number of shares to create.
        #[arg(long, default_value_t = 3)]
        shares: u8,
    },
    /// Reconstruct a private key from shard files.
    Reconstruct {
        /// Paths to the shard files to combine.
        #[arg(required = true)]
        shards: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { .. } => {
            anyhow::bail!("not implemented yet")
        }
        Command::Reconstruct { .. } => {
            anyhow::bail!("not implemented yet")
        }
    }
}
