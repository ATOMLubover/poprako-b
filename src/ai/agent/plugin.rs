pub mod embedded_local;

use crate::ai::agent::prompt::SystemPromptSubSection;
use crate::ai::agent::{interceptor::DynInterceptor, tool::DynTool};
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;

pub trait IAgentPlugin<M, R, S, A>
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    A: Default + Send + Sync + 'static,
{
    /// Generates a system prompt for the agent about this plugin.
    fn system_prompt(&self) -> Option<SystemPromptSubSection> {
        None
    }

    /// Takes all tools provided.
    fn tools(&mut self) -> Vec<DynTool> {
        Vec::default()
    }

    /// Takes all interceptors provided.
    fn interceptors(&mut self) -> Vec<DynInterceptor<S, M, A>> {
        Vec::default()
    }
}
