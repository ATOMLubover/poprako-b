use crate::ai::agent::tool::{ITool, ToolDef, result::ToolResult};

pub enum TodoState {
    Pending,
    Processing,
    Completed,
}

pub struct Todo {
    pub id: u32,
    pub state: TodoState,
    pub desc: String,
}

pub struct TodoTool {
    todos: Vec<Todo>,
}

#[async_trait::async_trait]
impl ITool for TodoTool {
    fn def(&self) -> ToolDef {
        todo!()
    }

    async fn exec(&mut self, args: &str) -> ToolResult {
        todo!()
    }
}
