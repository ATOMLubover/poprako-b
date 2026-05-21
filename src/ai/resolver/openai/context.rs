use openai_oxide::types::chat::ChatCompletionMessageParam;

use crate::ai::resolver;

pub type Context = resolver::context::Context<ChatCompletionMessageParam>;
