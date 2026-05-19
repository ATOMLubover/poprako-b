// OpenAI Resolver implementation based solely on HTTP.

use crate::ai::message::Message;
use crate::ai::resolver::action::{Action, Reason};
use crate::ai::resolver::result::{ResolveError, ResolveResult};
use crate::ai::resolver::tool::ToolCall;
use crate::ai::resolver::{Context, Resolver};
use openai_oxide::types::chat::{
    ChatCompletionChoice, ChatCompletionMessageParam, ChatCompletionRequest, FinishReason,
    FunctionCall, ToolCall as OxToolCall, UserContent,
};
use openai_oxide::{ClientConfig, OpenAI, OpenAIError};

pub struct OpenAiResolver {
    client: OpenAI,
}

// Transform direction:
// build: OpenAI -> crate
// map: crate -> OpenAI

impl OpenAiResolver {
    pub fn from_env() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("[OpenAiResolver::with_env] OPENAI_API_KEY not set in environment");
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        // Ensure trailing slash so Url::parse treats it as a directory base.
        let base_url = if base_url.ends_with('/') {
            base_url
        } else {
            format!("{base_url}/")
        };

        Self {
            client: OpenAI::with_config(ClientConfig::new(api_key).base_url(base_url)),
        }
    }

    fn map_message(msg: &Message) -> ChatCompletionMessageParam {
        match msg {
            Message::System { name, content } => ChatCompletionMessageParam::System {
                content: content.clone(),
                name: name.clone(),
            },
            Message::User { name, content } => ChatCompletionMessageParam::User {
                content: UserContent::Text(content.clone()),
                name: name.clone(),
            },
            Message::Assistant {
                name,
                content,
                tool_calls,
                refusal,
            } => ChatCompletionMessageParam::Assistant {
                content: content.clone(),
                name: name.clone(),
                tool_calls: tool_calls
                    .as_ref()
                    .map(|tc| tc.iter().map(Self::map_tool_call).collect()),
                refusal: refusal.clone(),
            },
        }
    }

    fn map_tool_call(call: &ToolCall) -> OxToolCall {
        OxToolCall {
            id: call.id.clone(),
            type_: "function".to_string(),
            function: FunctionCall {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        }
    }

    fn build_history(cx: &Context) -> Vec<ChatCompletionMessageParam> {
        cx.messages.iter().map(Self::map_message).collect()
    }

    fn build_tool_call(call: OxToolCall) -> ToolCall {
        ToolCall {
            id: call.id,
            name: call.function.name,
            arguments: call.function.arguments,
        }
    }

    fn map_err(err: OpenAIError) -> ResolveError {
        match err {
            OpenAIError::ApiError {
                status,
                message,
                type_: _,
                code: _,
                request_id: _,
            } => ResolveError::ApiError { status, message },
            OpenAIError::RequestError(e) => ResolveError::RequestError(e.to_string()),
            OpenAIError::JsonError(e) => ResolveError::JsonError(e.to_string()),
            OpenAIError::StreamError(msg) | OpenAIError::InvalidArgument(msg) => {
                ResolveError::Other(msg)
            }
        }
    }

    fn build_action(choice: ChatCompletionChoice) -> Action {
        let reason = match choice.finish_reason {
            FinishReason::Stop => Reason::Finish,
            FinishReason::Length => Reason::Length,
            FinishReason::ToolCalls | FinishReason::FunctionCall => Reason::ToolCall,
            other => Reason::Unknown(other.to_string()),
        };

        let msg = choice.message;

        let tool_calls = msg
            .tool_calls
            .map(|tc| tc.into_iter().map(Self::build_tool_call).collect());

        Action {
            reason,
            content: msg.content,
            refusal: msg.refusal,
            tool_calls,
        }
    }
}

#[async_trait::async_trait]
impl Resolver for OpenAiResolver {
    async fn resolve(&mut self, cx: &Context) -> ResolveResult<Action> {
        let request = ChatCompletionRequest::new(cx.model.clone(), Self::build_history(cx));

        let response = self
            .client
            .chat()
            .completions()
            .create(request)
            .await
            .map_err(Self::map_err)?;

        // Take only the first choice.
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(ResolveError::NoResponse)?;

        Ok(Self::build_action(choice))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::message::Message;
    use crate::ai::resolver::action::Reason;
    use crate::ai::resolver::{Context, Resolver};

    fn user(content: &str) -> Message {
        Message::User {
            name: None,
            content: content.into(),
        }
    }

    fn assistant(content: &str) -> Message {
        Message::Assistant {
            name: None,
            content: Some(content.into()),
            tool_calls: None,
            refusal: None,
        }
    }

    fn system(content: &str) -> Message {
        Message::System {
            name: None,
            content: content.into(),
        }
    }

    #[tokio::test]
    async fn single_turn_conversation() {
        dotenvy::dotenv().ok();

        let mut resolver = OpenAiResolver::from_env();

        let mut cx = Context::new("deepseek-v4-flash", vec![user("Hello, who are you?")]);

        let action = resolver
            .resolve(&mut cx)
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

        let mut cx = Context::new(
            "deepseek-v4-flash",
            vec![
                system("You are a helpful math assistant. Answer concisely with just the number."),
                user("What is 2 + 2?"),
                assistant("4"),
                user("Now multiply that result by 3. What do you get?"),
            ],
        );

        let action = resolver
            .resolve(&mut cx)
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
            "expected '12' in three-turn response, got: {content}"
        );
    }
}
