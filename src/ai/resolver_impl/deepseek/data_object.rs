//! Core data types for the DeepSeek V4 resolver — the equivalents of
//! `ChatCompletionMessageParam` and `ToolCall` from `openai_oxide`.
//!
//! Reference: <docs/api-format/deepseek.md>

use serde::{Deserialize, Serialize};

// ── Thinking mode control ────────────────────────────────────────────────────

/// Controls whether DeepSeek V4 thinking mode is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingType {
    Enabled,
    Disabled,
}

/// The `thinking` field in a DeepSeek V4 request body:
/// `{ "type": "enabled" }` or `{ "type": "disabled" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub r#type: ThinkingType,
}

impl Thinking {
    pub const fn enabled() -> Self {
        Self {
            r#type: ThinkingType::Enabled,
        }
    }

    pub const fn disabled() -> Self {
        Self {
            r#type: ThinkingType::Disabled,
        }
    }
}

// ── Reasoning effort ─────────────────────────────────────────────────────────

/// V4 reasoning intensity control.
///
/// `High` serializes as `"high"`; `Max` serializes as `"max"`.
/// Compatible mappings (per DeepSeek docs): `low`/`medium` → `high`,
/// `xhigh` → `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    High,
    #[serde(rename = "max")]
    Max,
}

// ── Tool call ────────────────────────────────────────────────────────────────

/// Function name + arguments inside a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// A tool call matching the OpenAI-compatible shape used by DeepSeek V4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepSeekToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: ToolCallFunction,
}

// ── Message ──────────────────────────────────────────────────────────────────

/// A conversation message for DeepSeek V4 Chat Completions.
///
/// The assistant variant carries an optional `reasoning_content` field for
/// V4 thinking-mode output. All variants use `"role"` as the JSON tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum DeepSeekMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        /// V4 thinking-mode reasoning text.
        ///
        /// Must be preserved across tool-call turns (see DeepSeek docs on
        /// reasoning-content carry-over rules).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<DeepSeekToolCall>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refusal: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl DeepSeekMessage {
    /// Ensure every assistant message has a `reasoning_content` field (at
    /// least an empty string) to satisfy DeepSeek V4's expected shape.
    ///
    /// This is a no-op if `reasoning_content` is already `Some`.
    pub fn ensure_reasoning_content(&mut self) {
        if let DeepSeekMessage::Assistant {
            reasoning_content, ..
        } = self
            && reasoning_content.is_none()
        {
            *reasoning_content = Some(String::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Thinking / ReasoningEffort ───────────────────────────────────────────

    #[test]
    fn thinking_serializes_correctly() {
        let enabled = Thinking::enabled();
        let json = serde_json::to_value(enabled).unwrap();
        assert_eq!(json, serde_json::json!({"type": "enabled"}));

        let disabled = Thinking::disabled();
        let json = serde_json::to_value(disabled).unwrap();
        assert_eq!(json, serde_json::json!({"type": "disabled"}));
    }

    #[test]
    fn reasoning_effort_serializes() {
        assert_eq!(serde_json::to_value(ReasoningEffort::High).unwrap(), "high");
        assert_eq!(serde_json::to_value(ReasoningEffort::Max).unwrap(), "max");
    }

    // ── Messages ─────────────────────────────────────────────────────────────

    #[test]
    fn message_role_tag() {
        let msg = DeepSeekMessage::Assistant {
            content: Some("hello".into()),
            reasoning_content: None,
            tool_calls: None,
            refusal: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "hello");
    }

    #[test]
    fn message_round_trips_json() {
        let msgs = vec![
            DeepSeekMessage::System {
                content: "sys".into(),
            },
            DeepSeekMessage::User {
                content: "usr".into(),
            },
            DeepSeekMessage::Assistant {
                content: Some("hello".into()),
                reasoning_content: Some("thinking...".into()),
                tool_calls: None,
                refusal: None,
            },
            DeepSeekMessage::Tool {
                tool_call_id: "call_1".into(),
                content: "result".into(),
            },
        ];

        let json = serde_json::to_value(&msgs).unwrap();
        let restored: Vec<DeepSeekMessage> = serde_json::from_value(json).unwrap();

        assert_eq!(restored, msgs);
    }

    #[test]
    fn ensure_reasoning_content_injects_empty_when_none() {
        let mut msg = DeepSeekMessage::Assistant {
            content: Some("hi".into()),
            reasoning_content: None,
            tool_calls: None,
            refusal: None,
        };
        msg.ensure_reasoning_content();

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["reasoning_content"], "");
    }

    #[test]
    fn ensure_reasoning_content_skips_when_present() {
        let mut msg = DeepSeekMessage::Assistant {
            content: Some("hi".into()),
            reasoning_content: Some("real thinking".into()),
            tool_calls: None,
            refusal: None,
        };
        msg.ensure_reasoning_content();

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["reasoning_content"], "real thinking");
    }

    // ── Tool call serde ──────────────────────────────────────────────────────

    #[test]
    fn tool_call_round_trips_json() {
        let tc = DeepSeekToolCall {
            id: "call_1".into(),
            type_: "function".into(),
            function: ToolCallFunction {
                name: "my_tool".into(),
                arguments: "{}".into(),
            },
        };

        let json = serde_json::to_value(&tc).unwrap();
        let restored: DeepSeekToolCall = serde_json::from_value(json).unwrap();

        assert_eq!(restored, tc);
    }
}
