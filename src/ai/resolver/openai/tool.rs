use openai_oxide::types::chat::ToolCall as OxToolCall;

use crate::ai::resolver::tool::IToolCall;

pub trait OpenAiToolCall: IToolCall {}

impl IToolCall for OxToolCall {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.function.name
    }

    fn args(&self) -> &str {
        &self.function.arguments
    }
}

impl OpenAiToolCall for OxToolCall {}
