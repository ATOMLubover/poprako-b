use crate::bot::message::Message;
use crate::bot::state::BotState;

pub async fn handle_group_message(
    _state: BotState,
    msg: Message,
) -> anyhow::Result<Option<Message>> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    if is_bot_request(&msg) {
        return Ok(Some(Message::text("你好喵")));
    }

    Ok(None)
}

fn is_bot_request(msg: &Message) -> bool {
    if !msg.raw_text().starts_with("/prk ") {
        return false;
    }

    msg.segments()
        .iter()
        .all(|s| matches!(s, onebot_v11::MessageSegment::Text { .. }))
}
