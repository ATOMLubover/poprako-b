use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessage {
    pub self_id: String,
    pub message_id: String,
    pub channel_id: String,
    pub actor: MessageActor,
    pub sent_at: OffsetDateTime,
    pub raw_text: String,
    pub content: MessageContent,
}

impl ChannelMessage {
    pub fn reply_target(&self) -> ReplyTarget {
        ReplyTarget {
            channel_id: self.channel_id.clone(),
            message_id: self.message_id.clone(),
        }
    }

    pub fn is_pure_text(&self) -> bool {
        self.content.is_pure_text()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageActor {
    pub id: String,
    pub nickname: String,
    pub channel_nickname: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageContent {
    pub parts: Vec<MessagePart>,
}

impl MessageContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            parts: vec![MessagePart::Text(text.into())],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_pure_text(&self) -> bool {
        self.parts
            .iter()
            .all(|part| matches!(part, MessagePart::Text(_)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePart {
    Text(String),
    Mention { actor_id: String },
    Reply { message_id: String },
    Image { data: ImageData },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
    Base64(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTarget {
    pub channel_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommand {
    ReplyTo {
        target: ReplyTarget,
        content: MessageContent,
    },
    SendChannel {
        channel_id: String,
        content: MessageContent,
    },
    SendDirect {
        actor_id: String,
        content: MessageContent,
    },
}

impl BotCommand {
    pub fn reply_text(target: ReplyTarget, text: impl Into<String>) -> Self {
        Self::ReplyTo {
            target,
            content: MessageContent::text(text),
        }
    }

    pub fn channel_text(channel_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::SendChannel {
            channel_id: channel_id.into(),
            content: MessageContent::text(text),
        }
    }

    pub fn direct_text(actor_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::SendDirect {
            actor_id: actor_id.into(),
            content: MessageContent::text(text),
        }
    }
}
