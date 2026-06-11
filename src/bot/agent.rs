mod data;
mod plugin;
mod prompt;
mod state;
mod tool;

use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::ai::agent::plugin::embedded_local::memory_shard::memory_shard_plugin;
use crate::ai::agent::plugin::embedded_local::websearch::websearch_plugin;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::agent_impl::deepseek::{DeepSeekAgent, DeepSeekAgentBuilder};
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver_impl::deepseek::DeepSeekResolver;
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::agent::plugin::review::{review_plugin, SolveKind};
use crate::bot::agent::prompt::system_prompt_from_dir;
use crate::bot::event::ReviewFollowupEvent;
use crate::bot::message::ChannelMessage;
use crate::bot::agent::plugin::inspiration::{BotCompact, inspiration_plugin};
use crate::bot::agent::plugin::prks::prks_plugin_from_env;
use crate::bot::agent::state::{BotAgentState, BotMessageAnnotation};

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
    pub async fn new(review_event_send: mpsc::Sender<ReviewFollowupEvent>) -> anyhow::Result<Self> {
        let resolver = DeepSeekResolver::from_env();

        let remote_proxy = RemoteProxy::from_local_config().await.ok();
        let memory_dir = memory_dir();
        let prompts_dir = memory_dir.join("prompts");
        let base_prompt = system_prompt_from_dir(&prompts_dir)?;
        let prompt_title = base_prompt.title().to_string();
        let prompt_sections = base_prompt.into_sections();

        let context =
            ContextBuilder::<DeepSeekMessage, BotMessageAnnotation>::new(MODEL_NAME).build();

        let prks_plugin = prks_plugin_from_env().await;

        let mut agent_state = BotAgentState::default();
        agent_state.set_review_event_send(review_event_send);

        let agent = DeepSeekAgentBuilder::new_with_state(agent_state, context, resolver)
            .base_system_sections(prompt_title, prompt_sections)
            .remote_proxy(remote_proxy)
            .compact(BotCompact::default())
            .plugin(websearch_plugin())
            .plugin(prks_plugin)
            .plugin(inspiration_plugin(memory_dir.clone())?)
            .plugin(memory_shard_plugin(memory_dir))
            .plugin(review_plugin())
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

    pub async fn respond(
        &mut self,
        message: ChannelMessage,
        content: String,
        respond_id: String,
    ) -> Option<String> {
        self.agent
            .state_mut()
            .begin_solve(SolveKind::Normal, respond_id.clone());

        let user_message = MessageOwned::User {
            // TODO: use actual sender_prks_id instead of "-"
            content: prompt_text(message, content, "-", &respond_id),
        }
        .into();

        self.agent.evaluate(user_message).await
    }

    pub async fn respond_review_feedback(&mut self, event: ReviewFollowupEvent) -> Option<String> {
        self.agent
            .state_mut()
            .begin_solve(SolveKind::ReviewFollowup, event.respond_id.clone());

        let content = format!(
            "[channel_id: {}, type: review_feedback, respond_id: {}, target_summary: {}]\n{}",
            event.channel_id, event.respond_id, event.target_summary, event.feedback
        );
        let user_message = MessageOwned::User { content }.into();

        self.agent.evaluate(user_message).await
    }
}

fn prompt_text(
    message: ChannelMessage,
    content: String,
    sender_prks_id: &str,
    respond_id: &str,
) -> String {
    format!(
        "[channel_id: {}, channel_name: {}, sender_id: {}, sender_nickname: {}, sender_channel_nickname: {}, sender_prks_id: {}, sent_at: {}, respond_id: {}]\n{}",
        message.channel_id,
        "-",
        message.actor.id,
        message.actor.nickname,
        message.actor.channel_nickname.as_deref().unwrap_or("-"),
        sender_prks_id,
        message.sent_at,
        respond_id,
        content
    )
}
