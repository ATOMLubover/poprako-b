mod plugin;
mod prompt;
mod state;
mod tool;
mod value_object;

use std::collections::HashMap;
use std::path::PathBuf;

use openai_oxide::types::chat::ChatCompletionMessageParam;
use plugin::inspiration::plugin_inspiration;
use state::BotAgentState;
use state::BotMessageAnnotation;
use tool::build_tools;

use crate::ai::agent::Agent;
use crate::ai::agent::AgentBuilder;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver_impl::openai::OpenAiResolver;
use crate::bot::agent::prompt::system_prompt;
use crate::bot::message::ChannelMessage;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub use prompt::watch_system_prompt;

const MODEL_NAME: &str = "deepseek-v4-flash";

pub struct BotAgent {
    agent: Agent<ChatCompletionMessageParam, OpenAiResolver, BotAgentState, BotMessageAnnotation>,
    /// Map from channel actor id to poprako-s user_id.
    id_transform: HashMap<String, String>,
}

impl BotAgent {
    pub async fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = system_prompt()?;

        let tools = build_tools().await;
        let remote_proxy = RemoteProxy::from_local_config().await.ok();

        let context = ContextBuilder::<_, BotMessageAnnotation>::new(MODEL_NAME)
            .messages(vec![
                MessageOwned::System {
                    content: system_prompt,
                }
                .into(),
            ])
            .build();

        let agent = AgentBuilder::new_with_state(BotAgentState::default(), context, resolver)
            .tools(tools)
            .remote_proxy(remote_proxy)
            .plugin(plugin_inspiration())
            .build();

        Ok(Self {
            agent,
            id_transform: HashMap::new(),
        })
    }

    /// Reload the system prompt at messages[0] without affecting the
    /// conversation history (messages[1..]).
    pub fn reload_system_prompt(&mut self, content: String) {
        self.agent
            .context_mut()
            .set_system_message(MessageOwned::System { content }.into());
    }

    pub async fn try_answer(&mut self, message: ChannelMessage, content: String) -> Option<String> {
        let sender_id = message.actor.id.as_str();
        let sender_prks_id = self
            .id_transform
            .get(sender_id)
            .map(String::as_str)
            .unwrap_or("-");

        let user_message = MessageOwned::User {
            content: prompt_text(message, content, sender_prks_id),
        }
        .into();

        self.agent.solve(user_message).await
    }
}

#[cfg(test)]
impl BotAgent {
    pub(crate) fn new_for_test() -> Self {
        let resolver = OpenAiResolver::from_env();
        let context = ContextBuilder::<_, BotMessageAnnotation>::new("test-model").build();
        let agent =
            AgentBuilder::new_with_state(BotAgentState::default(), context, resolver).build();

        Self {
            agent,
            id_transform: HashMap::new(),
        }
    }
}

fn prompt_text(message: ChannelMessage, content: String, sender_prks_id: &str) -> String {
    format!(
        "[channel_id: {}, channel_name: {}, sender_id: {}, sender_nickname: {}, sender_channel_nickname: {}, sender_prks_id: {}, sent_at: {}]\n{}",
        message.channel_id,
        "-",
        message.actor.id,
        message.actor.nickname,
        message.actor.channel_nickname.as_deref().unwrap_or("-"),
        sender_prks_id,
        message.sent_at,
        content
    )
}
