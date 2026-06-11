use crate::bot::keepalive::KeepaliveTrigger;
use crate::bot::message::ChannelMessage;
use crate::bot::scheduled_task::ScheduledSpamTrigger;

pub struct ReviewFollowupEvent {
    pub channel_id: String,
    pub respond_id: String,
    pub feedback: String,
    pub target_summary: String,
}

pub enum BotEvent {
    ChannelMessage(ChannelMessage),
    SystemPromptRefresh(String),
    ScheduledSpam(ScheduledSpamTrigger),
    Keepalive(KeepaliveTrigger),
    ReviewFollowup(ReviewFollowupEvent),
}

impl BotEvent {
    pub fn should_delay_response(&self) -> bool {
        matches!(self, Self::ChannelMessage(_))
    }
}
