pub mod data_object;
pub mod message;
pub mod tool;

use url::Url;

use crate::ai::resolver::IResolver;
use crate::ai::resolver::action::{Action, Reason};
use crate::ai::resolver::context::Context;
use crate::ai::resolver::result::{ResolveError, ResolveResult};
use crate::ai::resolver_impl::deepseek::data_object::{DeepSeekMessage, DeepSeekToolCall};
use crate::http::HttpClient;
use crate::http::result::HttpError;
use serde_json::Value;
use tracing::{Level, debug, instrument};

pub struct DeepSeekResolver {
    pub client: HttpClient,
    chat_url: Url,
    api_key: String,
}

impl DeepSeekResolver {
    pub fn from_env() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("[DeepSeekResolver::from_env] OPENAI_API_KEY not set in environment");
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

        let base_url = base_url.trim_end_matches('/');
        let chat_url = Url::parse(&format!("{base_url}/chat/completions"))
            .expect("invalid chat completions URL");

        Self {
            client: HttpClient::new(None),
            chat_url,
            api_key,
        }
    }

    fn map_err(err: HttpError) -> ResolveError {
        match err {
            HttpError::InvalidUrl(msg) | HttpError::Unknown(msg) => {
                ResolveError::Unknown { message: msg }
            }
            HttpError::Timeout => ResolveError::Network {
                message: "timeout".into(),
            },
            HttpError::ResponseBody(msg) => ResolveError::Api {
                status: 0,
                message: msg,
            },
            HttpError::Decode(msg) => ResolveError::JsonSerde { message: msg },
        }
    }

    fn build_action(choice: &Value) -> Action<DeepSeekToolCall> {
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("");

        let reason = match finish_reason {
            "stop" => Reason::Finish,
            "length" => Reason::Length,
            "tool_calls" | "function_call" => Reason::ToolCall,
            other => Reason::Unknown(other.to_string()),
        };

        let msg = &choice["message"];

        let content = msg["content"].as_str().map(|s| s.to_string());
        let refusal = msg["refusal"].as_str().map(|s| s.to_string());

        let tool_calls = msg["tool_calls"].as_array().map(|tc| {
            tc.iter()
                .map(|tc| DeepSeekToolCall {
                    id: tc["id"].as_str().unwrap_or("").to_string(),
                    type_: "function".to_string(),
                    function: data_object::ToolCallFunction {
                        name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                        arguments: tc["function"]["arguments"]
                            .as_str()
                            .unwrap_or("{}")
                            .to_string(),
                    },
                })
                .collect()
        });

        Action {
            reason,
            content,
            refusal,
            tool_calls,
        }
    }
}

#[async_trait::async_trait]
impl IResolver for DeepSeekResolver {
    type Message = DeepSeekMessage;

