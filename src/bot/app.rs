use crate::bot::event::{BotEvent, ReviewFollowupEvent};
use crate::bot::keepalive::KeepaliveTrigger;
use crate::bot::message::{BotCommand, ChannelMessage, ImageData, MessageContent, MessagePart};
use crate::bot::policy::repeat::try_repeat;
use crate::bot::policy::reply::split_reply_to_command;
use crate::bot::policy::trigger::extract_user_text;
use crate::bot::scheduled_task::ScheduledSpamTrigger;
use crate::bot::state::BotState;

fn image_content(image_base64: String) -> MessageContent {
    MessageContent {
        parts: vec![MessagePart::Image {
            data: ImageData::Base64(image_base64),
        }],
    }
}

fn respond_id(msg: &ChannelMessage, user_text: &str) -> String {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    hasher.update(msg.channel_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(msg.message_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(msg.actor.id.as_bytes());
    hasher.update(b"\0");
    hasher.update(msg.sent_at.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(user_text.as_bytes());

    let hash = hasher.finalize();
    format!("#{:02x}{:02x}{:02x}", hash[0], hash[1], hash[2])
}

pub struct BotApp {
    state: BotState,
}

impl BotApp {
    pub fn new(state: BotState) -> Self {
        Self { state }
    }

    pub async fn handle(&mut self, event: BotEvent) -> Vec<BotCommand> {
        match event {
            BotEvent::ChannelMessage(message) => self.handle_channel_message(message).await,
            BotEvent::SystemPromptRefresh(content) => self.handle_system_prompt_refresh(content),
            BotEvent::ScheduledSpam(trigger) => self.handle_scheduled_spam_trigger(trigger),
            BotEvent::Keepalive(trigger) => self.handle_keepalive_trigger(trigger),
            BotEvent::ReviewFollowup(event) => self.handle_review_followup(event).await,
        }
    }

    async fn handle_channel_message(&mut self, msg: ChannelMessage) -> Vec<BotCommand> {
        tracing::debug!(
            channel_id = msg.channel_id.as_str(),
            actor_id = msg.actor.id.as_str(),
            raw_message = msg.raw_text.as_str(),
            "received channel message"
        );

        if self.state.is_self(&msg.actor.id) {
            return Vec::new();
        }

        if msg.is_pure_text() {
            self.state.push_history_text(msg.raw_text.clone());
        }

        if let Some(text) = try_repeat(self.state.repeat_mut(), &msg) {
            return vec![BotCommand::channel_text(msg.channel_id.clone(), text)];
        }

        self.agent_respond(msg).await
    }

    async fn agent_respond(&mut self, msg: ChannelMessage) -> Vec<BotCommand> {
        let user_text = match extract_user_text(&msg) {
            Some(text) => text,
            None => return Vec::new(),
        };

        let user_text = if self.state.is_developer(&msg.actor.id) {
            format!("[开发者] {}", user_text)
        } else {
            user_text
        };

        let respond_id = respond_id(&msg, &user_text);
        let reply_target = msg.reply_target();
        let channel_id = msg.channel_id.clone();

        let text = self
            .state
            .agent_mut()
            .respond(msg, user_text, respond_id)
            .await
            .unwrap_or_else(|| "X﹏X 白杨子可能出现了点问题，无法回答这个问题哦".to_string());

        split_reply_to_command(reply_target, channel_id, text)
    }

    async fn handle_review_followup(&mut self, event: ReviewFollowupEvent) -> Vec<BotCommand> {
        let channel_id = event.channel_id.clone();
        let Some(text) = self.state.agent_mut().respond_review_feedback(event).await else {
            return Vec::new();
        };

        text.split("\n\n")
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|chunk| BotCommand::channel_text(channel_id.clone(), chunk.to_string()))
            .collect()
    }

    fn handle_system_prompt_refresh(&mut self, content: String) -> Vec<BotCommand> {
        self.state.agent_mut().reload_system_prompt(content);
        Vec::new()
    }

    fn handle_scheduled_spam_trigger(&mut self, trigger: ScheduledSpamTrigger) -> Vec<BotCommand> {
        match trigger {
            ScheduledSpamTrigger::Boot { image_base64 } => {
                vec![BotCommand::SendDirect {
                    actor_id: self.state.self_id().to_string(),
                    content: image_content(image_base64),
                }]
            }
            ScheduledSpamTrigger::Midnight {
                channel_ids,
                image_base64,
            } => channel_ids
                .into_iter()
                .map(|channel_id| BotCommand::SendChannel {
                    channel_id,
                    content: image_content(image_base64.clone()),
                })
                .collect(),
        }
    }

    fn handle_keepalive_trigger(&mut self, trigger: KeepaliveTrigger) -> Vec<BotCommand> {
        trigger
            .texts
            .into_iter()
            .map(|text| BotCommand::direct_text(self.state.self_id(), text))
            .collect()
    }
}
