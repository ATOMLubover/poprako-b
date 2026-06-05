pub mod prompt;
mod tool;
mod value_object;

use std::collections::HashMap;
use std::path::PathBuf;

use tool::build_tools;

use crate::ai::agent::compact::sliding_window_compact;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::agent_impl::openai::{OpenAiAgent, OpenAiAgentBuilder};
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::{MessageOwned, MessageRef};
use crate::ai::resolver_impl::openai::OpenAiResolver;
use crate::bot::agent::prompt::system_prompt;
use crate::bot::value_object::ChatMessage;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

const MODEL_NAME: &str = "deepseek-v4-flash";

pub struct BotAgent {
    agent: OpenAiAgent,
    /// Map from user_qid to poprako-s user_id.
    id_transform: HashMap<String, String>,
}

impl BotAgent {
    pub async fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = system_prompt()?;
        let tools = build_tools().await;
        let remote_proxy = RemoteProxy::from_local_config().await.ok();

        let context = ContextBuilder::new(MODEL_NAME)
            .messages(vec![
                MessageRef::System {
                    content: &system_prompt,
                }
                .into(),
            ])
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
            .replace_system_message(MessageOwned::System { content }.into());
    }

    pub async fn try_respond(&mut self, chat_message: ChatMessage) -> Option<String> {
        let sender_qid = chat_message.meta().sender_qid().to_string();
        let sender_prks_id = self
            .id_transform
            .get(&sender_qid)
            .map(String::as_str)
            .or_else(|| chat_message.meta().sender_prks_id());

        let chat_message = if sender_prks_id == chat_message.meta().sender_prks_id() {
            chat_message
        } else {
            let meta = chat_message.meta();
            ChatMessage::new(
                crate::bot::value_object::ChatMessageMeta::new(
                    meta.group_qid(),
                    meta.group_name(),
                    meta.sender_qid(),
                    meta.sender_nickname(),
                    meta.sender_group_nickname().map(str::to_string),
                    sender_prks_id.map(str::to_string),
                    meta.sent_at(),
                ),
                chat_message.content(),
            )
        };

        self.agent.push_message(
            MessageOwned::User {
                content: chat_message.into_prompt_text(),
            }
            .into(),
        );

        // Compact before solving to keep context within sliding window.
        // TODO: compact AFTER solving.
        self.agent.compact();

        self.agent.solve().await
    }
}
