#[derive(serde::Deserialize, Debug)]
pub struct Config {
    pub timeout_second: u64,
    pub api_key: String,
    pub nobitex_base_url: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string("config.toml")?;
        Ok(toml::from_str(content.as_str())?)
    }
}
