mod plugin;
mod prompt;
mod tool;
mod value_object;

use std::collections::HashMap;
use std::path::PathBuf;

use openai_oxide::types::chat::ChatCompletionMessageParam;
use plugin::inspiration::InspirationAnnotation;
use plugin::inspiration::InspirationState;
use plugin::inspiration::InspiredAgent;
use plugin::inspiration::InspiredAgentBuilder;
use plugin::inspiration::modify_agent_builder;
use tool::build_tools;

use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::{MessageOwned, MessageRef};
use crate::ai::resolver_impl::openai::OpenAiResolver;
use crate::bot::agent::prompt::system_prompt;
use crate::bot::value_object::ChatMessage;
use crate::bot::value_object::ChatMessageMeta;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub use prompt::spawn_refresh_system_promt_task;

const MODEL_NAME: &str = "deepseek-v4-flash";

pub struct BotAgent {
    agent: InspiredAgent<ChatCompletionMessageParam, OpenAiResolver>,
    /// Map from user_qid to poprako-s user_id.
    id_transform: HashMap<String, String>,
}

impl BotAgent {
    pub async fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = system_prompt()?;
        let tools = build_tools().await;
        let remote_proxy = RemoteProxy::from_local_config().await.ok();

        let context = ContextBuilder::<_, InspirationAnnotation>::new(MODEL_NAME)
            .messages(vec![
                MessageRef::System {
                    content: &system_prompt,
                }
                .into(),
            ])
            .build();

        let builder =
            InspiredAgentBuilder::new_with_state(InspirationState::default(), context, resolver)
                .tools(tools)
                .remote_proxy(remote_proxy);
        let agent = modify_agent_builder(builder).build();

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
                ChatMessageMeta::new(
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

        let user_message = MessageOwned::User {
            content: chat_message.into_prompt_text(),
        }
        .into();

        self.agent.solve(user_message).await
    }
}
