use std::time::Duration;

#[derive(serde::Deserialize, Debug)]
pub struct Config {
    pub timeout_second: u64,
    pub nobitex_base_url: String,
    pub nobitex_ws_base_url: String,
    #[serde(with = "humantime_serde")]
    pub wait_ws_second: Duration,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string("config.toml")?;
        Ok(toml::from_str(content.as_str())?)
    }
}
