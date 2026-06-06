use rand::Rng;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use onebot_v11::Event;
use onebot_v11::MessageSegment;
use onebot_v11::api::payload::ApiPayload;
use onebot_v11::api::payload::SendGroupMsg;
use onebot_v11::api::payload::SendPrivateMsg;
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use onebot_v11::event::message::GroupMessage;
use onebot_v11::event::message::Message as OneBotMessage;
use time::OffsetDateTime;
use time::UtcOffset;
use tokio::sync::broadcast::error::RecvError;

use crate::bot::message::BotCommand;
use crate::bot::message::ChannelMessage;
use crate::bot::message::ImageData;
use crate::bot::message::MessageActor;
use crate::bot::message::MessageContent;
use crate::bot::message::MessagePart;
use crate::bot::message::ReplyTarget;

const FIRST_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..5000;
const BATCH_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..3000;

pub struct OneBotSender {
    connect: Arc<ReverseWsConnect>,
}

impl OneBotSender {
    pub fn new(connect: Arc<ReverseWsConnect>) -> Self {
        Self { connect }
    }

    pub async fn send_batch(&self, commands: Vec<BotCommand>, delayed: bool) {
        if commands.is_empty() {
            return;
        }

        if delayed {
            sleep_random(FIRST_REPLY_DELAY_MS).await;
        }

        let total = commands.len();
        for (index, command) in commands.into_iter().enumerate() {
            if let Err(error) = self.send_one(command).await {
                tracing::error!("failed to send bot command: {}", error);
                break;
            }

            if delayed && index + 1 < total {
                sleep_random(BATCH_REPLY_DELAY_MS).await;
            }
        }
    }

    async fn send_one(&self, command: BotCommand) -> anyhow::Result<()> {
        match command {
            BotCommand::ReplyTo { target, content } => self.send_reply(target, content).await,
            BotCommand::SendChannel {
                channel_id,
                content,
            } => self.send_channel(channel_id, content).await,
            BotCommand::SendDirect { actor_id, content } => {
                self.send_direct(actor_id, content).await
            }
        }
    }

    async fn send_reply(&self, target: ReplyTarget, content: MessageContent) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let mut message = Vec::with_capacity(content.len() + 1);
        message.push(MessageSegment::reply(target.message_id));
        message.extend(content_into_segments(content));

        self.send_channel_segments(target.channel_id, message).await
    }

    async fn send_channel(
        &self,
        channel_id: String,
        content: MessageContent,
    ) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        self.send_channel_segments(channel_id, content_into_segments(content))
            .await
    }

    async fn send_direct(&self, actor_id: String, content: MessageContent) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let user_id = actor_id
            .parse::<i64>()
            .context("actor id must be a valid OneBot user id")?;
        let payload = ApiPayload::SendPrivateMsg(SendPrivateMsg {
            user_id,
            message: content_into_segments(content),
            auto_escape: false,
        });

        self.connect.clone().call_api(payload).await?;

        Ok(())
    }

    async fn send_channel_segments(
        &self,
        channel_id: String,
        message: Vec<MessageSegment>,
    ) -> anyhow::Result<()> {
        let group_id = channel_id
            .parse::<i64>()
            .context("channel id must be a valid OneBot group id")?;
        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message,
            auto_escape: false,
        });

        self.connect.clone().call_api(payload).await?;

        Ok(())
    }
}

pub fn channel_message_from_event(event: Result<Event, RecvError>) -> Option<ChannelMessage> {
    receive_event(event).and_then(extract_channel_message)
}

fn receive_event(event: Result<Event, RecvError>) -> Option<Event> {
    if let Err(error) = &event {
        tracing::error!("failed to receive event: {}", error);
        return None;
    }

    event.ok()
}

fn extract_channel_message(event: Event) -> Option<ChannelMessage> {
    match event {
        Event::Message(OneBotMessage::GroupMessage(message)) => {
            Some(channel_message_from_onebot_message(message))
        }
        _ => None,
    }
}

fn channel_message_from_onebot_message(message: GroupMessage) -> ChannelMessage {
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
        MessagePart::Image {
            data: ImageData::Base64(image_base64),
        } => Some(MessageSegment::easy_image(
            format!("base64://{}", image_base64),
            None::<String>,
        )),
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

#[cfg(test)]
mod tests {
    use super::*;
    use onebot_v11::event::message::GroupMessageSender;

    #[test]
    fn onebot_message_maps_to_channel_message() {
        let message = GroupMessage {
            time: 1_700_000_000,
            self_id: 100,
            post_type: "message".to_string(),
            message_type: "group".to_string(),
            sub_type: "normal".to_string(),
            message_id: 10,
            group_id: 200,
            user_id: 300,
            anonymous: None,
            message: vec![
                MessageSegment::reply("9"),
                MessageSegment::at("100"),
                MessageSegment::text(" hello"),
            ],
            raw_message: "[CQ:at,qq=100] hello".to_string(),
            font: 0,
            sender: GroupMessageSender {
                user_id: Some(300),
                nickname: Some("Alice".to_string()),
                card: Some("A".to_string()),
                sex: None,
                age: None,
                area: None,
                level: None,
                role: None,
                title: None,
            },
        };

        let mapped = channel_message_from_onebot_message(message);

        assert_eq!(mapped.self_id, "100");
        assert_eq!(mapped.message_id, "10");
        assert_eq!(mapped.channel_id, "200");
        assert_eq!(mapped.actor.id, "300");
        assert_eq!(mapped.actor.nickname, "Alice");
        assert_eq!(mapped.actor.channel_nickname.as_deref(), Some("A"));
        assert_eq!(
            mapped.content.parts,
            vec![
                MessagePart::Reply {
                    message_id: "9".to_string()
                },
                MessagePart::Mention {
                    actor_id: "100".to_string()
                },
                MessagePart::Text(" hello".to_string())
            ]
        );
    }

    #[test]
    fn image_part_maps_to_base64_image() {
        let segments = content_into_segments(MessageContent {
            parts: vec![MessagePart::Image {
                data: ImageData::Base64("abc".to_string()),
            }],
        });

        match &segments[0] {
            MessageSegment::Image { data } => assert_eq!(data.file, "base64://abc"),
            other => panic!("unexpected segment: {:?}", other),
        }
    }
}
