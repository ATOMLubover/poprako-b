pub mod prompt;

use crate::ai::agent::openai::OpenAiAgent;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::openai::OpenAiResolver;
use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};
use prompt::BotPrompt;

pub struct BotAgent {
    agent: OpenAiAgent,
    system_prompt: String,
}

impl BotAgent {
    pub fn new() -> Self {
        let resolver = OpenAiResolver::from_env();
        let system_prompt = BotPrompt::assemble();

        let mut cx = Context::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![ChatCompletionMessageParam::System {
            content: system_prompt.clone(),
            name: None,
        }]);

        Self {
            agent: OpenAiAgent::from_context(cx, resolver),
            system_prompt,
        }
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
