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
pub trait Interceptor<M>: Send
where
    M: IMessage + Send + Sync + 'static,
{
    async fn before_solve(&mut self, _cx: &mut Context<M>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_loop(&mut self, _cx: &mut Context<M>, _loop_index: usize) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_resolve(&mut self, _cx: &mut Context<M>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_resolve(
        &mut self,
        _cx: &mut Context<M>,
        _action: &mut Action<M::ToolCall>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_tool_call(
        &mut self,
        _cx: &mut Context<M>,
        _call: &M::ToolCall,
    ) -> ToolInterceptorFlow {
        ToolInterceptorFlow::Continue
    }

    async fn after_tool_call(
        &mut self,
        _cx: &mut Context<M>,
        _call: &M::ToolCall,
        _result: &mut CallResult,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn before_commit_messages(
        &mut self,
        _cx: &mut Context<M>,
        _action: &mut Action<M::ToolCall>,
        _tool_messages: &mut Vec<M>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_loop(&mut self, _cx: &mut Context<M>, _loop_index: usize) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    async fn after_solve(
        &mut self,
        _cx: &mut Context<M>,
        _output: &mut Option<String>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }
}

pub type DynInterceptor<M> = Box<dyn Interceptor<M>>;

pub struct InterceptorRegistry<M>
where
    M: IMessage + Send + Sync + 'static,
{
    interceptors: Vec<DynInterceptor<M>>,
}

impl<M> InterceptorRegistry<M>
where
    M: IMessage + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    pub fn push<I>(&mut self, interceptor: I)
    where
        I: Interceptor<M> + 'static,
    {
        self.interceptors.push(Box::new(interceptor));
    }

    pub fn set(&mut self, interceptors: Vec<DynInterceptor<M>>) {
        self.interceptors = interceptors;
    }

    pub async fn before_solve(&mut self, cx: &mut Context<M>) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_solve(cx).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_loop(&mut self, cx: &mut Context<M>, loop_index: usize) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_loop(cx, loop_index).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_resolve(&mut self, cx: &mut Context<M>) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_resolve(cx).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_resolve(
        &mut self,
        cx: &mut Context<M>,
        action: &mut Action<M::ToolCall>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_resolve(cx, action).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_tool_call(
        &mut self,
        cx: &mut Context<M>,
        call: &M::ToolCall,
    ) -> ToolInterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_tool_call(cx, call).await;
            if flow != ToolInterceptorFlow::Continue {
                return flow;
            }
        }

        ToolInterceptorFlow::Continue
    }

    pub async fn after_tool_call(
        &mut self,
        cx: &mut Context<M>,
        call: &M::ToolCall,
        result: &mut CallResult,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_tool_call(cx, call, result).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn before_commit_messages(
        &mut self,
        cx: &mut Context<M>,
        action: &mut Action<M::ToolCall>,
        tool_messages: &mut Vec<M>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor
                .before_commit_messages(cx, action, tool_messages)
                .await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_loop(&mut self, cx: &mut Context<M>, loop_index: usize) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_loop(cx, loop_index).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    pub async fn after_solve(
        &mut self,
        cx: &mut Context<M>,
        output: &mut Option<String>,
    ) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.after_solve(cx, output).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }
}
