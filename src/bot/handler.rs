use onebot_v11::MessageSegment;

use rand::Rng;

use crate::bot::message::{InputMessage, OutputMessage};
use crate::bot::state::BotState;

pub async fn handle_group_message(state: &mut BotState, msg: &InputMessage) -> Option<OutputMessage> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    if let Some(reply) = repeat(state, msg) {
        return Some(reply);
    }

    bot_respond(state, msg).await
}

fn repeat(state: &BotState, msg: &InputMessage) -> Option<OutputMessage> {
    let history = state.history();

    if history.len() < 3 {
        return None;
    }

    let raw_text = msg.raw_text();
    if raw_text.is_empty() {
        return None;
    }

    if !history.iter().all(|m| m.raw_text() == raw_text) {
        return None;
    }

    // Only repeat pure text messages — avoid serialising CQ codes.
    if !msg.is_pure_text() {
        return None;
    }

    // 80% chance to repeat.
    if !rand::thread_rng().gen_ratio(4, 5) {
        return None;
    }

    Some(OutputMessage::new(false, InputMessage::text(raw_text)))
}

async fn bot_respond(state: &mut BotState, msg: &InputMessage) -> Option<OutputMessage> {
    let user_text = match extract_user_text(msg) {
        Some(text) => text,
        None => return None,
    };

    let nickname = msg.nickname().unwrap_or_default().to_string();
    let user_qid = msg.user_id().map(|id| id.to_string()).unwrap_or_default();

    state
        .agent_mut()
        .try_respond(&nickname, &user_qid, &user_text)
        .await
        .or_else(|| Some("X﹏X 白杨子可能出现了点问题，无法回答这个问题哦".to_string()))
        .map(InputMessage::text)
        .map(|m| OutputMessage::new(true, m))
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
