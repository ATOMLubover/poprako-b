pub mod prompt;
mod tool;
mod value_object;

use std::collections::HashMap;
use std::path::PathBuf;

use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};
use tool::build_tools;

use crate::ai::agent::compact::sliding_window_compact;
use crate::ai::agent::openai::{OpenAiAgent, OpenAiAgentBuilder};
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::openai::OpenAiResolver;
use crate::bot::agent::prompt::system_prompt;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub struct BotAgent {
    agent: OpenAiAgent,
    /// Map from user_qid to poprako-s user_id.
    id_transform: HashMap<String, String>,
}

impl BotAgent {
    const MODEL_NAME: &'static str = "deepseek-v4-flash";

    pub async fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = system_prompt()?;
        let tools = build_tools().await;
        let remote_proxy = RemoteProxy::from_local_config().await.ok();

        let context = ContextBuilder::new(Self::MODEL_NAME)
            .messages(vec![ChatCompletionMessageParam::System {
                content: system_prompt,
                name: None,
            }])
            .build();

        let agent = OpenAiAgentBuilder::new(context, resolver)
            .tools(tools)
            .remote_proxy(remote_proxy)
            .compact(sliding_window_compact)
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
            .replace_system_message(ChatCompletionMessageParam::System {
                content,
                name: None,
            });
    }

    pub async fn try_respond(
        &mut self,
        user_nickname: &str,
        user_qid: &str,
        user_text: &str,
    ) -> Option<String> {
        // Inline the nickname and user_id into the message text so the LLM can
        // always see it. The `name` field on ChatCompletionMessageParam is ignored
        // by DeepSeek, so embedding this info in the visible `content` is the only
        // reliable way to carry it through the resolver pipeline (including
        // compaction, which preserves `content` but drops `name`).
        let user_id_display = self
            .id_transform
            .get(user_qid)
            .map(|s| format!("(poprako-s user_id: {s})"))
            .unwrap_or_else(|| "(poprako-s user_id: -)".to_string());

        let full_text = format!("[{} {}] {}", user_nickname, user_id_display, user_text);

        self.agent.push_message(ChatCompletionMessageParam::User {
            content: UserContent::Text(full_text),
            name: None,
        });

        // Compact before solving to keep context within sliding window.
        // TODO: compact AFTER solving.
        self.agent.compact();

        self.agent.solve().await
    }
}
