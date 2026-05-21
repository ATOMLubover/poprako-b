use crate::ai::agent::tools::local::{COMMAND_LINE_TOOL, run_command_line};
use crate::ai::agent::tools::result::DispatchResult;
use crate::ai::resolver::tool::IToolCall;

pub mod local;

#[derive(Debug)]
pub struct ToolOutput {
    pub id: String,
    pub content: String,
}

pub async fn dispatch_tool_call<C>(call: &C) -> DispatchResult
where
    C: IToolCall,
{
    match call.name() {
        COMMAND_LINE_TOOL => Ok(ToolOutput {
            id: call.id().to_string(),
            content: run_command_line(call.args()).await?,
        }),
        _ => Err(result::ToolError::Fail(format!(
            "Unknown tool: {}",
            call.name()
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
