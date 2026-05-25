use std::env;
use std::sync::Arc;

use anyhow::Context as _;
use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::{ReverseWsConfig, ReverseWsConnect};
use onebot_v11::event::message::Message as OneBotMessage;
use onebot_v11::{Event, MessageSegment};

use crate::bot::message::Message;
use crate::bot::router::Router;
use crate::bot::state::BotState;

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
                .with_context(|| format!("invalid NAPCAT_REVERSE_WS_PORT: {value}"))?,
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

pub struct BotServer {
    connection: Arc<ReverseWsConnect>,
}

impl BotServer {
    pub async fn reverse_websock(config: ReverseWebSockServerConfig) -> anyhow::Result<Self> {
        let connection = ReverseWsConnect::new(config.into()).await?;
        Ok(Self { connection })
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        Self::reverse_websock(ReverseWebSockServerConfig::from_env()?).await
    }

    pub async fn serve(self, router: Router) -> anyhow::Result<()> {
        let mut events = self.connection.subscribe().await;

        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!("failed to receive onebot event: {error}");
                    continue;
                }
            };

            let Some(message) = Self::extract_group_message(event) else {
                continue;
            };

            if message.user_id() == message.self_id() {
                continue;
            }

            let state = BotState::new(self.connection.clone());

            let reply = match router.handle_group_message(state, message.clone()).await {
                Ok(reply) => reply,
                Err(error) => {
                    tracing::error!("group message handler failed: {error:?}");
                    continue;
                }
            };

            let Some(reply) = reply else {
                continue;
            };

            if let Err(error) = self.reply_to_group_message(message, reply).await {
                tracing::error!("failed to send group reply: {error:?}");
            }
        }
    }

    fn extract_group_message(event: Event) -> Option<Message> {
        match event {
            Event::Message(OneBotMessage::GroupMessage(group_message)) => {
                Some(Message::from_group_message(group_message))
            }
            _ => None,
        }
    }

    async fn reply_to_group_message(
        &self,
        incoming: Message,
        reply: Message,
    ) -> anyhow::Result<()> {
        if reply.segments().is_empty() {
            return Ok(());
        }

        let group_id = incoming
            .group_id()
            .context("group reply is missing target group id")?;
        let message_id = incoming
            .message_id()
            .context("group reply is missing source message id")?;

        let mut message = Vec::with_capacity(reply.segments().len() + 1);
        message.push(MessageSegment::reply(message_id.to_string()));
        message.extend(reply.into_segments());

        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message,
            auto_escape: false,
        });

        self.connection.clone().call_api(payload).await?;
        Ok(())
    }
}
