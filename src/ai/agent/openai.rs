use openai_oxide::types::chat::ChatCompletionMessageParam;

use crate::ai::agent::{Agent, AgentBuilder};
use crate::ai::resolver::openai::OpenAiResolver;

pub type OpenAiAgent = Agent<ChatCompletionMessageParam, OpenAiResolver>;
pub type OpenAiAgentBuilder = AgentBuilder<ChatCompletionMessageParam, OpenAiResolver>;
