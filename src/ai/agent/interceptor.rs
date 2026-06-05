use crate::ai::agent::tool::result::CallResult;
use crate::ai::resolver::action::Action;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;

/// Flow control signal returned by non-tool interceptor hooks.
///
/// Each hook called during the agent solve lifecycle returns this to tell the
/// agent what to do next:
///
/// | Variant | Effect |
/// |---------|--------|
/// | `Continue` | Proceed to the next hook or lifecycle phase normally. |
/// | `Stop { output }` | Immediately abort the current phase and finish the
///   entire solve with the given `output`. The returned string becomes the
///   final answer of `Agent::solve()`. If `None`, the solve returns `None`. |
#[derive(Debug, PartialEq, Eq)]
pub enum InterceptorFlow {
    Continue,
    Stop { output: Option<String> },
}

/// Flow control signal returned by the tool pre-call hook
/// ([`Interceptor::before_tool_call`]).
///
/// | Variant | Effect |
/// |---------|--------|
/// | `Continue` | Execute the tool normally via the local / remote handler. |
/// | `Skip { content }` | Short-circuit tool execution — pretend the tool
///   returned `content` as its result, without actually running it. |
/// | `Stop { output }` | Immediately abort the current phase and finish the
///   entire solve with the given `output` (same semantics as
///   [`InterceptorFlow::Stop`]). |
#[derive(Debug, PartialEq, Eq)]
pub enum ToolInterceptorFlow {
    Continue,
    Skip { content: String },
    Stop { output: Option<String> },
}

