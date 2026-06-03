use openai_oxide::types::chat::ChatCompletionMessageParam;
use openai_oxide::types::chat::FunctionCall;
use openai_oxide::types::chat::ToolCall as OpenAiToolCall;
use openai_oxide::types::chat::UserContent;

use crate::ai::agent::persist::entity::ContextSnapshot;
use crate::ai::agent::persist::entity::Message;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageRef;

pub trait MessageSnapshotCodec<M>
where
    M: IMessage + Clone + 'static,
{
    fn encode_message(&self, message: &M) -> anyhow::Result<Message>;

    fn decode_message(&self, message: &Message) -> anyhow::Result<M>;
}

pub trait ContextSnapshotCodec<M>: MessageSnapshotCodec<M>
where
    M: IMessage + Clone + 'static,
{
    fn encode_context(&self, context: &Context<M>) -> anyhow::Result<ContextSnapshot> {
        let messages = context
            .messages()
            .iter()
            .map(|message| self.encode_message(message))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(ContextSnapshot {
            model: context.model().to_string(),
            messages,
        })
    }

    fn decode_context(&self, snapshot: &ContextSnapshot) -> anyhow::Result<Context<M>> {
        let messages = snapshot
            .messages
            .iter()
            .map(|message| self.decode_message(message))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(ContextBuilder::new(snapshot.model.clone())
            .messages(messages)
            .build())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiCodec;

impl MessageSnapshotCodec<ChatCompletionMessageParam> for OpenAiCodec {
    fn encode_message(&self, message: &ChatCompletionMessageParam) -> anyhow::Result<Message> {
        match message {
            ChatCompletionMessageParam::System { content, .. } => Ok(Message::System {
                content: content.clone(),
            }),
            ChatCompletionMessageParam::User { content, .. } => match content {
                UserContent::Text(text) => Ok(Message::User {
                    content: text.clone(),
                }),
                _ => Err(anyhow::anyhow!(
                    "cannot persist non-text OpenAI user message"
                )),
            },
            ChatCompletionMessageParam::Assistant {
                content,
                tool_calls,
                refusal,
                ..
            } => Ok(Message::Assistant {
                content: content.clone(),
                refusal: refusal.clone(),
                tool_calls: tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|call| crate::ai::agent::persist::entity::ToolCall {
                            id: call.id.clone(),
                            name: call.function.name.clone(),
                            args: call.function.arguments.clone(),
                        })
                        .collect()
                }),
            }),
            ChatCompletionMessageParam::Tool {
                tool_call_id,
                content,
            } => Ok(Message::Tool {
                tool_call_id: tool_call_id.clone(),
                content: content.clone(),
            }),
            ChatCompletionMessageParam::Developer { .. } => Err(anyhow::anyhow!(
                "cannot persist OpenAI developer message as agent message"
            )),
            _ => Err(anyhow::anyhow!(
                "cannot persist unsupported OpenAI message variant"
            )),
        }
    }

    fn decode_message(&self, message: &Message) -> anyhow::Result<ChatCompletionMessageParam> {
        match message {
            Message::System { content } => Ok(ChatCompletionMessageParam::System {
                content: content.clone(),
                name: None,
            }),
            Message::User { content } => Ok(ChatCompletionMessageParam::from(MessageRef::User {
                content: content.as_str(),
            })),
            Message::Assistant {
                content,
                refusal,
                tool_calls,
            } => Ok(ChatCompletionMessageParam::Assistant {
                content: content.clone(),
                name: None,
                tool_calls: tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|call| OpenAiToolCall {
                            id: call.id.clone(),
                            type_: "function".to_string(),
                            function: FunctionCall {
                                name: call.name.clone(),
                                arguments: call.args.clone(),
                            },
                        })
                        .collect()
                }),
                refusal: refusal.clone(),
            }),
            Message::Tool {
                tool_call_id,
                content,
            } => Ok(ChatCompletionMessageParam::Tool {
                tool_call_id: tool_call_id.clone(),
                content: content.clone(),
            }),
        }
    }
}

impl<M, C> ContextSnapshotCodec<M> for C
where
    M: IMessage + Clone + 'static,
    C: MessageSnapshotCodec<M>,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_message_round_trip_preserves_tool_calls() {
        let codec = OpenAiCodec;
        let original = ChatCompletionMessageParam::Assistant {
            content: Some("calling tool".to_string()),
            name: None,
            tool_calls: Some(vec![OpenAiToolCall {
                id: "call_1".to_string(),
                type_: "function".to_string(),
                function: FunctionCall {
                    name: "recall_memory_shard".to_string(),
                    arguments: "{\"shard_name\":\"dev-team\"}".to_string(),
                },
            }]),
            refusal: None,
        };

        let persisted = codec.encode_message(&original).unwrap();
        let restored = codec.decode_message(&persisted).unwrap();

        match restored {
            ChatCompletionMessageParam::Assistant {
                content,
                tool_calls,
                refusal,
                ..
            } => {
                assert_eq!(content.as_deref(), Some("calling tool"));
                assert_eq!(refusal, None);
                let calls = tool_calls.expect("tool calls should be restored");
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_1");
                assert_eq!(calls[0].function.name, "recall_memory_shard");
                assert_eq!(calls[0].function.arguments, "{\"shard_name\":\"dev-team\"}");
            }
            other => panic!("unexpected restored message: {:?}", other),
        }
    }

    #[test]
    fn openai_context_snapshot_excludes_tool_defs() {
        let codec = OpenAiCodec;
        let context = ContextBuilder::new("deepseek-v4-flash")
            .messages(vec![
                ChatCompletionMessageParam::System {
                    content: "system".to_string(),
                    name: None,
                },
                ChatCompletionMessageParam::User {
                    content: UserContent::Text("hello".to_string()),
                    name: None,
                },
            ])
            .build();

        let snapshot = codec.encode_context(&context).unwrap();

        assert_eq!(snapshot.model, "deepseek-v4-flash");
        assert_eq!(snapshot.messages.len(), 2);
    }
}
