use rand::Rng;

use crate::bot::message::ChannelMessage;
use crate::bot::message::MessagePart;
use crate::bot::message::SendMessage;
use crate::bot::state::BotState;
use crate::bot::value_object::ChatMessage;
use crate::bot::value_object::ChatMessageMeta;

pub async fn handle_channel_message(state: &mut BotState, msg: ChannelMessage) -> Vec<SendMessage> {
    tracing::info!(
        channel_id = msg.channel_id,
        actor_id = msg.actor.id,
        raw_message = msg.raw_text,
        "received channel message"
    );

    let reply = repeat(state, &msg);
    if !reply.is_empty() {
        return reply;
    }

    bot_respond(state, msg).await
}

fn repeat(state: &mut BotState, msg: &ChannelMessage) -> Vec<SendMessage> {
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

    vec![SendMessage::text(false, raw_text)]
}

async fn bot_respond(state: &mut BotState, msg: ChannelMessage) -> Vec<SendMessage> {
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

    let chat_message = ChatMessage::new(
        ChatMessageMeta::new(
            msg.channel_id,
            "",
            msg.actor.id,
            msg.actor.nickname,
            msg.actor.channel_nickname,
            None,
            msg.sent_at,
        ),
        user_text,
    );

    let text = state
        .agent_mut()
        .try_answer(chat_message)
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
            SendMessage::text(reply, chunk.to_string())
        })
        .collect()
}

fn extract_user_text(msg: &ChannelMessage) -> Option<String> {
    // Try @bot at beginning first
    if let Some(text) = try_extract_at(msg, &msg.self_id) {
        return Some(text);
    }

    // Fall back to /prk prefix
    try_extract_prk(msg)
}

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
