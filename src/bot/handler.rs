use rand::Rng;

use crate::bot::keepalive::KeepaliveTrigger;
use crate::bot::message::BotCommand;
use crate::bot::message::ChannelMessage;
use crate::bot::message::ImageData;
use crate::bot::message::MessageContent;
use crate::bot::message::MessagePart;
use crate::bot::scheduled_task::ScheduledSpamTrigger;
use crate::bot::state::BotState;

/// Extract text after @bot at the beginning of the message (skipping Reply segments).
fn try_extract_at(msg: &ChannelMessage, self_id: &str) -> Option<String> {
    let mut iter = msg
        .content
        .parts
        .iter()
        .skip_while(|part| matches!(part, MessagePart::Reply { .. }));

    match iter.next()? {
        MessagePart::Mention { actor_id } if actor_id == self_id => {}
        _ => return None,
    }

    let text: String = iter
        .filter_map(|part| match part {
            MessagePart::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

    if text.is_empty() { None } else { Some(text) }
}

/// Extract text after `/prk` prefix from raw_message.
fn try_extract_prk(msg: &ChannelMessage) -> Option<String> {
    let text = msg.raw_text.strip_prefix("/prk")?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn extract_user_text(msg: &ChannelMessage) -> Option<String> {
    // Try @bot at beginning first
    if let Some(text) = try_extract_at(msg, &msg.self_id) {
        return Some(text);
    }

    // Fall back to /prk prefix
    try_extract_prk(msg)
}

fn try_repeat(state: &mut BotState, msg: &ChannelMessage) -> Vec<BotCommand> {
    let history = state.history();

    if history.len() < 3 {
        return Vec::new();
    }

    let raw_text = msg.raw_text.as_str();
    if raw_text.is_empty() {
        return Vec::new();
    }

    if !history.iter().all(|text| text == raw_text) {
        return Vec::new();
    }

    // Only repeat pure text messages — avoid serialising CQ codes.
    if !msg.is_pure_text() {
        return Vec::new();
    }

    // Don't repeat the same text the bot already repeated in this chain.
    if state.last_repeat() == Some(raw_text) {
        return Vec::new();
    }

    // 50% chance to repeat.
    if !rand::thread_rng().gen_ratio(1, 2) {
        return Vec::new();
    }

    state.set_last_repeat(raw_text.to_string());

    tracing::info!("repeating '{}'", raw_text);

    vec![BotCommand::channel_text(msg.channel_id.clone(), raw_text)]
}

async fn bot_answer(state: &mut BotState, msg: ChannelMessage) -> Vec<BotCommand> {
    let user_text = match extract_user_text(&msg) {
        Some(text) => text,
        None => return Vec::new(),
    };

    let is_dev = state.is_developer(&msg.actor.id);

    let user_text = if is_dev {
        format!("[开发者] {}", user_text)
    } else {
        user_text
    };

    let reply_target = msg.reply_target();
    let channel_id = msg.channel_id.clone();
    let text = state
        .agent_mut()
        .try_answer(msg, user_text)
        .await
        .unwrap_or_else(|| "X﹏X 白杨子可能出现了点问题，无法回答这个问题哦".to_string());

    // Split by double newline into multiple messages.
    // The first message is a reply to the triggering message;
    // subsequent messages are standalone sends.
    text.split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(i, chunk)| {
            let reply = i == 0;
            if reply {
                BotCommand::reply_text(reply_target.clone(), chunk.to_string())
            } else {
                BotCommand::channel_text(channel_id.clone(), chunk.to_string())
            }
        })
        .collect()
}

pub async fn handle_channel_message(state: &mut BotState, msg: ChannelMessage) -> Vec<BotCommand> {
    tracing::info!(
        channel_id = msg.channel_id.as_str(),
        actor_id = msg.actor.id.as_str(),
        raw_message = msg.raw_text.as_str(),
        "received channel message"
    );

    if state.is_self(&msg.actor.id) {
        return Vec::new();
    }

    if msg.is_pure_text() {
        state.push_history_text(msg.raw_text.clone());
    }

    let reply = try_repeat(state, &msg);
    if !reply.is_empty() {
        return reply;
    }

    bot_answer(state, msg).await
}

pub async fn handle_system_prompt_refresh(
    state: &mut BotState,
    content: String,
) -> Vec<BotCommand> {
    state.agent_mut().reload_system_prompt(content);
    Vec::new()
}

fn image_content(image_base64: String) -> MessageContent {
    MessageContent {
        parts: vec![MessagePart::Image {
            data: ImageData::Base64(image_base64),
        }],
    }
}

pub async fn handle_scheduled_spam_trigger(
    state: &mut BotState,
    trigger: ScheduledSpamTrigger,
) -> Vec<BotCommand> {
    match trigger {
        ScheduledSpamTrigger::Boot { image_base64 } => {
            vec![BotCommand::SendDirect {
                actor_id: state.self_id().to_string(),
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

pub async fn handle_keepalive_trigger(
    state: &mut BotState,
    trigger: KeepaliveTrigger,
) -> Vec<BotCommand> {
    trigger
        .texts
        .into_iter()
        .map(|text| BotCommand::direct_text(state.self_id(), text))
        .collect()
}
