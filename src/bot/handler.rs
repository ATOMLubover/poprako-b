use onebot_v11::MessageSegment;

use crate::bot::message::Message;
use crate::bot::state::BotState;

pub async fn handle_group_message(state: &mut BotState, msg: &Message) -> Option<Message> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    let user_text = match extract_user_text(msg) {
        Some(text) => text,
        None => return None,
    };

    let nickname = msg.nickname().map(|n| n.to_string());

    state
        .agent_mut()
        .try_respond(&user_text, nickname)
        .await
        .or_else(|| Some("X﹏X 白杨子可能出现了点问题，无法回答这个问题哦".to_string()))
        .map(Message::text)
}

fn extract_user_text(msg: &Message) -> Option<String> {
    let self_id = msg.self_id()?;

    // Try @bot at beginning first
    if let Some(text) = try_extract_at(msg, self_id) {
        return Some(text);
    }

    // Fall back to /prk prefix
    try_extract_prk(msg)
}

/// Extract text after @bot at the beginning of the message (skipping Reply segments).
fn try_extract_at(msg: &Message, self_id: i64) -> Option<String> {
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
fn try_extract_prk(msg: &Message) -> Option<String> {
    let text = msg.raw_text().strip_prefix("/prk")?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}
