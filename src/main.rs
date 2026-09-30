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
        recording_path: PathBuf,

        #[arg(short, long, default_value = "BTCIRT")]
        symbol: String,
    },
    Reply {
        recording_path: PathBuf,
    },
}
fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Snapshot { symbol } => {
            println!("{symbol} snapshot: not implemented yet");
        }
        Command::Watch {
            symbol,
            recording_path,
        } => {
            println!(
                "{symbol} recording to {}: not implemented yet",
                recording_path.display()
            );
        }
        Command::Reply { recording_path } => {
            println!(
                "Replay from {}: not implemented yet",
                recording_path.display()
            );
        }
    }
}
