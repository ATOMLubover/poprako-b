use crate::ai::resolver::tool::IToolCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Finish,
    Length,
    ToolCall,
    Unknown(String),
}

#[derive(Debug)]
pub struct Action<C>
where
    C: IToolCall + std::fmt::Debug,
{
    pub reason: Reason,
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<C>>,
}
