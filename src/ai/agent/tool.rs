use crate::ai::agent::tool::result::ToolResult;
use crate::ai::resolver::tool::ToolDef;

pub mod local;

/// ITool represents a tool that can be called by the agent. It should have a name,
/// a description, and a function to execute with given arguments.
#[async_trait::async_trait]
pub trait ITool {
    fn def(&self) -> ToolDef;

    async fn exec(&mut self, args: &str) -> ToolResult;
}

/// DynTool is a type alias for a boxed dynamic ITool that is Send,
/// allowing it to be used in async contexts.
/// What matters is that it can **hold state of its own** (e.g. an API client with auth
/// credentials).
pub type DynTool = Box<dyn ITool + Send>;

pub mod result {
    #[derive(Debug)]
    pub enum ToolError {
        ArgsSchema { message: String },
        ExecFail { message: String },
        UserAbort,
    }

    impl ToolError {
        pub fn args_schema(msg: String) -> Self {
            Self::ArgsSchema { message: msg }
        }

        pub fn exec_fail(msg: String) -> Self {
            Self::ExecFail { message: msg }
        }

        pub fn user_abort() -> Self {
            Self::UserAbort
        }
    }

    #[derive(Debug)]
    pub struct ToolOutput {
        pub id: String,
        pub content: String,
    }

    impl ToolOutput {
        pub fn new(id: &str, content: String) -> Self {
            Self {
                id: id.to_string(),
                content,
            }
        }
    }

    pub type ToolResult = std::result::Result<String, ToolError>;

    pub type DispatchResult = std::result::Result<ToolOutput, ToolError>;
}
