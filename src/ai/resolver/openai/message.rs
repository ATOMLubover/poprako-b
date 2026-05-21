use openai_oxide::types::chat::{
    ChatCompletionMessageParam, ToolCall as OxToolCall, UserContent,
};

use crate::ai::resolver::action::Action;
use crate::ai::resolver::message::{IMessage, Message};

pub trait OpenAiMessage: IMessage<ToolCall = OxToolCall> {}

impl From<Action> for ChatCompletionMessageParam {
    fn from(value: Action) -> Self {
        let tool_calls = value
            .tool_calls
            .map(|calls| calls.into_iter().map(Into::into).collect());

        ChatCompletionMessageParam::Assistant {
            content: value.content,
            name: None,
            tool_calls,
            refusal: value.refusal,
        }
    }
}

impl IMessage for ChatCompletionMessageParam {
    type ToolCall = OxToolCall;

    fn system(content: &str) -> Self {
        ChatCompletionMessageParam::System {
            content: content.to_string(),
            name: None,
        }
    }

    fn user(content: &str) -> Self {
        ChatCompletionMessageParam::User {
            content: UserContent::Text(content.to_string()),
            name: None,
        }
    }

    fn assistant(
        content: Option<&str>,
        tool_calls: Option<&[Self::ToolCall]>,
        refusal: Option<&str>,
    ) -> Self {
        ChatCompletionMessageParam::Assistant {
            content: content.map(str::to_string),
            name: None,
            tool_calls: tool_calls.map(|calls| calls.to_vec()),
            refusal: refusal.map(str::to_string),
        }
    }

    fn tool(tool_call_id: &str, content: &str) -> Self {
        ChatCompletionMessageParam::Tool {
            tool_call_id: tool_call_id.to_string(),
            content: content.to_string(),
        }
    }

    fn message(&self) -> Message<'_, Self::ToolCall> {
        match self {
            ChatCompletionMessageParam::System { content, .. } => Message::System {
                content: content.as_str(),
            },
            ChatCompletionMessageParam::User { content, .. } => {
                let content = match content {
                    UserContent::Text(text) => text.as_str(),
                    _ => "",
                };

                Message::User { content }
            }
            ChatCompletionMessageParam::Assistant {
                content,
                tool_calls,
                refusal,
                ..
            } => Message::Assistant {
                content: content.as_deref(),
                tool_calls: tool_calls.as_deref(),
                refusal: refusal.as_deref(),
            },
            ChatCompletionMessageParam::Tool {
                tool_call_id,
                content,
            } => Message::Tool {
                tool_call_id: tool_call_id.as_str(),
                content: content.as_str(),
            },
            _ => Message::System { content: "" },
        }
    }
}

impl OpenAiMessage for ChatCompletionMessageParam {}
