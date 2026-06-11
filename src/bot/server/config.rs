use std::env;

use anyhow::Context as _;
use onebot_v11::connect::ws_reverse::ReverseWsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseWebSockServerConfig {
    pub host: String,
    pub port: u16,
    pub suffix: String,
    pub access_token: Option<String>,
}

impl Default for ReverseWebSockServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
            suffix: "onebot/v11".to_string(),
            access_token: None,
        }
    }
}

impl ReverseWebSockServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let host = env::var("NAPCAT_REVERSE_WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let port = match env::var("NAPCAT_REVERSE_WS_PORT") {
            Ok(value) => value
                .parse::<u16>()
                .with_context(|| format!("invalid NAPCAT_REVERSE_WS_PORT: {}", value))?,
            Err(_) => 8081,
        };

        let suffix =
            env::var("NAPCAT_REVERSE_WS_SUFFIX").unwrap_or_else(|_| "onebot/v11".to_string());

        let access_token = env::var("NAPCAT_ACCESS_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());

        Ok(Self {
            host,
            port,
            suffix,
            access_token,
        })
    }
}

impl From<ReverseWebSockServerConfig> for ReverseWsConfig {
    fn from(value: ReverseWebSockServerConfig) -> Self {
        Self {
            host: value.host,
            port: value.port,
            suffix: value.suffix,
            access_token: value.access_token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotServerConfig {
    pub reverse_ws: ReverseWebSockServerConfig,
    pub self_id: String,
}

impl BotServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let reverse_ws = ReverseWebSockServerConfig::from_env()?;
        let self_id = env::var("ACCOUNT").context("ACCOUNT not set in environment")?;

        Ok(Self {
            reverse_ws,
            self_id,
        })
    }
}
