use openai_oxide::types::chat::{FunctionCall, ToolCall as OxToolCall};

use crate::ai::resolver::tool::{IToolCall, ToolCall};

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

impl From<ToolCall> for OxToolCall {
    fn from(value: ToolCall) -> Self {
        Self {
            id: value.id,
            type_: "function".to_string(),
            function: FunctionCall {
                name: value.name,
                arguments: value.arguments,
            },
        }
    }
}
