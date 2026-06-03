use onebot_v11::MessageSegment;
use onebot_v11::event::message::GroupMessage;
use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputMessage {
    self_id: Option<i64>,
    message_id: Option<i64>,
    group_id: Option<i64>,
    user_id: Option<i64>,
    nickname: Option<String>,
    group_nickname: Option<String>,
    sent_at: Option<OffsetDateTime>,
    raw_message: Option<String>,
    segments: Vec<MessageSegment>,
}

impl InputMessage {
    pub fn text(text: impl Into<String>) -> Self {
        Self::from_segments(vec![MessageSegment::text(text)])
    }

    pub fn from_segments(segments: Vec<MessageSegment>) -> Self {
        Self {
            segments,
            ..Self::default()
        }
    }

    pub fn self_id(&self) -> Option<i64> {
        self.self_id
    }

    pub fn message_id(&self) -> Option<i64> {
        self.message_id
    }

    pub fn group_id(&self) -> Option<i64> {
        self.group_id
    }

    pub fn user_id(&self) -> Option<i64> {
        self.user_id
    }

    pub fn nickname(&self) -> Option<&str> {
        self.nickname.as_deref()
    }

    pub fn group_nickname(&self) -> Option<&str> {
        self.group_nickname.as_deref()
    }

    pub fn sent_at(&self) -> Option<OffsetDateTime> {
        self.sent_at
    }

    pub fn raw_text(&self) -> &str {
        self.raw_message.as_deref().unwrap_or("")
    }

    pub fn segments(&self) -> &[MessageSegment] {
        &self.segments
    }

    pub fn into_segments(self) -> Vec<MessageSegment> {
        self.segments
    }

    /// Returns true if all segments are plain text.
    pub fn is_pure_text(&self) -> bool {
        self.segments
            .iter()
            .all(|seg| matches!(seg, MessageSegment::Text { .. }))
    }

    pub fn from_group_message(group_message: GroupMessage) -> Self {
        Self {
            self_id: Some(group_message.self_id),
            message_id: Some(group_message.message_id),
            group_id: Some(group_message.group_id),
            user_id: Some(group_message.user_id),
            nickname: group_message.sender.nickname,
            group_nickname: group_message.sender.card,
            sent_at: OffsetDateTime::from_unix_timestamp(group_message.time)
                .ok()
                .map(to_local_time),
            raw_message: Some(group_message.raw_message),
            segments: group_message.message,
        }
    }
}

fn to_local_time(time: OffsetDateTime) -> OffsetDateTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    time.to_offset(local_offset)
}

pub struct OutputMessage {
    pub reply: bool,
    pub message: InputMessage,
}

impl OutputMessage {
    pub fn new(reply: bool, message: InputMessage) -> Self {
        Self { reply, message }
    }

    pub fn into_segments(self) -> Vec<MessageSegment> {
        self.message.into_segments()
    }

    pub fn segments(&self) -> &[MessageSegment] {
        self.message.segments()
    }
}
