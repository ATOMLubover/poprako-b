use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ChatMessageMeta {
    group_qid: i64,
    group_name: String,

    sender_qid: i64,
    sender_nickname: String,
    sender_group_nickname: Option<String>,
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
        group_qid: i64,
        group_name: impl Into<String>,
        sender_qid: i64,
        sender_nickname: impl Into<String>,
        sender_group_nickname: Option<String>,
        sender_prks_id: Option<String>,
        sent_at: OffsetDateTime,
    ) -> Self {
        Self {
            group_qid,
            group_name: group_name.into(),
            sender_qid,
            sender_nickname: sender_nickname.into(),
            sender_group_nickname,
            sender_prks_id,
            sent_at,
        }
    }

    pub fn group_qid(&self) -> i64 {
        self.group_qid
    }

    pub fn group_name(&self) -> &str {
        &self.group_name
    }

    pub fn sender_qid(&self) -> i64 {
        self.sender_qid
    }

    pub fn sender_nickname(&self) -> &str {
        &self.sender_nickname
    }

    pub fn sender_group_nickname(&self) -> Option<&str> {
        self.sender_group_nickname.as_deref()
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
        let group_name = if self.meta.group_name.is_empty() {
            "-"
        } else {
            &self.meta.group_name
        };

        format!(
            "[group_qid: {}, group_name: {}, sender_qid: {}, sender_nickname: {}, sender_group_nickname: {}, sender_prks_id: {}, sent_at: {}]\n{}",
            self.meta.group_qid,
            group_name,
            self.meta.sender_qid,
            self.meta.sender_nickname,
            self.meta.sender_group_nickname.as_deref().unwrap_or("-"),
            self.meta.sender_prks_id.as_deref().unwrap_or("-"),
            self.meta.sent_at,
            self.content
        )
    }
}
