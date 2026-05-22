use openai_oxide::types::chat::ChatCompletionMessageParam;

use crate::ai::agent::Agent;
use crate::ai::resolver::openai::OpenAiResolver;

pub type OpenAiAgent = Agent<ChatCompletionMessageParam, OpenAiResolver>;
