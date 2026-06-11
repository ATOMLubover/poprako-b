use crate::ai::resolver::tool::IToolCall;
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekToolCall;

impl IToolCall for DeepSeekToolCall {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::resolver_impl::deepseek::data_object::ToolCallFunction;

    #[test]
    fn tool_call_trait_returns_correct_fields() {
        let tc = DeepSeekToolCall {
            id: "call_1".into(),
            type_: "function".into(),
            function: ToolCallFunction {
                name: "my_tool".into(),
                arguments: "{\"key\": \"value\"}".into(),
            },
        };

        assert_eq!(tc.id(), "call_1");
        assert_eq!(tc.name(), "my_tool");
        assert_eq!(tc.args(), "{\"key\": \"value\"}");
    }
}
