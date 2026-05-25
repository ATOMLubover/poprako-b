use onebot_v11::MessageSegment;
use onebot_v11::event::message::GroupMessage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    self_id: Option<i64>,
    message_id: Option<i64>,
    group_id: Option<i64>,
    user_id: Option<i64>,
    raw_message: Option<String>,
    segments: Vec<MessageSegment>,
}

impl Message {
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

    pub fn raw_text(&self) -> &str {
        self.raw_message.as_deref().unwrap_or("")
    }

    pub fn segments(&self) -> &[MessageSegment] {
        &self.segments
    }

    pub fn into_segments(self) -> Vec<MessageSegment> {
        self.segments
    }

    pub(crate) fn from_group_message(group_message: GroupMessage) -> Self {
        Self {
            self_id: Some(group_message.self_id),
            message_id: Some(group_message.message_id),
            group_id: Some(group_message.group_id),
            user_id: Some(group_message.user_id),
            raw_message: Some(group_message.raw_message),
            segments: group_message.message,
        }
    }
}
