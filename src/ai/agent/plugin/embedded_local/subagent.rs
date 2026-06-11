use std::path::PathBuf;

use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::embedded_local::subagent::RunSubagentsTool;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;

pub fn subagent_plugin(
    default_model: impl Into<String>,
    max_tasks: usize,
    tools_base_dir: PathBuf,
) -> SubagentPlugin {
    SubagentPlugin::new(default_model.into(), max_tasks, tools_base_dir)
}

pub struct SubagentPlugin {
    default_model: String,
    max_tasks: usize,
    tools_base_dir: PathBuf,
}

impl SubagentPlugin {
    pub fn new(default_model: String, max_tasks: usize, tools_base_dir: PathBuf) -> Self {
        Self {
            default_model,
            max_tasks,
            tools_base_dir,
        }
    }
}

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for SubagentPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn tools(&mut self) -> Vec<DynTool> {
        vec![Box::new(RunSubagentsTool::new(
            self.default_model.clone(),
            self.max_tasks,
            self.tools_base_dir.clone(),
        ))]
    }
}
