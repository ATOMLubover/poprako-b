use crate::bot::message::BotCommand;
use crate::bot::message::ReplyTarget;

pub fn split_reply_to_command(
    reply_target: ReplyTarget,
    channel_id: String,
    text: String,
) -> Vec<BotCommand> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .enumerate()
        .map(|(index, chunk)| {
            if index == 0 {
                BotCommand::reply_text(reply_target.clone(), chunk.to_string())
            } else {
                BotCommand::channel_text(channel_id.clone(), chunk.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_chunk_replies_and_later_chunks_send_to_channel() {
        let target = ReplyTarget {
            channel_id: "200".to_string(),
            message_id: "1".to_string(),
        };

        let commands =
            split_reply_to_command(target.clone(), "200".to_string(), "a\n\nb".to_string());

        assert_eq!(
            commands,
            vec![
                BotCommand::reply_text(target, "a"),
                BotCommand::channel_text("200", "b")
            ]
        );
    }
}
