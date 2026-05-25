use crate::bot::message::Message;
use crate::bot::state::BotState;

use crate::bot::agent::BotAgent;

pub async fn handle_group_message(_: BotState, msg: Message) -> anyhow::Result<Option<Message>> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    let user_text = extract_prk_text(&msg);
    if user_text.is_empty() {
        return Ok(None);
    }

    let mut agent = BotAgent::new();

    let reply = agent.respond(&user_text).await;

    match reply {
        Some(text) => Ok(Some(Message::text(text))),
        None => Ok(None),
    }
}

fn extract_prk_text(msg: &Message) -> String {
    let raw = msg.raw_text();
    let prefix = "/prk";

    let after_prefix = if let Some(rest) = raw.strip_prefix(prefix) {
        rest.trim_start()
    } else {
        return String::new();
    };

    if after_prefix.is_empty() {
        return String::from("你好");
    }

    after_prefix.to_string()
}
