use crate::bot::message::{ChannelMessage, MessagePart};

fn non_empty_trimmed(text: String) -> Option<String> {
    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

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

    let text = iter
        .filter_map(|part| match part {
            MessagePart::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    non_empty_trimmed(text)
}

fn try_extract_prk(msg: &ChannelMessage) -> Option<String> {
    non_empty_trimmed(msg.raw_text.strip_prefix("/prk")?.to_string())
}

pub fn extract_user_text(msg: &ChannelMessage) -> Option<String> {
    if let Some(text) = try_extract_at(msg, &msg.self_id) {
        return Some(text);
    }

    try_extract_prk(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    use time::OffsetDateTime;

    use crate::bot::message::{MessageActor, MessageContent};

    fn message(raw_text: &str, parts: Vec<MessagePart>) -> ChannelMessage {
        ChannelMessage {
            self_id: "100".to_string(),
            message_id: "1".to_string(),
            channel_id: "200".to_string(),
            actor: MessageActor {
                id: "300".to_string(),
                nickname: "Alice".to_string(),
                channel_nickname: None,
            },
            sent_at: OffsetDateTime::now_utc(),
            raw_text: raw_text.to_string(),
            content: MessageContent { parts },
        }
    }

    #[test]
    fn extracts_text_after_leading_mention() {
        let msg = message(
            "[CQ:at,qq=100] hello",
            vec![
                MessagePart::Mention {
                    actor_id: "100".to_string(),
                },
                MessagePart::Text(" hello".to_string()),
            ],
        );

        assert_eq!(extract_user_text(&msg).as_deref(), Some("hello"));
    }

    #[test]
    fn extracts_text_after_prk_prefix() {
        let msg = message(
            "/prk hello",
            vec![MessagePart::Text("/prk hello".to_string())],
        );

        assert_eq!(extract_user_text(&msg).as_deref(), Some("hello"));
    }
}