/// Interceptor trait — hook into the [`Agent`](crate::ai::agent::Agent) solve lifecycle.
///
/// Each method is called at a specific node in the solve loop (see the
/// individual doc comments). All methods have default no-op implementations
/// (`InterceptorFlow::Continue`), so you only need to override the ones you
/// care about.
///
/// Hooks are run in insertion order through an
/// [`InterceptorRegistry`]. The first hook that returns a non-`Continue` value
/// short-circuits the remaining hooks at that node.
///
/// # Lifecycle overview
///
/// ```text
/// Agent::solve()
/// ├── before_solve          ← you are here: once before the loop
/// │
/// └── loop (repeat until Finish):
///     ├── before_loop
///     ├── before_resolve
///     ├── resolve            ← resolver produces an Action
///     ├── after_resolve      ← inspect / mutate the Action
///     │
///     ├── [for each tool call]:
///     │   ├── before_tool_call   ← skip / stop / let through
///     │   ├── execute tool       ← local handler or remote proxy
///     │   └── after_tool_call    ← inspect / mutate the result
///     │
///     ├── before_commit_messages  ← last chance before context changes
///     ├── commit messages         ← push action + tool results into Context
///     └── after_loop              ← decide next iteration or finish
///
/// after_solve              ← final output, after the loop
/// ```
#[async_trait::async_trait]
pub trait Interceptor<S, M, A>: Send
where
    S: Send + Sync + 'static,
    M: IMessage + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    /// Called **once** at the very start of [`Agent::solve`], before the solve
    /// loop begins.
    ///
    /// Use this to:
    /// - Validate or pre-process the initial `Context` (e.g. inject system
    ///   messages, enforce a minimum message count).
    /// - Perform auth / guard checks and abort early via `Stop`.
    /// - Initialize shared state in `S`.
    ///
    /// Returning `Stop { output }` **skips the entire solve loop** and jumps
    /// directly to `after_solve` with the given output.
    async fn before_solve(&mut self, _state: &mut S, _cx: &mut Context<M, A>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called at the **start of every loop iteration**, before the resolver.
    ///
    /// `loop_index` starts at `0` and increments each iteration.
    ///
    /// Use this to:
    /// - Enforce iteration limits (e.g. return `Stop` after N loops).
    /// - Run compaction / summarization before the next resolver call.
    /// - Inspect or modify `Context` between iterations.
    async fn before_loop(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _loop_index: usize,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **immediately before** the resolver is invoked.
    ///
    /// This is the last opportunity to modify the context before it is sent
    /// to the LLM. Use this to:
    /// - Pin or inject additional messages (e.g. retrieved memories, recent
    ///   observations).
    /// - Adjust available tool definitions dynamically.
    /// - Attach per-message annotations for downstream consumers.
    async fn before_resolve(&mut self, _state: &mut S, _cx: &mut Context<M, A>) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **immediately after** the resolver returns an [`Action`].
    ///
    /// The action is mutable, so you can:
    /// - Rewrite, append, or remove the assistant content.
    /// - Modify or clear `tool_calls` (e.g. reject undesired tool calls).
    /// - Change the `reason` (e.g. force `Finish` to short-circuit).
    async fn after_resolve(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _action: &mut Action<M::ToolCall>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **before each individual tool execution**.
    ///
    /// Unlike the other hooks, this returns a [`ToolInterceptorFlow`] which
    /// offers three options:
    ///
    /// - `Continue` — let the tool execute normally.
    /// - `Skip { content }` — pretend the tool returned `content` without
    ///   actually running it (useful for caching, mocking, or rate-limiting).
    /// - `Stop { output }` — abort the entire solve immediately.
    async fn before_tool_call(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _call: &M::ToolCall,
    ) -> ToolInterceptorFlow {
        ToolInterceptorFlow::Continue
    }

    /// Called **after each individual tool execution**, with the result.
    ///
    /// The `result` is mutable, so you can:
    /// - Inspect, redact, or rewrite the tool output before it is formatted
    ///   into a tool-response message.
    /// - Log or persist tool calls.
    /// - Convert an error into a friendly message (or vice versa).
    async fn after_tool_call(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _call: &M::ToolCall,
        _result: &mut CallResult,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **before** the action and tool-result messages are committed
    /// (pushed) into the [`Context`].
    ///
    /// Both `action` and `tool_messages` are mutable, giving you a final
    /// opportunity to:
    /// - Drop, reorder, or rewrite any message before it enters the permanent
    ///   conversation history.
    /// - Attach custom annotations to individual messages (via
    ///   [`Context::push_annotated_message`]).
    /// - Run side-effects like persistence or logging.
    ///
    /// After this hook, `commit_messages` calls
    /// [`Context::push_message`](crate::ai::resolver::context::Context::push_message)
    /// for each item — there is no further per-message interceptor hook.
    async fn before_commit_messages(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _action: &mut Action<M::ToolCall>,
        _tool_messages: &mut Vec<M>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **after** messages are committed, at the end of the current
    /// loop iteration, right before the agent decides whether to continue or
    /// finish.
    ///
    /// If you return `Stop` here, the loop exits and `after_solve` is called
    /// next.
    async fn after_loop(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _loop_index: usize,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }

    /// Called **once after the solve loop finishes** (either naturally with
    /// `Reason::Finish`, or via a `Stop` from any earlier hook).
    ///
    /// This is the last hook before the agent returns to its caller. Use it
    /// to:
    /// - Rewrite or annotate the final output.
    /// - Perform cleanup (close resources, flush logs, emit analytics).
    /// - Post-process state (e.g. save a compacted checkpoint).
    async fn after_solve(
        &mut self,
        _state: &mut S,
        _cx: &mut Context<M, A>,
        _output: &mut Option<String>,
    ) -> InterceptorFlow {
        InterceptorFlow::Continue
    }
}

/// Type alias for a heap-allocated, dynamically-dispatched interceptor.
pub type DynInterceptor<S, M, A> = Box<dyn Interceptor<S, M, A>>;

/// Registry that owns a list of [`Interceptor`]s and dispatches lifecycle
/// events to each one in insertion order.
///
/// # Short-circuit semantics
///
/// For every lifecycle node, the registry iterates through all registered
/// interceptors and stops at the first one that returns a non-`Continue`
/// value. That value (whether `Stop` or `Skip`) is returned immediately;
/// remaining interceptors at the same node are **not** called.
///
/// This means insertion order matters: put high-priority / guard-like
/// interceptors first.
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
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    /// Append a single interceptor (boxes it internally).
    pub fn push<I>(&mut self, interceptor: I)
    where
        I: Interceptor<S, M, A> + 'static,
    {
        self.interceptors.push(Box::new(interceptor));
    }

    /// Replace all interceptors with a pre-boxed list.
    ///
    /// This is the building block for `Agent::rebuild_interceptors`.
    pub fn set(&mut self, interceptors: Vec<DynInterceptor<S, M, A>>) {
        self.interceptors = interceptors;
    }

    /// Dispatch [`Interceptor::before_solve`] to all registered interceptors.
    /// Called by [`Agent::solve`](crate::ai::agent::Agent::solve) once before
    /// the loop begins.
    pub async fn before_solve(&mut self, state: &mut S, cx: &mut Context<M, A>) -> InterceptorFlow {
        for interceptor in &mut self.interceptors {
            let flow = interceptor.before_solve(state, cx).await;
            if flow != InterceptorFlow::Continue {
                return flow;
            }
        }

        InterceptorFlow::Continue
    }

    /// Dispatch [`Interceptor::before_loop`] to all registered interceptors.
    /// Called at the start of each solve-loop iteration.
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

    /// Dispatch [`Interceptor::before_resolve`] to all registered interceptors.
    /// Called right before the resolver is invoked in each loop iteration.
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

    /// Dispatch [`Interceptor::after_resolve`] to all registered interceptors.
    /// Called right after the resolver returns an [`Action`], before tool
    /// execution or message commit.
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

    /// Dispatch [`Interceptor::before_tool_call`] to all registered interceptors.
    /// Called once per tool call in the action, **before** execution.
    ///
    /// Unlike other hooks, this returns [`ToolInterceptorFlow`] which also
    /// supports `Skip` (short-circuit with fake result).
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

    /// Dispatch [`Interceptor::after_tool_call`] to all registered interceptors.
    /// Called once per tool call, **after** execution, with the mutable
    /// result.
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

    /// Dispatch [`Interceptor::before_commit_messages`] to all registered
    /// interceptors. Called before the action and tool-result messages are
    /// written into the [`Context`].
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

    /// Dispatch [`Interceptor::after_loop`] to all registered interceptors.
    /// Called at the end of each loop iteration, after messages are committed,
    /// before deciding whether to continue or finish.
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

    /// Dispatch [`Interceptor::after_solve`] to all registered interceptors.
    /// Called once after the solve loop finishes (naturally or via `Stop`),
    /// right before the agent returns.
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
