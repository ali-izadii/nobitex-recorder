use rust_decimal::Decimal;
use serde::Deserialize;
use std::time::SystemTime;

type ResponseResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
struct RawSnapshot {
    status: String,
    #[serde(rename = "lastUpdate")]
    last_updated: u64,
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

#[derive(Debug)]
pub struct Level {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug)]
pub struct Snapshot {
    pub exchange_last_updated: u64,
    pub receive_at: SystemTime,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

fn parse_levels(raw: Vec<[String; 2]>, side: &str) -> ResponseResult<Vec<Level>> {
    raw.into_iter()
        .enumerate()
        .map(|(index, [price, quantity])| {
            let price = Decimal::from_str_exact(&price)
                .map_err(|error| format!("{side}[{index}] invalid price: {error}"))?;
            let quantity = Decimal::from_str_exact(&quantity)
                .map_err(|error| format!("{side}[{index}] invalid quantity: {error}"))?;

            if price <= Decimal::ZERO {
                return Err(format!("{side}[{index}] price must be positive").into());
            }

            if quantity < Decimal::ZERO {
                return Err(format!("{side}[{index}] quantity must be positive").into());
            }
            Ok(Level { price, quantity })
        })
        .collect()
}

impl Snapshot {
    pub fn parse(body: &str, received_at: SystemTime) -> ResponseResult<Self> {
        let raw: RawSnapshot = serde_json::from_str(&body)?;
        if raw.status != "ok" {
            return Err(format!("exchange returned status: {}", raw.status).into());
        }
        Ok(Self {
            exchange_last_updated: raw.last_updated,
            receive_at: received_at,
            bids: parse_levels(raw.bids, "bids")?,
            asks: parse_levels(raw.asks, "ask")?,
        })
    }

    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids
            .iter()
            .filter(|level| level.quantity > Decimal::ZERO)
            .map(|level| level.price)
            .max()
    }

    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks
            .iter()
            .filter(|level| level.quantity > Decimal::ZERO)
            .map(|level| level.price)
            .min()
    }

    pub fn spread(&self) -> Option<Decimal> {
        Some(self.best_ask()? - self.best_bid()?)
    }
}
