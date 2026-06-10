pub mod embedded_local;

use crate::ai::{
    agent::{interceptor::DynInterceptor, tool::DynTool},
    resolver::{IResolver, message::IMessage},
};

pub trait IAgentPlugin<M, R, S, A>
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    A: Default + Send + Sync + 'static,
{
    /// Takes all tools provided.
    fn take_tools(&mut self) -> Vec<DynTool> {
        Vec::default()
    }

    /// Takes all interceptors provided.
    fn take_interceptors(&mut self) -> Vec<DynInterceptor<S, M, A>> {
        Vec::default()
    }
}
