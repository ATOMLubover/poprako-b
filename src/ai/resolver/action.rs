use crate::ai::resolver::message::Message;
use crate::ai::resolver::tool::ToolCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Finish,
    Length,
    ToolCall,
    Unknown(String),
}

#[derive(Debug)]
pub struct Action {
    pub reason: Reason,
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[allow(clippy::from_over_into)]
impl Into<Message> for Action {
    fn into(self) -> Message {
        Message::Assistant {
            name: None,
            content: self.content,
            tool_calls: self.tool_calls,
            refusal: self.refusal,
        }
    }
}
