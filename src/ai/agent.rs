use std::collections::HashMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use uuid::Uuid;

use crate::ai::agent::persist::codec::IContextSnapshotCodec;
use crate::ai::agent::persist::codec::OpenAiCodec;
use crate::ai::agent::persist::data_object::Checkpoint;
use crate::ai::agent::persist::data_object::CheckpointContext;
use crate::ai::agent::persist::data_object::CheckpointKind;
use crate::ai::agent::persist::data_object::ContextSnapshot;
use crate::ai::agent::persist::data_object::NewCheckpoint;
use crate::ai::agent::persist::data_object::NewSession;
use crate::ai::agent::persist::data_object::Session;
use crate::ai::agent::persist::storage::IStorage;
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
use openai_oxide::types::chat::ChatCompletionMessageParam;

pub mod compact;
pub mod openai;
pub mod persist;
pub mod tool;

pub type Compact<M> = fn(&mut Context<M>);

pub struct Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    local_tools: HashMap<String, DynTool>,
    remote_proxy: Option<RemoteProxy>,

    resolver: R,

    compact: Option<Compact<M>>,
}

impl<M, R> Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn from_context(cx: Context<M>, resolver: R) -> Self {
        Self {
            context: cx,
            local_tools: HashMap::new(),
            remote_proxy: None,
            resolver,
            compact: None,
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

    pub fn push_message(&mut self, message: M) {
        self.context.push_message(message);
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.context.set_messages(messages);
    }

    pub fn snapshot_messages(&self) -> Vec<M>
    where
        M: Clone,
    {
        self.context.messages().to_vec()
    }

    pub fn context(&self) -> &Context<M> {
        &self.context
    }

    /// Replace the first message (system prompt) while keeping the rest intact.
    /// If the message list is empty, the new message is pushed as the sole entry.
    pub fn replace_system_message(&mut self, message: M) {
        let mut messages = self.context.take_messages();
        if messages.is_empty() {
            messages.push(message);
        } else {
            messages[0] = message;
        }
        self.context.set_messages(messages);
    }

    pub fn set_compact(&mut self, compact: Compact<M>) {
        self.compact = Some(compact);
    }

    /// Replace all registered tools, returning the old ones.
    pub fn replace_tools(&mut self, tools: Vec<DynTool>) -> Vec<DynTool> {
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
        loop {
            let action = match self.resolver.resolve(&self.context).await {
                Ok(action) => {
                    tracing::info!("resolver produced action: {:?}", action);
                    action
                }
                Err(e) => {
                    tracing::error!("resolve failed: {:?}", e);
                    return None;
                }
            };

            let reason = action.reason.clone();

            // Extract finish content before action is consumed.
            let finish_content = if matches!(reason, Reason::Finish) {
                action.content.clone()
            } else {
                None
            };

            // Process tool calls from the local action — no borrow on self.context.
            let tool_messages: Vec<M> = match &action.tool_calls {
                None => Vec::new(),
                Some(calls) => {
                    let mut messages = Vec::with_capacity(calls.len());
                    for call in calls {
                        let result = self.handle_call(call).await;
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

            // Push assistant message and tool results to context together.
            self.context.push_message(action.into());
            for msg in tool_messages {
                self.context.push_message(msg);
            }

            match reason {
                Reason::Finish => return finish_content,
                // FIXME: ToolCall, Length, Unknown → continue the loop.
                _ => continue,
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
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    resolver: R,
    tools: Vec<DynTool>,
    remote_proxy: Option<RemoteProxy>,
    compact: Option<Compact<M>>,
}

impl<M, R> AgentBuilder<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn new(context: Context<M>, resolver: R) -> Self {
        Self {
            context,
            resolver,
            tools: Vec::new(),
            remote_proxy: None,
            compact: None,
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

    pub fn build(self) -> Agent<M, R> {
        let mut agent = Agent::from_context(self.context, self.resolver);

        agent.compact = self.compact;
        agent.remote_proxy = self.remote_proxy;
        agent.set_tools(self.tools);

        agent
    }
}

pub struct AgentManager<S, M, C>
where
    M: IMessage + 'static,
    C: IContextSnapshotCodec<M>,
{
    store: S,
    codec: C,
    message: PhantomData<fn() -> M>,
}

impl<S> AgentManager<S, ChatCompletionMessageParam, OpenAiCodec> {
    pub fn new_openai(store: S) -> Self {
        Self {
            store,
            codec: OpenAiCodec,
            message: PhantomData,
        }
    }
}

impl<S, M, C> AgentManager<S, M, C>
where
    M: IMessage + 'static,
    C: IContextSnapshotCodec<M>,
{
    pub fn new(store: S, codec: C) -> Self {
        Self {
            store,
            codec,
            message: PhantomData,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }
}

impl<S, M, C> AgentManager<S, M, C>
where
    S: IStorage,
    M: IMessage + 'static,
    C: IContextSnapshotCodec<M>,
{
    pub async fn create_session(
        &self,
        model: impl Into<String>,
        name: Option<String>,
    ) -> anyhow::Result<Session> {
        self.store
            .create_session(NewSession {
                name,
                model: model.into(),
                forked_from_checkpoint_id: None,
            })
            .await
    }

    pub async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
        self.store.archive_session(session_id).await
    }

    pub async fn load_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint> {
        self.store.get_checkpoint(checkpoint_id).await
    }

    pub async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>> {
        self.store.list_checkpoints(session_id).await
    }

    /// Load full checkpoint context (metadata + reconstructed messages) from storage.
    pub async fn load_checkpoint_context(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<CheckpointContext> {
        self.store.load_checkpoint_context(checkpoint_id).await
    }

    pub async fn fork_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)> {
        self.store
            .fork_session_from_checkpoint(parent_checkpoint_id, name)
            .await
    }

    pub async fn checkpoint_before_solution<R>(
        &self,
        session_id: Uuid,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        R: IResolver<Message = M> + Send,
    {
        let solution_id = Uuid::new_v4();
        self.create_agent_checkpoint(session_id, Some(solution_id), CheckpointKind::BeforeSolution, agent)
            .await
    }

    pub async fn checkpoint_after_solution<R>(
        &self,
        session_id: Uuid,
        solution_id: Uuid,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        R: IResolver<Message = M> + Send,
    {
        self.create_agent_checkpoint(session_id, Some(solution_id), CheckpointKind::AfterSolution, agent)
            .await
    }

    /// Encode agent context into a `ContextSnapshot` via the codec.
    pub fn encode_snapshot<R>(&self, agent: &Agent<M, R>) -> anyhow::Result<ContextSnapshot>
    where
        R: IResolver<Message = M> + Send,
    {
        self.codec.encode_context(agent.context())
    }

    /// Decode a `ContextSnapshot` into a resolver `Context<M>`.
    pub fn decode_snapshot(&self, snapshot: &ContextSnapshot) -> anyhow::Result<Context<M>> {
        self.codec.decode_context(snapshot)
    }

    async fn create_agent_checkpoint<R>(
        &self,
        session_id: Uuid,
        solution_id: Option<Uuid>,
        kind: CheckpointKind,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        R: IResolver<Message = M> + Send,
    {
        let snapshot = self.encode_snapshot(agent)?;
        self.store
            .create_checkpoint(NewCheckpoint {
                session_id,
                solution_id,
                kind,
                model: snapshot.model,
                messages: snapshot.messages,
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use tokio::sync::Mutex;

    use std::path::PathBuf;

    use crate::ai::agent::persist::data_object::Message;
    use crate::ai::agent::persist::data_object::Status;
    use crate::ai::agent::tool::local::fs::{CreateFileTool, ReadFileTool};
    use crate::ai::resolver::action::Action;
    use crate::ai::resolver::action::Reason;
    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::openai::OpenAiResolver;
    use crate::ai::resolver::result::ResolveResult;

    use openai_oxide::types::chat::ChatCompletionMessageParam;
    use openai_oxide::types::chat::UserContent;

    #[derive(Default)]
    struct FakeStore {
        state: Arc<Mutex<FakeState>>,
    }

    #[derive(Default)]
    struct FakeState {
        sessions: Vec<Session>,
        checkpoints: Vec<Checkpoint>,
        /// (checkpoint_id -> Vec<Message>) storing the reconstructed context
        /// that would be produced by load_checkpoint_context.
        contexts: HashMap<Uuid, Vec<Message>>,
        forks: Vec<(Uuid, Option<String>)>,
    }

    struct FakeResolver;

    #[async_trait::async_trait]
    impl IResolver for FakeResolver {
        type Message = ChatCompletionMessageParam;

        async fn resolve(
            &mut self,
            _cx: &Context<Self::Message>,
        ) -> ResolveResult<Action<<Self::Message as IMessage>::ToolCall>> {
            Ok(Action {
                reason: Reason::Finish,
                content: Some("done".to_string()),
                refusal: None,
                tool_calls: None,
            })
        }
    }

    fn make_snapshot(model: &str, messages: Vec<Message>) -> ContextSnapshot {
        ContextSnapshot {
            model: model.to_string(),
            messages,
        }
    }

    #[async_trait::async_trait]
    impl IStorage for FakeStore {
        async fn create_session(&self, input: NewSession) -> anyhow::Result<Session> {
            let session = Session {
                id: Uuid::new_v4(),
                name: input.name,
                model: input.model,
                status: Status::Active,
                forked_from_checkpoint_id: input.forked_from_checkpoint_id,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            self.state.lock().await.sessions.push(session.clone());
            Ok(session)
        }

        async fn get_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
            self.state
                .lock()
                .await
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("session not found"))
        }

        async fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
            Ok(self
                .state
                .lock()
                .await
                .sessions
                .iter()
                .filter(|session| session.status == Status::Active)
                .cloned()
                .collect())
        }

        async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
            let mut state = self.state.lock().await;
            let session = state
                .sessions
                .iter_mut()
                .find(|session| session.id == session_id)
                .ok_or_else(|| anyhow::anyhow!("session not found"))?;
            session.status = Status::Archived;
            Ok(session.clone())
        }

        async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint> {
            let checkpoint = Checkpoint {
                id: Uuid::new_v4(),
                session_id: input.session_id,
                solution_id: input.solution_id,
                kind: input.kind,
                model: input.model,
                base_checkpoint_id: None,
                created_at: chrono::Utc::now(),
            };
            let mut state = self.state.lock().await;
            state
                .contexts
                .insert(checkpoint.id, input.messages.clone());
            state.checkpoints.push(checkpoint.clone());
            Ok(checkpoint)
        }

        async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint> {
            self.state
                .lock()
                .await
                .checkpoints
                .iter()
                .find(|checkpoint| checkpoint.id == checkpoint_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("checkpoint not found"))
        }

        async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>> {
            Ok(self
                .state
                .lock()
                .await
                .checkpoints
                .iter()
                .filter(|checkpoint| checkpoint.session_id == session_id)
                .cloned()
                .collect())
        }

        async fn load_checkpoint_context(
            &self,
            checkpoint_id: Uuid,
        ) -> anyhow::Result<CheckpointContext> {
            let state = self.state.lock().await;
            let checkpoint = state
                .checkpoints
                .iter()
                .find(|c| c.id == checkpoint_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("checkpoint not found"))?;
            let messages = state
                .contexts
                .get(&checkpoint_id)
                .cloned()
                .unwrap_or_default();
            Ok(CheckpointContext {
                checkpoint,
                snapshot: make_snapshot("deepseek-v4-flash", messages),
            })
        }

        async fn checkpoint_local_ref_count(&self, checkpoint_id: Uuid) -> anyhow::Result<i64> {
            let state = self.state.lock().await;
            let count = state
                .contexts
                .get(&checkpoint_id)
                .map(|messages| messages.len() as i64)
                .unwrap_or(0);
            Ok(count)
        }

        async fn persist_diagnostics(
            &self,
        ) -> anyhow::Result<crate::ai::agent::persist::data_object::PersistDiagnostics> {
            let state = self.state.lock().await;
            Ok(crate::ai::agent::persist::data_object::PersistDiagnostics {
                session_count: state
                    .sessions
                    .iter()
                    .filter(|session| session.status == Status::Active)
                    .count() as i64,
                checkpoint_count: state.checkpoints.len() as i64,
                message_count: state.contexts.values().flatten().count() as i64,
                checkpoint_local_ref_count: state.contexts.values().flatten().count() as i64,
            })
        }

        async fn fork_session_from_checkpoint(
            &self,
            parent_checkpoint_id: Uuid,
            name: Option<String>,
        ) -> anyhow::Result<(Session, Checkpoint)> {
            // Clone name before moving it into create_session.
            let fork_name = name.clone();

            // Fetch parent data under a short lock, then construct new entities
            // outside the lock to avoid deadlocking with self.create_session().
            let (parent_model, parent_messages) = {
                let state = self.state.lock().await;
                let parent = state
                    .checkpoints
                    .iter()
                    .find(|c| c.id == parent_checkpoint_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("parent checkpoint not found"))?;
                let msgs = state
                    .contexts
                    .get(&parent_checkpoint_id)
                    .cloned()
                    .unwrap_or_default();
                (parent.model.clone(), msgs)
            };

            let session = self
                .create_session(NewSession {
                    name,
                    model: parent_model.clone(),
                    forked_from_checkpoint_id: Some(parent_checkpoint_id),
                })
                .await?;

            let checkpoint = Checkpoint {
                id: Uuid::new_v4(),
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::Fork,
                model: parent_model,
                base_checkpoint_id: Some(parent_checkpoint_id),
                created_at: chrono::Utc::now(),
            };

            let mut state = self.state.lock().await;
            state.forks.push((parent_checkpoint_id, fork_name));
            state
                .contexts
                .insert(checkpoint.id, parent_messages);
            state.checkpoints.push(checkpoint.clone());

            Ok((session, checkpoint))
        }
    }

    fn test_agent() -> Agent<ChatCompletionMessageParam, FakeResolver> {
        let context = ContextBuilder::new("deepseek-v4-flash")
            .messages(vec![
                ChatCompletionMessageParam::System {
                    content: "system".to_string(),
                    name: None,
                },
                ChatCompletionMessageParam::User {
                    content: UserContent::Text("hello".to_string()),
                    name: None,
                },
            ])
            .build();

        AgentBuilder::new(context, FakeResolver).build()
    }

    #[tokio::test]
    async fn manager_creates_and_archives_session() {
        let store = FakeStore::default();
        let manager = AgentManager::new_openai(store);

        let session = manager
            .create_session("deepseek-v4-flash", Some("test-session".to_string()))
            .await
            .unwrap();
        let archived = manager.archive_session(session.id).await.unwrap();

        assert_eq!(session.name.as_deref(), Some("test-session"));
        assert_eq!(session.model, "deepseek-v4-flash");
        assert_eq!(archived.status, Status::Archived);
    }

    #[tokio::test]
    async fn manager_checkpoints_agent_context_and_decodes_it() {
        let store = FakeStore::default();
        let manager = AgentManager::new_openai(store);
        let session_id = Uuid::new_v4();
        let agent = test_agent();

        let before = manager
            .checkpoint_before_solution(session_id, &agent)
            .await
            .unwrap();
        let after = manager
            .checkpoint_after_solution(session_id, before.solution_id.unwrap(), &agent)
            .await
            .unwrap();

        // Load context from after checkpoint.
        let ctx = manager.load_checkpoint_context(after.id).await.unwrap();
        let restored: Context<ChatCompletionMessageParam> =
            manager.decode_snapshot(&ctx.snapshot).unwrap();

        assert_eq!(before.kind, CheckpointKind::BeforeSolution);
        assert_eq!(after.kind, CheckpointKind::AfterSolution);
        assert_eq!(before.solution_id, after.solution_id);
        assert_eq!(ctx.snapshot.model, "deepseek-v4-flash");
        assert_eq!(ctx.snapshot.messages.len(), 2);
        assert_eq!(restored.model(), "deepseek-v4-flash");
        assert_eq!(restored.messages().len(), 2);
    }

    #[tokio::test]
    async fn manager_forks_from_checkpoint_through_store() {
        let store = FakeStore::default();
        let state = Arc::clone(&store.state);
        let manager = AgentManager::new_openai(store);
        let session_id = Uuid::new_v4();
        let agent = test_agent();
        let checkpoint = manager
            .checkpoint_before_solution(session_id, &agent)
            .await
            .unwrap();

        let (fork, fork_checkpoint) = manager
            .fork_from_checkpoint(checkpoint.id, Some("forked".to_string()))
            .await
            .unwrap();

        {
            let fstate = state.lock().await;
            assert_eq!(fstate.forks.len(), 1);
            assert_eq!(fstate.forks[0].0, checkpoint.id);
        }
        assert_eq!(fork.name.as_deref(), Some("forked"));
        assert_eq!(
            fork.forked_from_checkpoint_id,
            Some(checkpoint.id)
        );
        assert_eq!(fork_checkpoint.kind, CheckpointKind::Fork);
        assert_eq!(
            fork_checkpoint.base_checkpoint_id,
            Some(checkpoint.id)
        );

        // Fork checkpoint context should match parent.
        let fork_ctx = manager
            .load_checkpoint_context(fork_checkpoint.id)
            .await
            .unwrap();
        let parent_ctx = manager
            .load_checkpoint_context(checkpoint.id)
            .await
            .unwrap();
        assert_eq!(fork_ctx.snapshot.messages, parent_ctx.snapshot.messages);
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

        let contents =
            std::fs::read_to_string(&target_file).expect("should read created file");
        assert!(
            contents.contains("hello from agent"),
            "file should contain 'hello from agent', got: {contents}"
        );

        // Pop CreateFileTool out before agent drops, then clean up the file directly.
        let _old_tools = agent.replace_tools(vec![]);
        let _ = std::fs::remove_file(&target_file);
    }
}
