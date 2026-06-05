use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRole};

pub type Compact<M> = fn(&mut Context<M>);

pub fn sliding_window_compact<M>(cx: &mut Context<M>)
where
    M: IMessage + 'static,
{
    // Use a MAX_MESSAGES larger than RESERVE_MESSAGES, in casae the agent
    // splits every time a message is pushed when the number of messages is above the limit.
    const MAX_MESSAGES: usize = 80;
    const RESERVE_MESSAGES: usize = 50;

    let len = if let len = cx.messages().len()
        && len > MAX_MESSAGES
    {
        len
    } else {
        return;
    };

    let mut messages = cx.take_messages();

    // Reserve only the latest RESERVE_MESSAGES messages, and drop the rest.
    // NOTE: First message(system prompt) is always reserved,
    // so the agent can keep the system prompt in the context.
    messages.drain(1..len.saturating_sub(RESERVE_MESSAGES));

    // In case the leading message in left part is not user message.
    let user_first = messages
        .iter()
        .skip(1)
        .position(|m| m.role() == MessageRole::User)
        .map(|i| i + 1);

    if let Some(i) = user_first
        && i > 1
    {
        messages.drain(1..i);
    }

    cx.set_messages(messages);
}
