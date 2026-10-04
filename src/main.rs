mod config;
mod snapshot;
mod watch;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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

fn fetch_snapshot(symbol: &str) -> Result<snapshot::Snapshot, Box<dyn std::error::Error>> {
    let config = config::Config::load()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(config.timeout_second))
        .build()?;

    let response = client
        .get(format!("{}/v3/orderbook/{symbol}", config.nobitex_base_url))
        .send()?
        .error_for_status()?
        .text()?;

    snapshot::Snapshot::parse(&response, SystemTime::now())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load()?;

    let cli = Cli::parse();
    match cli.command {
        Command::Snapshot { symbol } => {
            let snapshot = fetch_snapshot(&symbol)?;
            let display = |value: Option<rust_decimal::Decimal>| {
                value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            };

            println!(
                "{symbol} best_bid={} best_ask={} spread={}",
                display(snapshot.best_bid()),
                display(snapshot.best_ask()),
                display(snapshot.spread()),
            );
            println!(
                "exchange_last_update={} received_at={:?}",
                snapshot.exchange_last_updated, snapshot.receive_at,
            );
        }
        Command::Watch {
            symbol,
            recording_path,
        } => {
            if let Some(path) = &recording_path {
                eprintln!("Recording requested: {}", path.display());
            } else {
                eprintln!("Console only.");
            }
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(watch::run(
                    &config.nobitex_ws_base_url,
                    &symbol,
                    recording_path,
                ))?;
        }
        Command::Reply { recording_path } => {
            println!(
                "Replay from {}: not implemented yet",
                recording_path.display()
            );
        }
    }
    Ok(())
}
