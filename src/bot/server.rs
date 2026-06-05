pub mod config;

use rand::Rng;
use std::sync::Arc;
use std::time::Duration;

use crate::bot::agent::BotAgent;
use crate::bot::agent::spawn_refresh_system_promt_task;
use crate::bot::handler::handle_channel_message;
use crate::bot::keepalive::spawn_keepalive_task;
use crate::bot::message::ChannelMessage;
use crate::bot::message::MessageActor;
use crate::bot::message::MessageContent;
use crate::bot::message::MessagePart;
use crate::bot::message::ReplyTarget;
use crate::bot::message::SendMessage;
use crate::bot::scheduled_task::spawn_spam_task;
use crate::bot::server::config::BotServerConfig;
use crate::bot::state::BotState;

use anyhow::Context as _;
use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use onebot_v11::event::message::GroupMessage;
use onebot_v11::event::message::Message as OneBotMessage;
use onebot_v11::{Event, MessageSegment};
use time::OffsetDateTime;
use time::UtcOffset;
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
        let state = BotState::new(agent, config.self_id);

        Ok(Self { conn, state })
    }

    pub async fn serve(mut self) -> anyhow::Result<()> {
        let reply_sender = GroupReplySender::new(self.conn.clone());
        let self_id = self
            .state
            .self_id()
            .parse::<i64>()
            .context("self id must be a valid OneBot user id")?;

        spawn_keepalive_task(self.conn.clone(), self_id);
        spawn_spam_task(self.conn.clone(), self_id);

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
        let Some(message) = filter_channel_message(&mut self.state, event) else {
            return;
        };

        let reply_target = message.reply_target();
        let outputs = handle_channel_message(&mut self.state, message).await;
        if outputs.is_empty() {
            return;
        }

        reply_sender.send_batch(reply_target, outputs).await;
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

    async fn send_batch(&self, target: ReplyTarget, outputs: Vec<SendMessage>) {
        sleep_random(FIRST_REPLY_DELAY_MS).await;

        let total = outputs.len();
        for (i, output) in outputs.into_iter().enumerate() {
            if let Err(error) = self.send_one(&target, output).await {
                tracing::error!("failed to reply to group message: {error}");
                break;
            }

            if i + 1 < total {
                sleep_random(BATCH_REPLY_DELAY_MS).await;
            }
        }
    }

    async fn send_one(&self, target: &ReplyTarget, output: SendMessage) -> anyhow::Result<()> {
        if output.content.is_empty() {
            return Ok(());
        }

        let message = if output.reply {
            let mut parts = Vec::with_capacity(output.content.len() + 1);

            parts.push(MessageSegment::reply(target.message_id.clone()));
            parts.extend(content_into_segments(output.content));

            parts
        } else {
            content_into_segments(output.content)
        };

        let group_id = target
            .channel_id
            .parse::<i64>()
            .context("channel id must be a valid OneBot group id")?;

        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message,
            auto_escape: false,
        });

        self.conn.clone().call_api(payload).await?;

        Ok(())
    }
}

fn filter_channel_message(
    state: &mut BotState,
    event: Result<Event, RecvError>,
) -> Option<ChannelMessage> {
    let event = receive_event(event)?;

    let message = extract_channel_message(event)?;

    if state.is_self(&message.actor.id) {
        return None;
    }

    if message.is_pure_text() {
        state.push_history_text(message.raw_text.clone());
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

fn extract_channel_message(event: Event) -> Option<ChannelMessage> {
    match event {
        Event::Message(OneBotMessage::GroupMessage(group_message)) => {
            Some(channel_message_from_onebot_group(group_message))
        }
        _ => None,
    }
}

fn channel_message_from_onebot_group(message: GroupMessage) -> ChannelMessage {
    ChannelMessage {
        self_id: message.self_id.to_string(),
        message_id: message.message_id.to_string(),
        channel_id: message.group_id.to_string(),
        actor: MessageActor {
            id: message.user_id.to_string(),
            nickname: message.sender.nickname.unwrap_or_default(),
            channel_nickname: message.sender.card,
        },
        sent_at: OffsetDateTime::from_unix_timestamp(message.time)
            .ok()
            .map(to_local_time)
            .unwrap_or_else(current_local_time),
        raw_text: message.raw_message,
        content: content_from_segments(message.message),
    }
}

fn content_from_segments(segments: Vec<MessageSegment>) -> MessageContent {
    MessageContent {
        parts: segments.into_iter().map(part_from_segment).collect(),
    }
}

fn part_from_segment(segment: MessageSegment) -> MessagePart {
    match segment {
        MessageSegment::Text { data } => MessagePart::Text(data.text),
        MessageSegment::At { data } => MessagePart::Mention { actor_id: data.qq },
        MessageSegment::Reply { data } => MessagePart::Reply {
            message_id: data.id,
        },
        _ => MessagePart::Other,
    }
}

fn content_into_segments(content: MessageContent) -> Vec<MessageSegment> {
    content
        .parts
        .into_iter()
        .filter_map(part_into_segment)
        .collect()
}

fn part_into_segment(part: MessagePart) -> Option<MessageSegment> {
    match part {
        MessagePart::Text(text) => Some(MessageSegment::text(text)),
        MessagePart::Mention { actor_id } => Some(MessageSegment::at(actor_id)),
        MessagePart::Reply { message_id } => Some(MessageSegment::reply(message_id)),
        MessagePart::Other => None,
    }
}

fn to_local_time(time: OffsetDateTime) -> OffsetDateTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    time.to_offset(local_offset)
}

fn current_local_time() -> OffsetDateTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc().to_offset(local_offset)
}

async fn sleep_random(range: std::ops::Range<u64>) {
    let delay_ms = rand::thread_rng().gen_range(range);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}
