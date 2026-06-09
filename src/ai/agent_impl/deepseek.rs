use crate::ai::agent::{Agent, AgentBuilder};
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::ai::resolver_impl::deepseek::DeepSeekResolver;

pub type DeepSeekAgent<S, A> = Agent<DeepSeekMessage, DeepSeekResolver, S, A>;
pub type DeepSeekAgentBuilder<S, A> = AgentBuilder<DeepSeekMessage, DeepSeekResolver, S, A>;
