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

    state
        .agent_mut()
        .try_respond(&user_text)
        .await
        .or_else(|| Some("X﹏X 白杨子可能出现了点问题，无法回答这个问题哦".to_string()))
        .map(Message::text)
}

fn extract_user_text(msg: &Message) -> Option<String> {
    let raw = msg.raw_text();
    let prefix = "/prk";

    let after_prefix = match raw.strip_prefix(prefix) {
        Some(text) => text.trim(),
        None => return None,
    };

    Some(after_prefix.to_string())
}
