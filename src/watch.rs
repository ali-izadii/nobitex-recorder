use crate::config::Config;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq)]
enum Phase {
    Connecting,
    Subscribing,
    Watching,
}

pub fn run(symbol: &str, recording_path: Option<PathBuf>, config: &Config) -> Result<()> {
    match &recording_path {
        Some(path) => eprintln!("Recording requested: {}", path.display()),
        None => eprintln!("Console only."),
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(watch(
            &config.nobitex_ws_base_url,
            symbol,
            recording_path,
            config.wait_ws_second,
        ))
}

async fn watch(
    url: &str,
    symbol: &str,
    recording_path: Option<PathBuf>,
    wait: Duration,
) -> Result<()> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() || !symbol.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("symbol must contain only ASCII letters and digits".into());
    }

    let mut output = open_output(recording_path.as_ref())?;
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    let (mut socket, _) = tokio::select! {
        result = timeout(wait, connect_async(url)) => result??,
        result = &mut shutdown => {
            result?;
            return Ok(());
        }
    };

    eprintln!("Connected to {url} for {symbol}");

    timeout(
        wait,
        socket.send(Message::Text(
            json!({"id": 1, "connect": {}}).to_string().into(),
        )),
    )
    .await??;

    let channel = format!("public:orderbook-{symbol}");

    timeout(
        wait,
        socket.send(Message::Text(
            json!({"id": 2, "subscribe": {"channel": &channel}})
                .to_string()
                .into(),
        )),
    )
    .await??;

    loop {
        let message = tokio::select! {
            result = &mut shutdown => {
                result?;
                return Ok(());
            }
            message = socket.next() => {
                message.ok_or("WebSocket stream ended unexpectedly")??
            }
        };

        match message {
            Message::Text(text) if text.as_str().trim() == "{}" => {
                timeout(wait, socket.send(Message::Text("{}".into()))).await??;
            }
            Message::Text(text) => writeln!(output, "{}\n\n", text.as_str())?,
            other => eprintln!("non-text message: {other:?}"),
        }
    }
}

fn open_output(recording_path: Option<&PathBuf>) -> Result<Box<dyn Write>> {
    match recording_path {
        Some(path) => {
            let directory = PathBuf::from("recording");
            std::fs::create_dir_all(&directory)?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join(path))?;
            Ok(Box::new(file))
        }
        None => Ok(Box::new(std::io::stdout())),
    }
}
