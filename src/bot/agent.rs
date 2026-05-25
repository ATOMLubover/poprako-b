pub mod prompt;

use std::path::PathBuf;

use crate::ai::agent::openai::OpenAiAgent;
use crate::ai::agent::tool::local::memory::{ListMemoryShardsTool, RecallMemoryShardTool};
use crate::ai::resolver::openai::OpenAiResolver;
use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};
use prompt::BotPrompt;

pub fn memory_dir() -> PathBuf {
    std::env::var("MEMORY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}

pub struct BotAgent {
    agent: OpenAiAgent,
    system_prompt: String,
}

impl BotAgent {
    const MODEL_NAME: &'static str = "deepseek-v4-flash[1m]";

    pub fn new() -> anyhow::Result<Self> {
        let resolver = OpenAiResolver::from_env();

        let system_prompt = BotPrompt::assemble()?;
        let mem_dir = memory_dir();

        let agent = OpenAiAgent::builder(Self::MODEL_NAME, resolver)
            .messages(vec![ChatCompletionMessageParam::System {
                content: system_prompt.clone(),
                name: None,
            }])
            .tools(vec![
                Box::new(ListMemoryShardsTool::new(mem_dir.clone())),
                Box::new(RecallMemoryShardTool::new(mem_dir)),
            ])
            .build();

        Ok(Self {
            agent,
            system_prompt,
        })
    }

    pub async fn respond(&mut self, user_text: &str) -> Option<String> {
        let mut messages = vec![ChatCompletionMessageParam::System {
            content: self.system_prompt.clone(),
            name: None,
        }];

        messages.push(ChatCompletionMessageParam::User {
            content: UserContent::Text(user_text.to_string()),
            name: None,
        });

        self.agent.set_messages(messages);
        self.agent.run_loop().await
    }
}
