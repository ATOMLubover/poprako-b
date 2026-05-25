use openai_oxide::types::chat::{ChatCompletionMessageParam, ToolCall as OxToolCall, UserContent};

use crate::ai::resolver::action::Action;
use crate::ai::resolver::message::{IMessage, MessageRef};

pub trait IOpenAiMessage: IMessage<ToolCall = OxToolCall> {}

impl From<Action<OxToolCall>> for ChatCompletionMessageParam {
    fn from(value: Action<OxToolCall>) -> Self {
        let tool_calls = value.tool_calls.map(|calls| calls.into_iter().collect());

        ChatCompletionMessageParam::Assistant {
            content: value.content,
            name: None,
            tool_calls,
            refusal: value.refusal,
        }
    }
}

impl<'a> From<MessageRef<'a, OxToolCall>> for ChatCompletionMessageParam {
    fn from(value: MessageRef<'a, OxToolCall>) -> Self {
        match value {
            MessageRef::System { content } => ChatCompletionMessageParam::System {
                content: content.to_string(),
                name: None,
            },
            MessageRef::User { content } => ChatCompletionMessageParam::User {
                content: UserContent::Text(content.to_string()),
                name: None,
            },
            MessageRef::Assist {
                content,
                tool_calls,
                refusal,
            } => ChatCompletionMessageParam::Assistant {
                content: content.map(str::to_string),
                name: None,
                tool_calls: tool_calls.map(|calls| calls.to_vec()),
                refusal: refusal.map(str::to_string),
            },
            MessageRef::Tool {
                tool_call_id,
                content,
            } => ChatCompletionMessageParam::Tool {
                tool_call_id: tool_call_id.to_string(),
                content: content.to_string(),
            },
        }
    }
}

impl IMessage for ChatCompletionMessageParam {
    type ToolCall = OxToolCall;

    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall> {
        match self {
            ChatCompletionMessageParam::System { content, .. } => MessageRef::System {
                content: content.as_str(),
            },
            ChatCompletionMessageParam::User { content, .. } => {
                let content = match content {
                    UserContent::Text(text) => text.as_str(),
                    _ => "",
                };

                MessageRef::User { content }
            }
            ChatCompletionMessageParam::Assistant {
                content,
                tool_calls,
                refusal,
                ..
            } => MessageRef::Assist {
                content: content.as_deref(),
                tool_calls: tool_calls.as_deref(),
                refusal: refusal.as_deref(),
            },
            ChatCompletionMessageParam::Tool {
                tool_call_id,
                content,
            } => MessageRef::Tool {
                tool_call_id: tool_call_id.as_str(),
                content: content.as_str(),
            },
            _ => MessageRef::System { content: "" },
        }
    }
}

impl IOpenAiMessage for ChatCompletionMessageParam {}