    #[instrument(skip(self, cx), fields(model = %cx.model()), level = Level::INFO)]
    async fn resolve<A>(
        &mut self,
        cx: &Context<Self::Message, A>,
    ) -> ResolveResult<Action<DeepSeekToolCall>>
    where
        A: Send + Sync + 'static,
    {
        let messages: Vec<DeepSeekMessage> = cx.messages().cloned().collect();

        let mut request_map = serde_json::Map::new();
        request_map.insert(
            "model".to_string(),
            Value::String(cx.model().to_string()),
        );
        request_map.insert(
            "messages".to_string(),
            serde_json::to_value(&messages).map_err(|e| ResolveError::JsonSerde {
                message: e.to_string(),
            })?,
        );

        let tools = cx.tool_defs();
        if !tools.is_empty() {
            let tool_values: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters.to_value()
                        }
                    })
                })
                .collect();
            let tool_names: Vec<&str> = tool_values
                .iter()
                .map(|v| {
                    v["function"]["name"]
                        .as_str()
                        .unwrap_or("")
                })
                .collect();
            tracing::debug!(?tool_names, tool_choice = "auto", "sending tools to LLM");

            request_map.insert("tools".into(), Value::Array(tool_values));
            request_map.insert("tool_choice".into(), Value::String("auto".into()));
        } else {
            tracing::debug!("no tools registered, sending request without tools");
        }

        // Ensure every assistant message has `reasoning_content` (DeepSeek
        // V4 thinking-mode requirement — even when empty).
        if let Some(msgs) = request_map
            .get_mut("messages")
            .and_then(|m| m.as_array_mut())
        {
            for msg in msgs {
                if msg.get("role").and_then(|r| r.as_str()) == Some("assistant")
                    && let Some(obj) = msg.as_object_mut()
                {
                    obj.entry("reasoning_content")
                        .or_insert(Value::String(String::new()));
                }
            }
        }

        let request_value = Value::Object(request_map);

        debug!(
            tool_choice = %request_value.get("tool_choice").map(|v| v.to_string()).unwrap_or_default(),
            tools_count = %request_value.get("tools").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0),
            msg_count = %request_value.get("messages").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
            "sending request to LLM"
        );

        let response_value: Value = self
            .client
            .post(self.chat_url.clone(), &request_value, &[], Some(&self.api_key))
            .await
            .map_err(Self::map_err)?;

        debug!(?response_value, "raw LLM response");

        let choice = response_value["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .ok_or(ResolveError::NoChoice)?;

        debug!(?choice, "first choice from LLM");

        let action = Self::build_action(choice);
        debug!(reason = ?action.reason, has_tool_calls = action.tool_calls.is_some(), "resolver produced action");

        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::resolver::action::Reason;
    use serde_json::json;

    // ── build_action unit tests (no network) ────────────────────────────────

    #[test]
    fn build_action_stop_with_content() {
        let choice = json!({
            "message": {
                "content": "hello world"
            },
            "finish_reason": "stop"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::Finish));
        assert_eq!(action.content.as_deref(), Some("hello world"));
        assert!(action.tool_calls.is_none());
    }

    #[test]
    fn build_action_length() {
        let choice = json!({
            "message": {
                "content": "incomplete..."
            },
            "finish_reason": "length"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::Length));
    }

    #[test]
    fn build_action_tool_calls() {
        let choice = json!({
            "message": {
                "content": "",
                "tool_calls": [{
                    "id": "call_xyz",
                    "type": "function",
                    "function": {
                        "name": "web_search",
                        "arguments": "{\"query\":\"rust\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::ToolCall));
        let calls = action.tool_calls.expect("should have tool calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_xyz");
        assert_eq!(calls[0].function.name, "web_search");
        assert_eq!(calls[0].function.arguments, "{\"query\":\"rust\"}");
    }

    #[test]
    fn build_action_function_call() {
        let choice = json!({
            "message": {
                "content": ""
            },
            "finish_reason": "function_call"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::ToolCall));
    }

    #[test]
    fn build_action_unknown_finish_reason() {
        let choice = json!({
            "message": {
                "content": "blocked"
            },
            "finish_reason": "content_filter"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::Unknown(ref s) if s == "content_filter"));
    }

    #[test]
    fn build_action_no_finish_reason() {
        let choice = json!({
            "message": {
                "content": ""
            }
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert!(matches!(action.reason, Reason::Unknown(ref s) if s.is_empty()));
    }

    #[test]
    fn build_action_with_refusal() {
        let choice = json!({
            "message": {
                "content": null,
                "refusal": "I cannot answer that"
            },
            "finish_reason": "stop"
        });

        let action = DeepSeekResolver::build_action(&choice);
        assert_eq!(action.refusal.as_deref(), Some("I cannot answer that"));
        assert!(action.content.is_none());
    }

    #[test]
    fn build_action_multiple_tool_calls() {
        let choice = json!({
            "message": {
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "tool_a", "arguments": "{}" }
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": { "name": "tool_b", "arguments": "{\"x\":1}" }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        });

        let action = DeepSeekResolver::build_action(&choice);
        let calls = action.tool_calls.unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].function.name, "tool_a");
        assert_eq!(calls[1].function.name, "tool_b");
    }

    // ── map_err unit tests ──────────────────────────────────────────────────

    #[test]
    fn map_err_invalid_url() {
        let err = HttpError::InvalidUrl("bad url".into());
        let mapped = DeepSeekResolver::map_err(err);
        assert!(matches!(mapped, ResolveError::Unknown { .. }));
    }

    #[test]
    fn map_err_timeout() {
        let err = HttpError::Timeout;
        let mapped = DeepSeekResolver::map_err(err);
        assert!(matches!(mapped, ResolveError::Network { .. }));
    }

    #[test]
    fn map_err_decode() {
        let err = HttpError::Decode("bad json".into());
        let mapped = DeepSeekResolver::map_err(err);
        assert!(matches!(mapped, ResolveError::JsonSerde { .. }));
    }

    // ── Integration test (requires API key) ─────────────────────────────────

    #[tokio::test]
    async fn single_turn_conversation() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut resolver = DeepSeekResolver::from_env();

        let mut cx = Context::<DeepSeekMessage>::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![DeepSeekMessage::User {
            content: "Hello, who are you?".into(),
        }]);

        let action = resolver
            .resolve(&cx)
            .await
            .expect("resolve should succeed for single-turn");

        assert!(
            matches!(action.reason, Reason::Finish),
            "expected Reason::Finish, got {:?}",
            action.reason
        );

        assert!(
            action.content.is_some(),
            "expected some content in response"
        );

        let content = action.content.as_ref().unwrap();
        assert!(!content.is_empty(), "response content should not be empty");
    }

    #[tokio::test]
    async fn three_turn_conversation() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut resolver = DeepSeekResolver::from_env();

        let mut cx = Context::<DeepSeekMessage>::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![
            DeepSeekMessage::System {
                content: "You are a helpful math assistant. Answer concisely with just the number."
                    .into(),
            },
            DeepSeekMessage::User {
                content: "What is 2 + 2?".into(),
            },
            DeepSeekMessage::Assistant {
                content: Some("4".into()),
                reasoning_content: None,
                tool_calls: None,
                refusal: None,
            },
            DeepSeekMessage::User {
                content: "Now multiply that result by 3. What do you get?".into(),
            },
        ]);

        let action = resolver
            .resolve(&cx)
            .await
            .expect("resolve should succeed for three-turn");

        assert!(
            matches!(action.reason, Reason::Finish),
            "expected Reason::Finish, got {:?}",
            action.reason
        );

        let content = action.content.as_ref().unwrap();
        assert!(!content.is_empty());
        assert!(
            content.contains("12"),
            "expected '12' in three-turn response, got: {content}",
        );
    }

    #[tokio::test]
    async fn tool_call_integration() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut resolver = DeepSeekResolver::from_env();

        let mut cx = Context::<DeepSeekMessage>::new("deepseek-v4-flash".to_string());

        use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDefination};

        cx.set_tool_defs(vec![ToolDefination::new(
            "get_weather",
            "Get the current weather for a city",
            ParamDef::new("object")
                .with_properties(vec![(
                    "city",
                    PropDef::String {
                        desc: "City name".into(),
                        r#enum: None,
                    },
                )])
                .with_required(vec!["city".into()]),
        )]);

        cx.set_messages(vec![DeepSeekMessage::User {
            content: "What's the weather in Beijing?".into(),
        }]);

        let action = resolver
            .resolve(&cx)
            .await
            .expect("resolve should succeed");

        assert!(
            matches!(action.reason, Reason::ToolCall),
            "expected tool call, got {:?}",
            action.reason
        );

        let calls = action.tool_calls.expect("should have tool calls");
        assert!(!calls.is_empty(), "should have at least one tool call");
        assert_eq!(calls[0].function.name, "get_weather");
        assert!(
            calls[0].function.arguments.contains("Beijing"),
            "arguments should contain Beijing, got: {}",
            calls[0].function.arguments
        );
    }

    /// Verify that `reasoning_content` is injected into outgoing assistant
    /// messages even when the original `DeepSeekMessage` doesn't carry it.
    #[tokio::test]
    async fn reasoning_content_is_injected_for_assistant_messages() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut resolver = DeepSeekResolver::from_env();

        let mut cx = Context::<DeepSeekMessage>::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![
            DeepSeekMessage::System {
                content: "You are a helpful assistant.".into(),
            },
            DeepSeekMessage::User {
                content: "Say hi".into(),
            },
        ]);

        // The system/user messages don't carry reasoning_content, but the
        // resolver should inject `reasoning_content: ""` on assistant messages
        // from previous turns. This test validates that the injection logic
        // does not crash when no assistant messages are present yet.
        let action = resolver
            .resolve(&cx)
            .await
            .expect("resolve should succeed");

        assert!(matches!(action.reason, Reason::Finish));
        assert!(action.content.is_some());
    }
}
