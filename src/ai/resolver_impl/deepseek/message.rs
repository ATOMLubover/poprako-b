use crate::ai::resolver::action::Action;
use crate::ai::resolver::message::{IMessage, MessageOwned, MessageRef};
use crate::ai::resolver_impl::deepseek::data_object::{DeepSeekMessage, DeepSeekToolCall};

// ── IMessage ─────────────────────────────────────────────────────────────────

impl IMessage for DeepSeekMessage {
    type ToolCall = DeepSeekToolCall;

    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall> {
        match self {
            DeepSeekMessage::System { content } => MessageRef::System {
                content: content.as_str(),
            },
            DeepSeekMessage::User { content } => MessageRef::User {
                content: content.as_str(),
            },
            DeepSeekMessage::Assistant {
                content,
                tool_calls,
                refusal,
                ..
            } => MessageRef::Assist {
                content: content.as_deref(),
                tool_calls: tool_calls.as_deref(),
                refusal: refusal.as_deref(),
            },
            DeepSeekMessage::Tool {
                tool_call_id,
                content,
            } => MessageRef::Tool {
                tool_call_id: tool_call_id.as_str(),
                content: content.as_str(),
            },
        }
    }
}

// ── From<Action> ─────────────────────────────────────────────────────────────

impl From<Action<DeepSeekToolCall>> for DeepSeekMessage {
    fn from(action: Action<DeepSeekToolCall>) -> Self {
        DeepSeekMessage::Assistant {
            content: action.content,
            reasoning_content: None,
            tool_calls: action.tool_calls,
            refusal: action.refusal,
        }
    }
}

// ── From<MessageRef> ─────────────────────────────────────────────────────────

impl<'a> From<MessageRef<'a, DeepSeekToolCall>> for DeepSeekMessage {
    fn from(msg: MessageRef<'a, DeepSeekToolCall>) -> Self {
        match msg {
            MessageRef::System { content } => DeepSeekMessage::System {
                content: content.to_string(),
            },
            MessageRef::User { content } => DeepSeekMessage::User {
                content: content.to_string(),
            },
            MessageRef::Assist {
                content,
                tool_calls,
                refusal,
            } => DeepSeekMessage::Assistant {
                content: content.map(str::to_string),
                reasoning_content: None,
                tool_calls: tool_calls.map(|calls| calls.to_vec()),
                refusal: refusal.map(str::to_string),
            },
            MessageRef::Tool {
                tool_call_id,
                content,
            } => DeepSeekMessage::Tool {
                tool_call_id: tool_call_id.to_string(),
                content: content.to_string(),
            },
        }
    }
}

// ── From<MessageOwned> ───────────────────────────────────────────────────────

impl From<MessageOwned<DeepSeekToolCall>> for DeepSeekMessage {
    fn from(msg: MessageOwned<DeepSeekToolCall>) -> Self {
        match msg {
            MessageOwned::System { content } => DeepSeekMessage::System { content },
            MessageOwned::User { content } => DeepSeekMessage::User { content },
            MessageOwned::Assist {
                content,
                tool_calls,
                refusal,
            } => DeepSeekMessage::Assistant {
                content,
                reasoning_content: None,
                tool_calls,
                refusal,
            },
            MessageOwned::Tool {
                tool_call_id,
                content,
            } => DeepSeekMessage::Tool {
                tool_call_id,
                content,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::resolver::action::Reason;
    use crate::ai::resolver::message::MessageRole;
    use crate::ai::resolver::tool::IToolCall;
    use crate::ai::resolver_impl::deepseek::data_object::ToolCallFunction;

    // ── IMessage::role() ─────────────────────────────────────────────────────

    #[test]
    fn role_maps_correctly() {
        let system_msg = DeepSeekMessage::System {
            content: "s".into(),
        };
        assert_eq!(system_msg.role(), MessageRole::System);

        let user_msg = DeepSeekMessage::User {
            content: "u".into(),
        };
        assert_eq!(user_msg.role(), MessageRole::User);

        let assist_msg = DeepSeekMessage::Assistant {
            content: Some("a".into()),
            reasoning_content: None,
            tool_calls: None,
            refusal: None,
        };
        assert_eq!(assist_msg.role(), MessageRole::Assist);

        let tool_msg = DeepSeekMessage::Tool {
            tool_call_id: "id".into(),
            content: "t".into(),
        };
        assert_eq!(tool_msg.role(), MessageRole::Tool);
    }

    // ── IMessage::message_ref() ──────────────────────────────────────────────

    #[test]
    fn message_ref_extracts_content() {
        let msg = DeepSeekMessage::User {
            content: "hi".into(),
        };
        let r#ref = msg.message_ref();
        match r#ref {
            MessageRef::User { content } => assert_eq!(content, "hi"),
            _ => panic!("expected User ref"),
        }
    }

    #[test]
    fn message_ref_extracts_assist_fields() {
        let tc = DeepSeekToolCall {
            id: "call_1".into(),
            type_: "function".into(),
            function: ToolCallFunction {
                name: "my_tool".into(),
                arguments: "{}".into(),
            },
        };

        let msg = DeepSeekMessage::Assistant {
            content: Some("hello".into()),
            reasoning_content: Some("thinking".into()),
            tool_calls: Some(vec![tc]),
            refusal: Some("refused".into()),
        };

        let r#ref = msg.message_ref();
        match r#ref {
            MessageRef::Assist {
                content,
                tool_calls,
                refusal,
            } => {
                assert_eq!(content, Some("hello"));
                assert_eq!(refusal, Some("refused"));
                assert!(tool_calls.is_some());
                assert_eq!(tool_calls.unwrap()[0].name(), "my_tool");
            }
            _ => panic!("expected Assist ref"),
        }
    }

    // ── From<Action> ─────────────────────────────────────────────────────────

    #[test]
    fn action_to_message_preserves_content_and_tool_calls() {
        let action = Action {
            reason: Reason::Finish,
            content: Some("done".into()),
            refusal: None,
            tool_calls: None,
        };

        let msg: DeepSeekMessage = action.into();
        match msg {
            DeepSeekMessage::Assistant {
                content,
                reasoning_content,
                ..
            } => {
                assert_eq!(content.as_deref(), Some("done"));
                assert_eq!(reasoning_content, None);
            }
            _ => panic!("expected Assistant"),
        }
    }

    // ── From<MessageRef> ─────────────────────────────────────────────────────

    #[test]
    fn message_ref_to_owned_round_trip() {
        let owned = DeepSeekMessage::System {
            content: "system prompt".into(),
        };
        let r#ref = owned.message_ref();
        let back: DeepSeekMessage = r#ref.into();

        assert_eq!(back, owned);
    }

    // ── From<MessageOwned> ───────────────────────────────────────────────────

    #[test]
    fn message_owned_to_deepseek_round_trip() {
        let m = MessageOwned::Tool {
            tool_call_id: "id".into(),
            content: "payload".into(),
        };
        let ds: DeepSeekMessage = m.into();
        match ds {
            DeepSeekMessage::Tool {
                tool_call_id,
                content,
            } => {
                assert_eq!(tool_call_id, "id");
                assert_eq!(content, "payload");
            }
            _ => panic!("expected Tool"),
        }
    }
}
