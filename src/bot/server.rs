pub mod config;

use rand::Rng;
use std::sync::Arc;
use std::time::Duration;

use crate::bot::agent::BotAgent;
use crate::bot::agent::prompt::spawn_refresh_system_promt_task;
use crate::bot::handler::handle_group_message;
use crate::bot::keepalive::spawn_keepalive_task;
use crate::bot::message::{InputMessage, OutputMessage};
use crate::bot::scheduled_task::spawn_spam_task;
use crate::bot::server::config::BotServerConfig;
use crate::bot::state::BotState;

use anyhow::Context as _;
use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use onebot_v11::event::message::Message as OneBotMessage;
use onebot_v11::{Event, MessageSegment};
use tokio::sync::broadcast::error::RecvError;

const FIRST_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..5000;
const BATCH_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..3000;

pub struct BotServer {
    conn: Arc<ReverseWsConnect>,
    state: BotState,
}

impl BotServer {
    pub async fn from_env() -> anyhow::Result<Self> {
        Self::new(BotServerConfig::from_env()?).await
    }

    pub async fn new(config: BotServerConfig) -> anyhow::Result<Self> {
        let conn = ReverseWsConnect::new(config.reverse_ws.into()).await?;

        let agent = BotAgent::new().await?;
        let state = BotState::new(agent, config.self_qid);

        Ok(Self { conn, state })
    }

    pub async fn serve(mut self) -> anyhow::Result<()> {
        let reply_sender = GroupReplySender::new(self.conn.clone());
        let self_qid = self.state.self_qid();

        spawn_keepalive_task(self.conn.clone(), self_qid);
        spawn_spam_task(self.conn.clone(), self_qid);

        let mut event_recv = self.conn.subscribe().await;
        let mut prompt_recv = spawn_refresh_system_promt_task()?;

        loop {
            tokio::select! {
                event = event_recv.recv() => {
                    self.handle_event(event, &reply_sender).await;
                }
                prompt = prompt_recv.recv() => {
                    if !self.handle_prompt_reload(prompt) {
                        break Ok(());
                    }
                }
            }
        }
    }

    async fn handle_event(
        &mut self,
        event: Result<Event, RecvError>,
        reply_sender: &GroupReplySender,
    ) {
        let Some(message) = filter_group_message(&mut self.state, event) else {
            return;
        };

        let outputs = handle_group_message(&mut self.state, &message).await;
        if outputs.is_empty() {
            return;
        }

        reply_sender.send_batch(message, outputs).await;
    }

    /// Returns `true` if the loop should continue, `false` if the channel closed.
    fn handle_prompt_reload(&mut self, new_prompt: Option<String>) -> bool {
        match new_prompt {
            Some(content) => {
                self.state.agent_mut().reload_system_prompt(content);
                true
            }
            None => {
                tracing::warn!("prompt refresh channel closed, stopping reloads");
                false
            }
        }
    }
}

struct GroupReplySender {
    conn: Arc<ReverseWsConnect>,
}

impl GroupReplySender {
    fn new(conn: Arc<ReverseWsConnect>) -> Self {
        Self { conn }
    }

    async fn send_batch(&self, message: InputMessage, outputs: Vec<OutputMessage>) {
        sleep_random(FIRST_REPLY_DELAY_MS).await;

        let total = outputs.len();
        for (i, output) in outputs.into_iter().enumerate() {
            if let Err(error) = self.send_one(&message, output).await {
                tracing::error!("failed to reply to group message: {error}");
                break;
            }

            if i + 1 < total {
                sleep_random(BATCH_REPLY_DELAY_MS).await;
            }
        }
    }

    async fn send_one(&self, incoming: &InputMessage, output: OutputMessage) -> anyhow::Result<()> {
        if output.segments().is_empty() {
            return Ok(());
        }

        let group_id = incoming
            .group_id()
            .context("group reply is missing target group id")?;
        let message = if output.reply {
            let message_id = incoming
                .message_id()
                .context("group reply is missing source message id")?;

            let mut parts = Vec::with_capacity(output.segments().len() + 1);

            parts.push(MessageSegment::reply(message_id.to_string()));
            parts.extend(output.into_segments());

            parts
        } else {
            output.into_segments()
        };

        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message,
            auto_escape: false,
        });

        self.conn.clone().call_api(payload).await?;

        Ok(())
    }
}

fn filter_group_message(
    state: &mut BotState,
    event: Result<Event, RecvError>,
) -> Option<InputMessage> {
    let event = receive_event(event)?;

    let message = extract_group_message(event)?;

    if message.user_id().is_some_and(|qid| state.is_self(qid)) {
        return None;
    }

    if message.is_pure_text() {
        state.push_history(message.clone());
    }

    Some(message)
}

fn receive_event(event: Result<Event, RecvError>) -> Option<Event> {
    if let Err(e) = &event {
        tracing::error!("failed to receive event: {e}");
        return None;
    }

    event.ok()
}

fn extract_group_message(event: Event) -> Option<InputMessage> {
    match event {
        Event::Message(OneBotMessage::GroupMessage(group_message)) => {
            Some(InputMessage::from_group_message(group_message))
        }
        _ => None,
    }
}

async fn sleep_random(range: std::ops::Range<u64>) {
    let delay_ms = rand::thread_rng().gen_range(range);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}
