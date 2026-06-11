use crate::bot::keepalive::KeepaliveTrigger;
use crate::bot::message::ChannelMessage;
use crate::bot::scheduled_task::ScheduledSpamTrigger;

pub enum BotEvent {
    ChannelMessage(ChannelMessage),
    SystemPromptRefresh(String),
    ScheduledSpam(ScheduledSpamTrigger),
    Keepalive(KeepaliveTrigger),
}

impl BotEvent {
    pub fn should_delay_response(&self) -> bool {
        matches!(self, Self::ChannelMessage(_))
    }
}
