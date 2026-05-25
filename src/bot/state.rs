use std::sync::Arc;

use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::ReverseWsConnect;

use crate::bot::message::Message;

#[derive(Clone)]
pub struct Bot {
    connection: Arc<ReverseWsConnect>,
}

impl Bot {
    pub fn new(connection: Arc<ReverseWsConnect>) -> Self {
        Self { connection }
    }

    pub async fn send_group(&self, group_id: i64, message: Message) -> anyhow::Result<()> {
        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message: message.into_segments(),
            auto_escape: false,
        });

        self.connection.clone().call_api(payload).await?;

        Ok(())
    }

    pub async fn send_group_text(
        &self,
        group_id: i64,
        text: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.send_group(group_id, Message::text(text)).await
    }
}

#[derive(Clone)]
pub struct BotState {
    pub bot: Bot,
}

impl BotState {
    pub(crate) fn new(connection: Arc<ReverseWsConnect>) -> Self {
        Self {
            bot: Bot::new(connection),
        }
    }
}
