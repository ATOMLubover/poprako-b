use crate::ai::resolver::{action::Action, tool::IToolCall};

#[derive(Debug)]
pub enum Message<'a, C>
where
    C: IToolCall,
{
    System {
        content: &'a str,
    },
    User {
        content: &'a str,
    },
    Assistant {
        content: Option<&'a str>,
        tool_calls: Option<&'a [C]>,
        refusal: Option<&'a str>,
    },
    Tool {
        tool_call_id: &'a str,
        content: &'a str,
    },
}

pub trait IMessage: From<Action> {
    type ToolCall: IToolCall;

    fn system(content: &str) -> Self;

    fn user(content: &str) -> Self;

    fn assistant(
        content: Option<&str>,
        tool_calls: Option<&[Self::ToolCall]>,
        refusal: Option<&str>,
    ) -> Self;

    fn tool(tool_call_id: &str, content: &str) -> Self;

    fn message(&self) -> Message<'_, Self::ToolCall>;
}
