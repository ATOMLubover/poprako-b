use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ChatMessageMeta {
    channel_id: String,
    channel_name: String,

    sender_id: String,
    sender_nickname: String,
    sender_channel_nickname: Option<String>,
    sender_prks_id: Option<String>,

    sent_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    meta: ChatMessageMeta,
    content: String,
}

impl ChatMessageMeta {
    pub fn new(
        channel_id: impl Into<String>,
        channel_name: impl Into<String>,
        sender_id: impl Into<String>,
        sender_nickname: impl Into<String>,
        sender_channel_nickname: Option<String>,
        sender_prks_id: Option<String>,
        sent_at: OffsetDateTime,
    ) -> Self {
        Self {
            channel_id: channel_id.into(),
            channel_name: channel_name.into(),
            sender_id: sender_id.into(),
            sender_nickname: sender_nickname.into(),
            sender_channel_nickname,
            sender_prks_id,
            sent_at,
        }
    }

    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    pub fn channel_name(&self) -> &str {
        &self.channel_name
    }

    pub fn sender_id(&self) -> &str {
        &self.sender_id
    }

    pub fn sender_nickname(&self) -> &str {
        &self.sender_nickname
    }

    pub fn sender_channel_nickname(&self) -> Option<&str> {
        self.sender_channel_nickname.as_deref()
    }

    pub fn sender_prks_id(&self) -> Option<&str> {
        self.sender_prks_id.as_deref()
    }

    pub fn sent_at(&self) -> OffsetDateTime {
        self.sent_at
    }
}

impl ChatMessage {
    pub fn new(meta: ChatMessageMeta, content: impl Into<String>) -> Self {
        Self {
            meta,
            content: content.into(),
        }
    }

    pub fn meta(&self) -> &ChatMessageMeta {
        &self.meta
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn into_prompt_text(self) -> String {
        let channel_name = if self.meta.channel_name.is_empty() {
            "-"
        } else {
            &self.meta.channel_name
        };

        format!(
            "[channel_id: {}, channel_name: {}, sender_id: {}, sender_nickname: {}, sender_channel_nickname: {}, sender_prks_id: {}, sent_at: {}]\n{}",
            self.meta.channel_id,
            channel_name,
            self.meta.sender_id,
            self.meta.sender_nickname,
            self.meta.sender_channel_nickname.as_deref().unwrap_or("-"),
            self.meta.sender_prks_id.as_deref().unwrap_or("-"),
            self.meta.sent_at,
            self.content
        )
    }
}
