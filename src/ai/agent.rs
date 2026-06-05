use std::collections::HashMap;
use std::collections::HashSet;

use crate::ai::agent::compact::Compact;
use crate::ai::agent::interceptor::DynInterceptor;
use crate::ai::agent::interceptor::Interceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::agent::interceptor::InterceptorRegistry;
use crate::ai::agent::interceptor::ToolInterceptorFlow;
use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::agent::tool::result::CallOutput;
use crate::ai::agent::tool::result::CallResult;
use crate::ai::agent::tool::result::ExecutionError;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::action::Reason;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRef};
use crate::ai::resolver::tool::IToolCall;

pub mod compact;
pub mod interceptor;
pub mod tool;

pub struct Agent<M, R>
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,

    local_tools: HashMap<String, DynTool>,
    remote_proxy: Option<RemoteProxy>,

    resolver: R,

    compact: Option<Compact<M>>,

    interceptor_registry: InterceptorRegistry<M>,
}

impl<M, R> Agent<M, R>
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn from_context(cx: Context<M>, resolver: R) -> Self {
        Self {
            context: cx,
            local_tools: HashMap::new(),
            remote_proxy: None,
            resolver,
            compact: None,
            interceptor_registry: InterceptorRegistry::new(),
        }
    }

    pub fn set_tools(&mut self, tools: Vec<DynTool>) {
        self.local_tools.clear();

        for tool in tools.into_iter() {
            let def = tool.defination();
            self.local_tools.insert(def.name.clone(), tool);
        }

        self.refresh_tools();
    }

    pub fn context(&self) -> &Context<M> {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut Context<M> {
        &mut self.context
    }

    pub fn set_compact(&mut self, compact: Compact<M>) {
        self.compact = Some(compact);
    }

    pub fn push_interceptor<I>(&mut self, interceptor: I)
    where
        I: Interceptor<M> + 'static,
    {
        self.interceptor_registry.push(interceptor);
    }

    pub fn set_interceptors(&mut self, interceptors: Vec<DynInterceptor<M>>) {
        self.interceptor_registry.set(interceptors);
    }

    /// Replace all registered tools, returning the old ones.
    pub fn swap_tools(&mut self, tools: Vec<DynTool>) -> Vec<DynTool> {
        let old: Vec<DynTool> = self.local_tools.drain().map(|(_, v)| v).collect();

        for tool in tools {
            let def = tool.defination();
            self.local_tools.insert(def.name.clone(), tool);
        }

        self.refresh_tools();

        old
    }

    /// Run the agent loop. Returns the final assistant text response, or `None`
    /// if the resolver failed before producing a final answer.
    pub async fn solve(&mut self) -> Option<String> {
        if let InterceptorFlow::Stop { output } = self
            .interceptor_registry
            .before_solve(&mut self.context)
            .await
        {
            return self.finish_solve(output).await;
        }

        let mut loop_index = 0;

        loop {
            if let InterceptorFlow::Stop { output } = self
                .interceptor_registry
                .before_loop(&mut self.context, loop_index)
                .await
            {
                return self.finish_solve(output).await;
            }

            if let InterceptorFlow::Stop { output } = self
                .interceptor_registry
                .before_resolve(&mut self.context)
                .await
            {
                return self.finish_solve(output).await;
            }

            let action = match self.resolver.resolve(&self.context).await {
                Ok(action) => {
                    tracing::info!("resolver produced action: {:?}", action);
                    action
                }
                Err(e) => {
                    tracing::error!("resolve failed: {:?}", e);
                    return self.finish_solve(None).await;
                }
            };

            let mut action = action;
            if let InterceptorFlow::Stop { output } = self
                .interceptor_registry
                .after_resolve(&mut self.context, &mut action)
                .await
            {
                return self.finish_solve(output).await;
            }

            // Process tool calls from the local action — no borrow on self.context.
            let tool_messages: Vec<M> = match &action.tool_calls {
                None => Vec::new(),
                Some(calls) => {
                    let mut messages = Vec::with_capacity(calls.len());
                    for call in calls {
                        let mut result = match self
                            .interceptor_registry
                            .before_tool_call(&mut self.context, call)
                            .await
                        {
                            ToolInterceptorFlow::Continue => self.handle_call(call).await,
                            ToolInterceptorFlow::Skip { content } => {
                                Ok(CallOutput::new(call.id().to_string(), content))
                            }
                            ToolInterceptorFlow::Stop { output } => {
                                return self.finish_solve(output).await;
                            }
                        };

                        if let InterceptorFlow::Stop { output } = self
                            .interceptor_registry
                            .after_tool_call(&mut self.context, call, &mut result)
                            .await
                        {
                            return self.finish_solve(output).await;
                        }

                        let message = match &result {
                            Ok(output) => M::from(MessageRef::Tool {
                                tool_call_id: &output.call_id,
                                content: &output.content,
                            }),
                            Err(e) => {
                                let err_msg = format!("{:?}", e);
                                M::from(MessageRef::Tool {
                                    tool_call_id: call.id(),
                                    content: &err_msg,
                                })
                            }
                        };

                        messages.push(message);
                    }

                    messages
                }
            };

            let mut tool_messages = tool_messages;
            if let InterceptorFlow::Stop { output } = self
                .interceptor_registry
                .before_commit_messages(&mut self.context, &mut action, &mut tool_messages)
                .await
            {
                return self.finish_solve(output).await;
            }

            let reason = action.reason.clone();

            // Extract finish content before action is consumed.
            let finish_content = if matches!(reason, Reason::Finish) {
                action.content.clone()
            } else {
                None
            };

            // Push assistant message and tool results to context together.
            self.context.push_message(action.into());
            for msg in tool_messages {
                self.context.push_message(msg);
            }

            if let InterceptorFlow::Stop { output } = self
                .interceptor_registry
                .after_loop(&mut self.context, loop_index)
                .await
            {
                return self.finish_solve(output).await;
            }

            match reason {
                Reason::Finish => return self.finish_solve(finish_content).await,
                // FIXME: ToolCall, Length, Unknown → continue the loop.
                _ => {
                    loop_index += 1;
                    continue;
                }
            }
        }
    }

    pub fn compact(&mut self) {
        if let Some(compact) = self.compact {
            compact(&mut self.context);
        }
    }

    async fn handle_call(&mut self, call: &M::ToolCall) -> CallResult {
        if let Some(tool) = self.local_tools.get_mut(call.name()) {
            let content = tool.execute(call.args()).await?;
            return Ok(CallOutput::new(call.id().to_string(), content));
        }

        if let Some(remote_proxy) = &self.remote_proxy
            && remote_proxy.has_tool(call.name())
        {
            return remote_proxy.handle_call(call).await;
        }

        Err(ExecutionError::exec_fail(format!(
            "tool not found: {}",
            call.name()
        )))
    }

    async fn finish_solve(&mut self, mut output: Option<String>) -> Option<String> {
        match self
            .interceptor_registry
            .after_solve(&mut self.context, &mut output)
            .await
        {
            InterceptorFlow::Continue => output,
            InterceptorFlow::Stop { output } => output,
        }
    }

    fn refresh_tools(&mut self) {
        let mut defs = Vec::with_capacity(self.local_tools.len());
        let mut local_names = HashSet::with_capacity(self.local_tools.len());

        for tool in self.local_tools.values() {
            let def = tool.defination();
            local_names.insert(def.name.clone());
            defs.push(def);
        }

        if let Some(remote_proxy) = &self.remote_proxy {
            for def in remote_proxy.tool_definations() {
                if local_names.contains(&def.name) {
                    tracing::warn!(tool = %def.name, "remote tool conflicts with local tool, skip");
                    continue;
                }

                defs.push(def);
            }
        }

        self.context.set_tool_defs(defs);
    }
}

