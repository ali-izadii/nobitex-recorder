use crate::config;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq)]
enum Phase {
    Connecting,
    Subscribing,
    Watching,
}

pub async fn run(url: &str, symbol: &str, recording_path: Option<PathBuf>) -> Result<()> {
    let config = config::Config::load()?;
    let wait = config.wait_ws_second;

    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() || !symbol.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("symbol must contain only ASCII letters and digits".into());
    }

    let mut recording = match &recording_path {
        Some(path) => {
            let directory = PathBuf::from("recording");
            std::fs::create_dir_all(&directory)?;
            Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(directory.join(path))?,
            )
        }
        None => None,
    };

    let channel = format!("public:orderbook-{symbol}");
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    let (mut socket, _) = tokio::select! {
        result = timeout(wait, connect_async(url)) => result??,
        result = &mut shutdown => {
            result?;
            return Ok(());
        }
    };

    timeout(
        wait,
        socket.send(Message::Text(
            json!({"id": 1, "connect": {}}).to_string().into(),
        )),
    )
    .await??;

    let mut phase = Phase::Connecting;
    let mut deadline = Instant::now() + wait;

    loop {
        let message = tokio::select! {
            result = &mut shutdown => {
                result?;

                // Send Close, then wait briefly for the peer's acknowledgement.
                timeout(wait, async {
                    socket.close(None).await?;

                    while let Some(message) = socket.next().await {
                        if matches!(message?, Message::Close(_)) {
                            return Ok::<(), Box<dyn std::error::Error>>(());
                        }
                    }

                    Err("socket ended without a close acknowledgement".into())
                })
                .await??;

                eprintln!("Watch stopped.");
                return Ok(());
            }

            _ = sleep_until(deadline), if phase != Phase::Watching => {
                return Err(format!("timed out while {phase:?}").into());
            }

            message = socket.next() => {
                message.ok_or("WebSocket stream ended unexpectedly")??
            }
        };

        match message {
            Message::Text(text) => {
                // A text message can contain multiple JSON envelopes.
                for envelope in
                    serde_json::Deserializer::from_str(text.as_str()).into_iter::<Value>()
                {
                    let envelope = envelope?;
                    let object = envelope
                        .as_object()
                        .ok_or("expected a JSON object envelope")?;

                    // Centrifugo heartbeat: different from WebSocket Ping.
                    if object.is_empty() {
                        timeout(wait, socket.send(Message::Text("{}".into()))).await??;
                        continue;
                    }

                    if let Some(error) = envelope.get("error") {
                        return Err(format!("Centrifugo error: {error}").into());
                    }

                    if let Some(id) = envelope.get("id") {
                        match (id.as_u64(), &phase) {
                            (Some(1), Phase::Connecting)
                                if envelope.get("connect").is_some_and(Value::is_object) =>
                            {
                                timeout(
                                    wait,
                                    socket.send(Message::Text(
                                        json!({
                                            "id": 2,
                                            "subscribe": {"channel": &channel}
                                        })
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

                            _ => {
                                return Err(format!("unexpected command reply: {envelope}").into());
                            }
                        }

                        continue;
                    }

                    let push = envelope
                        .get("push")
                        .ok_or("expected a command reply or push")?;

                    if let Some(reason) = push.get("disconnect") {
                        return Err(format!("server disconnected: {reason}").into());
                    }

                    if let Some(reason) = push.get("unsubscribe") {
                        return Err(format!("server unsubscribed: {reason}").into());
                    }

                    if phase != Phase::Watching {
                        return Err("publication arrived before subscription confirmation".into());
                    }

                    if push.get("channel").and_then(Value::as_str) != Some(channel.as_str()) {
                        return Err(format!("unexpected publication channel: {push}").into());
                    }

                    let data = push
                        .get("pub")
                        .and_then(|publication| publication.get("data"))
                        .ok_or("push has no publication data")?;

                    // Nobitex documents both JSON strings and objects.
                    let book = match data {
                        Value::String(encoded) => serde_json::from_str::<Value>(encoded)?,
                        Value::Object(_) => data.clone(),
                        _ => return Err("publication data must be a string or object".into()),
                    };

                    if !book.is_object() {
                        return Err("decoded order book must be an object".into());
                    }

                    let line = format!("{book}\n");
                    match recording.as_mut() {
                        Some(file) => file.write_all(line.as_bytes())?,
                        _ => {}
                    }
                }
            }

            Message::Ping(_) => {
                // Tungstenite queues the matching Pong automatically.
                timeout(wait, socket.flush()).await??;
            }

            Message::Pong(_) => {}

            Message::Close(reason) => {
                timeout(wait, socket.flush()).await??;
                return Err(format!("server closed the WebSocket: {reason:?}").into());
            }

            other => {
                return Err(format!("unexpected WebSocket message: {other:?}").into());
            }
        }
    }
}
