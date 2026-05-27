use rand::Rng;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use crate::bot::agent::BotAgent;
use crate::bot::handler::handle_group_message;
use crate::bot::keepalive::spawn_keepalive_task;
use crate::bot::message::Message;
use crate::bot::scheduled_task::spawn_spam_task;
use crate::bot::state::BotState;

use anyhow::Context as _;
use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::{ReverseWsConfig, ReverseWsConnect};
use onebot_v11::event::message::Message as OneBotMessage;
use onebot_v11::{Event, MessageSegment};

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

    pub async fn serve(self) -> anyhow::Result<()> {
        let mut events = self.connection.subscribe().await;

        let agent = BotAgent::new().await?;
        let mut state = BotState::new(agent);

        let self_id: i64 = env::var("ACCOUNT")
            .context("ACCOUNT not set in environment")?
            .parse()
            .context("ACCOUNT must be a valid i64")?;

        let connection = self.connection;

        spawn_keepalive_task(connection.clone(), self_id);
        spawn_spam_task(connection.clone());

        // Main event loop — handles group messages only.
        loop {
            let event = match events.recv().await {
                Ok(event) => {
                    tracing::debug!("received onebot event: {event:?}");
                    event
                }
                Err(error) => {
                    tracing::warn!("failed to receive onebot event: {error}");
                    continue;
                }
            };

            let message = match Self::extract_group_message(event) {
                Some(msg) => msg,
                None => continue,
            };

            if message.user_id() == message.self_id() {
                continue;
            }

            let reply = match handle_group_message(&mut state, &message).await {
                Some(reply) => reply,
                None => continue,
            };

            // Random delay 2–5 seconds to avoid rate-limiting.
            let delay_ms = rand::thread_rng().gen_range(2000..5000);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;

            if let Err(error) =
                Self::reply_to_group_message(connection.clone(), message, reply).await
            {
                tracing::error!("failed to reply to group message: {error}");
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
        connection: Arc<ReverseWsConnect>,
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

        connection.call_api(payload).await?;

        Ok(())
    }
}