pub struct AgentBuilder<M, R>
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    resolver: R,
    tools: Vec<DynTool>,
    remote_proxy: Option<RemoteProxy>,
    compact: Option<Compact<M>>,
    interceptors: Vec<DynInterceptor<M>>,
}

impl<M, R> AgentBuilder<M, R>
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn new(context: Context<M>, resolver: R) -> Self {
        Self {
            context,
            resolver,
            tools: Vec::new(),
            remote_proxy: None,
            compact: None,
            interceptors: Vec::new(),
        }
    }

    pub fn tools(mut self, tools: Vec<DynTool>) -> Self {
        self.tools = tools;
        self
    }

    pub fn compact(mut self, compact: Compact<M>) -> Self {
        self.compact = Some(compact);
        self
    }

    pub fn remote_proxy(mut self, remote_proxy: Option<RemoteProxy>) -> Self {
        self.remote_proxy = remote_proxy;
        self
    }

    pub fn interceptor<I>(mut self, interceptor: I) -> Self
    where
        I: Interceptor<M> + 'static,
    {
        self.interceptors.push(Box::new(interceptor));
        self
    }

    pub fn interceptors(mut self, interceptors: Vec<DynInterceptor<M>>) -> Self {
        self.interceptors = interceptors;
        self
    }

    pub fn build(self) -> Agent<M, R> {
        let mut agent = Agent::from_context(self.context, self.resolver);

        agent.compact = self.compact;
        agent.remote_proxy = self.remote_proxy;
        agent.set_interceptors(self.interceptors);
        agent.set_tools(self.tools);

        agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use crate::ai::agent::interceptor::InterceptorFlow;
    use crate::ai::agent::tool::local::fs::{CreateFileTool, ReadFileTool};
    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::result::ResolveResult;
    use crate::ai::resolver_impl::openai::OpenAiResolver;

    use openai_oxide::types::chat::ChatCompletionMessageParam;
    use openai_oxide::types::chat::ToolCall as OxToolCall;
    use openai_oxide::types::chat::UserContent;

    struct CountingResolver {
        calls: Arc<AtomicUsize>,
        content: String,
    }

    #[async_trait::async_trait]
    impl IResolver for CountingResolver {
        type Message = ChatCompletionMessageParam;

        async fn resolve(
            &mut self,
            _cx: &Context<Self::Message>,
        ) -> ResolveResult<crate::ai::resolver::action::Action<OxToolCall>> {
            self.calls.fetch_add(1, Ordering::SeqCst);

            Ok(crate::ai::resolver::action::Action {
                reason: Reason::Finish,
                content: Some(self.content.clone()),
                refusal: None,
                tool_calls: None,
            })
        }
    }

    struct StopBeforeSolve;

    #[async_trait::async_trait]
    impl crate::ai::agent::interceptor::Interceptor<ChatCompletionMessageParam> for StopBeforeSolve {
        async fn before_solve(
            &mut self,
            _cx: &mut Context<ChatCompletionMessageParam>,
        ) -> InterceptorFlow {
            InterceptorFlow::Stop {
                output: Some("stopped".to_string()),
            }
        }
    }

    struct RewriteOutput;

    #[async_trait::async_trait]
    impl crate::ai::agent::interceptor::Interceptor<ChatCompletionMessageParam> for RewriteOutput {
        async fn after_resolve(
            &mut self,
            _cx: &mut Context<ChatCompletionMessageParam>,
            action: &mut crate::ai::resolver::action::Action<OxToolCall>,
        ) -> InterceptorFlow {
            action.content = Some("rewritten".to_string());
            InterceptorFlow::Continue
        }

        async fn after_solve(
            &mut self,
            _cx: &mut Context<ChatCompletionMessageParam>,
            output: &mut Option<String>,
        ) -> InterceptorFlow {
            *output = output.take().map(|text| format!("{}!", text));
            InterceptorFlow::Continue
        }
    }

    #[tokio::test]
    async fn before_solve_interceptor_can_stop_without_resolving() {
        let calls = Arc::new(AtomicUsize::new(0));
        let resolver = CountingResolver {
            calls: Arc::clone(&calls),
            content: "resolver output".to_string(),
        };
        let cx = ContextBuilder::new("test-model").build();

        let mut agent = AgentBuilder::new(cx, resolver)
            .interceptor(StopBeforeSolve)
            .build();

        let result = agent.solve().await;

        assert_eq!(result.as_deref(), Some("stopped"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn interceptors_can_rewrite_resolved_and_final_output() {
        let calls = Arc::new(AtomicUsize::new(0));
        let resolver = CountingResolver {
            calls: Arc::clone(&calls),
            content: "original".to_string(),
        };
        let cx = ContextBuilder::new("test-model").build();

        let mut agent = AgentBuilder::new(cx, resolver)
            .interceptor(RewriteOutput)
            .build();

        let result = agent.solve().await;

        assert_eq!(result.as_deref(), Some("rewritten!"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn create_file_and_read() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let output_dir = PathBuf::from("tests/output");
        let target_file = output_dir.join("hello.txt");

        // Create the output directory; leftover files from previous runs are
        // cleaned up by CreateFileTool's Drop when the agent goes out of scope.
        std::fs::create_dir_all(&output_dir).expect("should create tests/output");

        let resolver = OpenAiResolver::from_env();

        let cx = ContextBuilder::new("deepseek-v4-flash")
            .messages(vec![
                ChatCompletionMessageParam::System {
                    content:
                        "You are a helpful assistant. Use the create_file tool to create files and \
                              the read_file tool to read them. The path parameter is relative to the \
                              base directory. When asked to create a file, use the tool directly - \
                              do not ask for confirmation."
                            .to_string(),
                    name: None,
                },
                ChatCompletionMessageParam::User {
                    content: UserContent::Text(
                        "Create a file at 'hello.txt' with the content 'hello from agent', \
                         then read it back to confirm the content."
                            .to_string(),
                    ),
                    name: None,
                },
            ])
            .build();

        let mut agent = AgentBuilder::new(cx, resolver)
            .tools(vec![
                Box::new(CreateFileTool::new(output_dir.clone())),
                Box::new(ReadFileTool::new(output_dir)),
            ])
            .build();

        let result = agent.solve().await;
        assert!(result.is_some(), "agent should return a final response");
        assert!(
            target_file.exists(),
            "expected file at {}",
            target_file.display()
        );

        let contents = std::fs::read_to_string(&target_file).expect("should read created file");
        assert!(
            contents.contains("hello from agent"),
            "file should contain 'hello from agent', got: {contents}"
        );

        // Pop CreateFileTool out before agent drops, then clean up the file directly.
        let _old_tools = agent.swap_tools(vec![]);
        let _ = std::fs::remove_file(&target_file);
    }
}
