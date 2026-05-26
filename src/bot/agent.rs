mod prompt;
mod tool;
mod value_object;

use std::path::PathBuf;

use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};
use prompt::BotPrompt;
use tool::build_tools;

use crate::ai::agent::compact::sliding_window_compact;
use crate::ai::agent::openai::{OpenAiAgent, OpenAiAgentBuilder};
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::openai::OpenAiResolver;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub struct BotAgent {
    agent: OpenAiAgent,
}

impl BotAgent {
    const MODEL_NAME: &'static str = "deepseek-v4-flash";

    pub async fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = BotPrompt::system_prompt()?;
        let tools = build_tools().await;

        let cx = ContextBuilder::new(Self::MODEL_NAME)
            .messages(vec![ChatCompletionMessageParam::System {
                content: system_prompt,
                name: None,
            }])
            .build();

        let agent = OpenAiAgentBuilder::new(cx, resolver)
            .tools(tools)
            .compact(sliding_window_compact)
            .build();

        Ok(Self { agent })
    }

    pub async fn try_respond(
        &mut self,
        user_text: &str,
        user_name: Option<String>,
    ) -> Option<String> {
        self.agent.push_message(ChatCompletionMessageParam::User {
            content: UserContent::Text(user_text.to_string()),
            name: user_name,
        });

        // Compact before solving to keep context within sliding window.
        self.agent.compact();

        self.agent.solve().await
    }
}
