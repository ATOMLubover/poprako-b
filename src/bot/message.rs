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
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendMessage {
    pub reply: bool,
    pub content: MessageContent,
}

impl SendMessage {
    pub fn new(reply: bool, content: MessageContent) -> Self {
        Self { reply, content }
    }

    pub fn text(reply: bool, text: impl Into<String>) -> Self {
        Self::new(reply, MessageContent::text(text))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTarget {
    pub channel_id: String,
    pub message_id: String,
}
