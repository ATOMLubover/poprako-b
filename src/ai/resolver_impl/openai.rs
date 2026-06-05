// OpenAI Resolver implementation based solely on HTTP.
pub mod context;
pub mod message;
pub mod tool;

use crate::ai::resolver::IResolver;
use crate::ai::resolver::action::{Action, Reason};
use crate::ai::resolver::context::Context;
use crate::ai::resolver::result::{ResolveError, ResolveResult};
use openai_oxide::types::chat::{
    ChatCompletionMessageParam, ChatCompletionRequest, FunctionCall, Tool as OxTool,
    ToolCall as OxToolCall, ToolChoice,
};
use openai_oxide::{ClientConfig, OpenAI, OpenAIError};
use serde_json::Value;
use tracing::{Level, debug, info, instrument};

pub struct OpenAiResolver {
    client: OpenAI,
}

impl OpenAiResolver {
    pub fn from_env() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("[OpenAiResolver::with_env] OPENAI_API_KEY not set in environment");
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        // Strip any trailing slash; openai-oxide concatenates base_url + path directly.
        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            client: OpenAI::with_config(ClientConfig::new(api_key).base_url(base_url)),
        }
    }

    fn map_tool(tool: &crate::ai::resolver::tool::ToolDefination) -> OxTool {
        OxTool::function(&tool.name, &tool.description, tool.parameters.to_value())
    }

    fn map_err(err: OpenAIError) -> ResolveError {
        match err {
            OpenAIError::ApiError {
                status, message, ..
            } => ResolveError::Api { status, message },
            OpenAIError::RequestError(e) => ResolveError::Network {
                message: e.to_string(),
            },
            OpenAIError::JsonError(e) => ResolveError::JsonSerde {
                message: e.to_string(),
            },
            OpenAIError::StreamError(msg) | OpenAIError::InvalidArgument(msg) => {
                ResolveError::Unknown { message: msg }
            }
        }
    }

    fn build_action(choice: &Value) -> Action<OxToolCall> {
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
                .map(|tc| OxToolCall {
                    id: tc["id"].as_str().unwrap_or("").to_string(),
                    type_: "function".to_string(),
                    function: FunctionCall {
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
impl IResolver for OpenAiResolver {
    type Message = ChatCompletionMessageParam;

    #[instrument(skip(self, cx), fields(model = %cx.model()), level = Level::INFO)]
    async fn resolve<A>(
        &mut self,
        cx: &Context<Self::Message, A>,
    ) -> ResolveResult<Action<OxToolCall>>
    where
        A: Send + Sync + 'static,
    {
        let messages: Vec<ChatCompletionMessageParam> = cx.messages().cloned().collect();
        let mut request = ChatCompletionRequest::new(cx.model().to_string(), messages);

        let tools = cx.tool_defs();
        if !tools.is_empty() {
            let ox_tools: Vec<OxTool> = tools.iter().map(Self::map_tool).collect();
            let tool_names: Vec<&str> = ox_tools.iter().map(|t| t.function.name.as_str()).collect();
            info!(?tool_names, tool_choice = "auto", "sending tools to LLM");
            request = request
                .tools(ox_tools)
                .tool_choice(ToolChoice::Mode("auto".into()));
        } else {
            info!("no tools registered, sending request without tools");
        }

        // Serialize to JSON and inject `reasoning_content: ""` on assistant messages
        // (DeepSeek thinking-mode requirement).
        let mut request_value =
            serde_json::to_value(&request).map_err(|e| ResolveError::JsonSerde {
                message: e.to_string(),
            })?;
        if let Some(messages) = request_value
            .get_mut("messages")
            .and_then(|m| m.as_array_mut())
        {
            for msg in messages {
                if msg.get("role").and_then(|r| r.as_str()) == Some("assistant")
                    && let Some(obj) = msg.as_object_mut()
                {
                    obj.entry("reasoning_content")
                        .or_insert(Value::String(String::new()));
                }
            }
        }

        debug!(
            tool_choice = %request_value.get("tool_choice").map(|v| v.to_string()).unwrap_or_default(),
            tools_count = %request_value.get("tools").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0),
            msg_count = %request_value.get("messages").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
            "sending request to LLM"
        );

        let response_value = self
            .client
            .chat()
            .completions()
            .create_raw(&request_value)
            .await
            .map_err(Self::map_err)?;

        debug!(?response_value, "raw LLM response");

        let choice = response_value["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .ok_or(ResolveError::NoChoice)?;

        debug!(?choice, "first choice from LLM");

        let action = Self::build_action(choice);
        info!(reason = ?action.reason, has_tool_calls = action.tool_calls.is_some(), "resolver produced action");
        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::resolver::IResolver;
    use crate::ai::resolver::action::Reason;
    use crate::ai::resolver_impl::openai::context::Context;

    fn user(content: &str) -> ChatCompletionMessageParam {
        ChatCompletionMessageParam::User {
            content: openai_oxide::types::chat::UserContent::Text(content.to_string()),
            name: None,
        }
    }

    fn assistant(content: &str) -> ChatCompletionMessageParam {
        ChatCompletionMessageParam::Assistant {
            content: Some(content.to_string()),
            name: None,
            tool_calls: None,
            refusal: None,
        }
    }

    fn system(content: &str) -> ChatCompletionMessageParam {
        ChatCompletionMessageParam::System {
            content: content.to_string(),
            name: None,
        }
    }

    #[tokio::test]
    async fn single_turn_conversation() {
        dotenvy::dotenv().ok();

        let mut resolver = OpenAiResolver::from_env();

        let mut cx = Context::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![user("Hello, who are you?")]);

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
    async fn three_turn_conversation_with_context() {
        dotenvy::dotenv().ok();

        let mut resolver = OpenAiResolver::from_env();

        let mut cx = Context::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![
            system("You are a helpful math assistant. Answer concisely with just the number."),
            user("What is 2 + 2?"),
            assistant("4"),
            user("Now multiply that result by 3. What do you get?"),
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

        assert!(
            action.content.is_some(),
            "expected some content in response"
        );

        let content = action.content.as_ref().unwrap();

        assert!(!content.is_empty());

        assert!(
            content.contains("12"),
            "expected '12' in three-turn response, got: {}",
            content
        );
    }
}
