use crate::ai::resolver::tool::ToolCall;

#[derive(Debug)]
pub enum Message {
    System {
        name: Option<String>,
        content: String,
    },
    User {
        name: Option<String>,
        content: String,
    },
    Assistant {
        name: Option<String>,
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
        refusal: Option<String>,
    },
}

#[cfg(test)]
mod test {}
