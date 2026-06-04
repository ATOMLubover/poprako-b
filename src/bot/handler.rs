use onebot_v11::MessageSegment;

use rand::Rng;

use crate::bot::message::{InputMessage, OutputMessage};
use crate::bot::state::BotState;
use crate::bot::value_object::{ChatMessage, ChatMessageMeta};

pub async fn handle_group_message(state: &mut BotState, msg: &InputMessage) -> Vec<OutputMessage> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    let reply = repeat(state, msg);
    if !reply.is_empty() {
        return reply;
    }

    bot_respond(state, msg).await
}

fn repeat(state: &mut BotState, msg: &InputMessage) -> Vec<OutputMessage> {
    let history = state.history();

    if history.len() < 3 {
        return Vec::new();
    }

    let raw_text = msg.raw_text();
    if raw_text.is_empty() {
        return Vec::new();
    }

    if !history.iter().all(|m| m.raw_text() == raw_text) {
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

    vec![OutputMessage::new(false, InputMessage::text(raw_text))]
}

async fn bot_respond(state: &mut BotState, msg: &InputMessage) -> Vec<OutputMessage> {
    let user_text = match extract_user_text(msg) {
        Some(text) => text,
        None => return Vec::new(),
    };

    let is_dev = msg.user_id().is_some_and(|qid| state.is_developer(qid));

    let user_text = if is_dev {
        format!("[开发者] {}", user_text)
    } else {
        user_text
    };

    let chat_message = ChatMessage::new(
        ChatMessageMeta::new(
            msg.group_id().unwrap_or_default(),
            "",
            msg.user_id().unwrap_or_default(),
            msg.nickname().unwrap_or_default(),
            msg.group_nickname().map(str::to_string),
            None,
            msg.sent_at().unwrap_or_else(current_local_time),
        ),
        user_text,
    );

    let text = state
        .agent_mut()
        .try_respond(chat_message)
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
            OutputMessage::new(reply, InputMessage::text(chunk.to_string()))
        })
        .collect()
}

fn extract_user_text(msg: &InputMessage) -> Option<String> {
    let self_id = msg.self_id()?;

    // Try @bot at beginning first
    if let Some(text) = try_extract_at(msg, self_id) {
        return Some(text);
    }

    // Fall back to /prk prefix
    try_extract_prk(msg)
}

/// Extract text after @bot at the beginning of the message (skipping Reply segments).
fn try_extract_at(msg: &InputMessage, self_id: i64) -> Option<String> {
    let mut iter = msg
        .segments()
        .iter()
        .skip_while(|seg| matches!(seg, MessageSegment::Reply { .. }));

    match iter.next()? {
        MessageSegment::At { data } if data.qq == self_id.to_string() => {}
        _ => return None,
    }

    let text: String = iter
        .filter_map(|seg| match seg {
            MessageSegment::Text { data } => Some(data.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

    if text.is_empty() { None } else { Some(text) }
}

/// Extract text after `/prk` prefix from raw_message.
fn try_extract_prk(msg: &InputMessage) -> Option<String> {
    let text = msg.raw_text().strip_prefix("/prk")?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn current_local_time() -> time::OffsetDateTime {
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    time::OffsetDateTime::now_utc().to_offset(local_offset)
}
