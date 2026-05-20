use crate::ai::agent::tools::local::{COMMAND_LINE_TOOL, run_command_line};
use crate::ai::agent::tools::result::DispatchResult;
use crate::ai::resolver::tool::ToolCall;

pub mod local;

#[derive(Debug)]
pub struct ToolOutput {
    pub id: String,
    pub content: String,
}

pub async fn dispatch_tool_call(call: &ToolCall) -> DispatchResult {
    match call.name.as_str() {
        COMMAND_LINE_TOOL => Ok(ToolOutput {
            id: call.id.clone(),
            content: run_command_line(&call.arguments).await?,
        }),
        _ => Err(result::ToolError::Fail(format!(
            "Unknown tool: {}",
            call.name
        ))),
    }
}

pub mod result {
    use crate::ai::agent::tools::ToolOutput;

    #[derive(Debug)]
    pub enum ToolError {
        Fail(String),
        UserAbort,
    }

    pub type ToolResult = std::result::Result<String, ToolError>;
    pub type DispatchResult = std::result::Result<ToolOutput, ToolError>;
}
