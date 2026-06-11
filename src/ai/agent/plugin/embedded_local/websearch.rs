use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::embedded_local::web::WebSearchTool;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;

pub fn websearch_plugin() -> WebSearchPlugin {
    WebSearchPlugin
}

pub struct WebSearchPlugin;

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for WebSearchPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn tools(&mut self) -> Vec<DynTool> {
        match WebSearchTool::from_env() {
            Some(tool) => vec![Box::new(tool)],
            None => {
                tracing::info!("TAVILY_API_KEY not set, skip web_search tool");
                Vec::new()
            }
        }
    }
}
