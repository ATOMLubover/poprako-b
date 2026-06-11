use rand::Rng;

use crate::bot::message::ChannelMessage;
use crate::bot::state::RepeatState;

fn can_repeat(state: &RepeatState, msg: &ChannelMessage) -> bool {
    let history = state.history();

    if history.len() < 3 {
        return false;
    }

    let raw_text = msg.raw_text.as_str();
    if raw_text.is_empty() {
        return false;
    }

    if !history.iter().all(|text| text == raw_text) {
        return false;
    }

    if !msg.is_pure_text() {
        return false;
    }

    state.last_repeat() != Some(raw_text)
}

pub fn try_repeat(state: &mut RepeatState, msg: &ChannelMessage) -> Option<String> {
    if !can_repeat(state, msg) {
        return None;
    }

    if !rand::thread_rng().gen_ratio(1, 2) {
        return None;
    }

    let text = msg.raw_text.clone();
    state.set_last_repeat(text.clone());

    tracing::info!("repeating '{}'", text);

    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    use time::OffsetDateTime;

    use crate::bot::message::MessageActor;
    use crate::bot::message::MessageContent;

    fn message(raw_text: &str) -> ChannelMessage {
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
            content: MessageContent::text(raw_text),
        }
    }

    #[test]
    fn repeat_requires_three_same_messages() {
        let mut state = RepeatState::new();
        state.push_text("hello".to_string());
        state.push_text("hello".to_string());

        assert!(!can_repeat(&state, &message("hello")));

        state.push_text("hello".to_string());

        assert!(can_repeat(&state, &message("hello")));
    }

    #[test]
    fn repeat_rejects_last_repeated_text() {
        let mut state = RepeatState::new();
        state.push_text("hello".to_string());
        state.push_text("hello".to_string());
        state.push_text("hello".to_string());
        state.set_last_repeat("hello".to_string());

        assert!(!can_repeat(&state, &message("hello")));
    }
}
