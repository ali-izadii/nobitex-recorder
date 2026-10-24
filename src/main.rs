mod config;
mod snapshot;
mod watch;

use crate::config::Config;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Snapshot {
        #[arg(short, long, default_value = "BTCIRT")]
        symbol: String,
    },
    Watch {
        recording_path: Option<PathBuf>,

        #[arg(short, long, default_value = "BTCIRT")]
        symbol: String,
    },
    Reply {
        recording_path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Command::Snapshot { symbol } => snapshot::run(&symbol, &config),
        Command::Watch {
            symbol,
            recording_path,
        } => watch::run(&symbol, recording_path, &config),
        Command::Reply { recording_path } => {
            println!(
                "Replay from {}: not implemented yet",
                recording_path.display()
            );
            Ok(())
        }
    }
}
