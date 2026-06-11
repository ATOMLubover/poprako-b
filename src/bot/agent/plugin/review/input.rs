use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRef};
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::agent::plugin::review::annotation::{IReviewAnnotated, ReviewMessageSource};

pub fn parse_envelope_field(content: &str, field: &str) -> Option<String> {
    let header = content.lines().next()?.trim();
    let header = header.strip_prefix('[')?.strip_suffix(']')?;

    for part in header.split(", ") {
        let (key, value) = part.split_once(": ")?;
        if key == field {
            return Some(value.to_string());
        }
    }

    None
}

pub fn review_channel_id<A>(cx: &Context<DeepSeekMessage, A>, respond_id: &str) -> String
where
    A: IReviewAnnotated + Send + Sync + 'static,
{
    for message in cx.annotated_messages().iter().rev() {
        let ann = message.annotation.review_annotation();
        if ann.respond_id() == Some(respond_id)
            && ann.source() == Some(ReviewMessageSource::NormalUser)
            && let MessageRef::User { content } = message.message.message_ref()
            && let Some(channel_id) = parse_envelope_field(content, "channel_id")
        {
            return channel_id;
        }
    }

    String::new()
}
