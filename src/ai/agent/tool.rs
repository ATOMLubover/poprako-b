use crate::ai::agent::tool::result::ExecutionResult;
use crate::ai::resolver::tool::ToolDefination;

pub mod local;
pub mod remote;

/// ITool represents a tool that can be called by the agent. It should have a name,
/// a description, and a function to execute with given arguments.
#[async_trait::async_trait]
pub trait ITool {
    /// Returns the definition of the tool, including its name, description, and parameters.
    fn defination(&self) -> ToolDefination;

    /// Executes the tool with the given arguments, and returns the result as a string.
    async fn execute(&mut self, args: &str) -> ExecutionResult;
}

/// DynTool is a type alias for a boxed dynamic ITool that is Send,
/// allowing it to be used in async contexts.
/// What matters is that it can **hold state of its own** (e.g. an API client with auth
/// credentials).
pub type DynTool = Box<dyn ITool + Send>;

pub mod result {
    #[derive(Debug)]
    pub enum ExecutionError {
        ArgsSchema { message: String },
        ExecFail { message: String },
        UserAbort,
    }

    impl ExecutionError {
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

    pub type ExecutionResult = std::result::Result<String, ExecutionError>;

    #[derive(Debug)]
    pub struct CallOutput {
        pub call_id: String,
        pub content: String,
    }

    impl CallOutput {
        pub fn new(call_id: String, content: String) -> Self {
            Self { call_id, content }
        }
    }

    pub type CallResult = std::result::Result<CallOutput, ExecutionError>;
}
