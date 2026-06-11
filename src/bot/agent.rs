mod data;
mod plugin;
mod prompt;
mod state;
mod tool;

use std::path::PathBuf;

use crate::ai::agent::plugin::embedded_local::memory_shard::memory_shard_plugin;
use crate::ai::agent::plugin::embedded_local::websearch::websearch_plugin;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::agent_impl::deepseek::DeepSeekAgent;
use crate::ai::agent_impl::deepseek::DeepSeekAgentBuilder;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver_impl::deepseek::DeepSeekResolver;
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::agent::prompt::system_prompt;
use crate::bot::message::ChannelMessage;
use plugin::inspiration::{BotCompact, inspiration_plugin};
use plugin::prks::prks_plugin_from_env;
use state::{BotAgentState, BotMessageAnnotation};

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub use prompt::watch_system_prompt;

const MODEL_NAME: &str = "deepseek-v4-flash";

pub struct BotAgent {
    agent: DeepSeekAgent<BotAgentState, BotMessageAnnotation>,
}

impl BotAgent {
    pub async fn new() -> anyhow::Result<Self> {
        let resolver = DeepSeekResolver::from_env();

        let system_prompt = system_prompt()?;

        let remote_proxy = RemoteProxy::from_local_config().await.ok();
        let memory_dir = memory_dir();

        let context = ContextBuilder::<DeepSeekMessage, BotMessageAnnotation>::new(MODEL_NAME)
            .messages(vec![
                MessageOwned::System {
                    content: system_prompt,
                }
                .into(),
            ])
            .build();

        let prks_plugin = prks_plugin_from_env().await;

        let agent =
            DeepSeekAgentBuilder::new_with_state(BotAgentState::default(), context, resolver)
                .remote_proxy(remote_proxy)
                .compact(BotCompact::default())
                .plugin(websearch_plugin())
                .plugin(prks_plugin)
                .plugin(inspiration_plugin(memory_dir.clone())?)
                .plugin(memory_shard_plugin(memory_dir))
                .build();

        Ok(Self { agent })
    }

    /// Reload the system prompt at messages[0] without affecting the
    /// conversation history (messages[1..]).
    pub fn reload_system_prompt(&mut self, content: String) {
        self.agent
            .context_mut()
            .set_system_message(MessageOwned::System { content }.into());
    }

    pub async fn respond(&mut self, message: ChannelMessage, content: String) -> Option<String> {
        let user_message = MessageOwned::User {
            // TODO: use actual sender_prks_id instead of "-"
            content: prompt_text(message, content, "-"),
        }
        .into();

        self.agent.evaluate(user_message).await
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
