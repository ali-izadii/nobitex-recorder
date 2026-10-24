use crate::config::Config;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::{Instant, sleep_until, timeout};
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

    let channel = format!("public:orderbook-{symbol}");
    let mut phase = Phase::Connecting;
    let mut deadline = Instant::now() + wait;

    timeout(
        wait,
        socket.send(Message::Text(
            json!({"id": 1, "connect": {}}).to_string().into(),
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

            _ = sleep_until(deadline), if phase != Phase::Watching => {
                return Err(format!("timed out while {phase:?}").into());
            }
        };

        match message {
            Message::Text(text) => {
                let text = text.as_str().trim();
                if text.is_empty() {
                    continue;
                }
                let envelope: Value = serde_json::from_str(text)?;

                if envelope.as_object().is_some_and(|object| object.is_empty()) {
                    timeout(wait, socket.send(Message::Text("{}".into()))).await??;
                    continue;
                }

                if let Some(error) = envelope.get("error") {
                    return Err(format!("Centrifugo error: {error}").into());
                }

                match (envelope.get("id").and_then(Value::as_u64), &phase) {
                    (Some(1), Phase::Connecting)
                        if envelope.get("connect").is_some_and(Value::is_object) =>
                    {
                        timeout(
                            wait,
                            socket.send(Message::Text(
                                json!({"id": 2, "subscribe": {"channel": &channel}})
                                    .to_string()
                                    .into(),
                            )),
                        )
                        .await??;

                        phase = Phase::Subscribing;
                        deadline = Instant::now() + wait;
                    }
                    (Some(2), Phase::Subscribing)
                        if envelope.get("subscribe").is_some_and(Value::is_object) =>
                    {
                        phase = Phase::Watching;
                        eprintln!("Subscribed to {channel}");
                    }
                    (Some(_), _) => {
                        return Err(
                            format!("unexpected command reply in {phase:?}: {envelope}").into()
                        );
                    }
                    (None, Phase::Watching) => {
                        let push = envelope
                            .get("push")
                            .ok_or("expected a command reply or push")?;
                        let book = order_book(push, &channel)?;
                        writeln!(output, "{book}")?;
                    }

                    (None, _) => {
                        return Err(format!(
                            "message before subscription in {phase:?}: {envelope}"
                        )
                        .into());
                    }
                }
            }
            Message::Ping(_) => {
                timeout(wait, socket.send(Message::Text("{}".into()))).await??;
            }
            Message::Pong(_) => {}
            Message::Close(frame) => {
                return Err(format!("server closed the WebSocket: {frame:?}").into());
            }
            other => {
                return Err(format!("unexpected WebSocket message: {other:?}").into());
            }
        }
    }
}

fn order_book(push: &Value, channel: &str) -> Result<Value> {
    if let Some(reason) = push.get("disconnect") {
        return Err(format!("server disconnected: {reason}").into());
    }

    if let Some(reason) = push.get("unsubscribe") {
        return Err(format!("server unsubscribed: {reason}").into());
    }

    if push.get("channel").and_then(Value::as_str) != Some(channel) {
        return Err(format!("unexpected publication channel: {push}").into());
    }

    let data = push
        .get("pub")
        .and_then(|publication| publication.get("data"))
        .ok_or("push has no publication data")?;

    let book = match data {
        Value::String(encoded) => serde_json::from_str::<Value>(encoded)?,
        Value::Object(_) => data.clone(),
        _ => return Err("publication data must be a string or object".into()),
    };

    if !book.is_object() {
        return Err("decoded order book must be an object".into());
    }

    Ok(book)
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
