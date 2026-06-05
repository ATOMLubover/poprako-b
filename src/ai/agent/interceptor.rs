use crate::ai::agent::tool::result::CallResult;
use crate::ai::resolver::action::Action;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;

#[derive(Debug, PartialEq, Eq)]
pub enum InterceptorFlow {
    Continue,
    Stop { output: Option<String> },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ToolInterceptorFlow {
    Continue,
    Skip { content: String },
    Stop { output: Option<String> },
}

#[async_trait::async_trait]
pub trait Interceptor<S, M, A>: Send
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    async fn before_solve(&mut self, _state: &mut S, _cx: &mut Context<M, A>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_loop(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _loop_index: usize,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_resolve(&mut self, _state: &mut S, _cx: &mut Context<M, A>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_resolve(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _action: &mut Action<M::ToolCall>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_tool_call(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _call: &M::ToolCall,
    ) -> ToolInterceptorFlow {
        ToolInterceptorFlow::Continue
    }

    async fn after_tool_call(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _call: &M::ToolCall,
        _result: &mut CallResult,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_commit_messages(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _action: &mut Action<M::ToolCall>,
        _tool_messages: &mut Vec<M>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_loop(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _loop_index: usize,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_solve(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _output: &mut Option<String>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }
}

pub type DynInterceptor<S, M, A> = Box<dyn Interceptor<S, M, A>>;

pub struct InterceptorRegistry<S, M, A>
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    interceptors: Vec<DynInterceptor<S, M, A>>,
}

impl<S, M, A> InterceptorRegistry<S, M, A>
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    pub fn push<I>(&mut self, interceptor: I)
    where
        I: Interceptor<S, M, A> + 'static,
    {
        self.interceptors.push(Box::new(interceptor));
    }

    pub fn set(&mut self, interceptors: Vec<DynInterceptor<S, M, A>>) {
        self.interceptors = interceptors;
    }

    pub async fn before_solve(&mut self, state: &mut S, cx: &mut Context<M, A>) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_solve(state, cx).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_loop(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        loop_index: usize,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_loop(state, cx, loop_index).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_resolve(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_resolve(state, cx).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_resolve(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        action: &mut Action<M::ToolCall>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_resolve(state, cx, action).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_tool_call(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        call: &M::ToolCall,
    ) -> ToolInterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_tool_call(state, cx, call).await;
            if flow != ToolInterceptorFlow::Continue {
                return flow;
            }
        }

        ToolInterceptorFlow::Continue
    }

    pub async fn after_tool_call(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        call: &M::ToolCall,
        result: &mut CallResult,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_tool_call(state, cx, call, result).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_commit_messages(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        action: &mut Action<M::ToolCall>,
        tool_messages: &mut Vec<M>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor
                .before_commit_messages(state, cx, action, tool_messages)
                .await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_loop(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        loop_index: usize,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_loop(state, cx, loop_index).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_solve(
        &mut self,
        state: &mut S,
        cx: &mut Context<M, A>,
        output: &mut Option<String>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_solve(state, cx, output).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }
}

impl<S, M, A> Default for InterceptorRegistry<S, M, A>
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
