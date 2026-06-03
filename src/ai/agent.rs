use std::collections::HashMap;
use std::collections::HashSet;
use uuid::Uuid;

use crate::ai::agent::persist::codec::ContextSnapshotCodec;
use crate::ai::agent::persist::codec::OpenAiCodec;
use crate::ai::agent::persist::entity::Checkpoint;
use crate::ai::agent::persist::entity::CheckpointKind;
use crate::ai::agent::persist::entity::ContextSnapshot;
use crate::ai::agent::persist::entity::NewCheckpoint;
use crate::ai::agent::persist::entity::NewSession;
use crate::ai::agent::persist::entity::Session;
use crate::ai::agent::persist::store::Store;
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
pub mod openai;
pub mod persist;
pub mod tool;

pub type Compact<M> = fn(&mut Context<M>);

pub struct Agent<M, R>
where
    M: IMessage + Clone + 'static,
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
    M: IMessage + Clone + 'static,
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

    pub fn snapshot_messages(&self) -> Vec<M> {
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
    M: IMessage + Clone + 'static,
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
    M: IMessage + Clone + 'static,
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

pub struct AgentManager<S, C> {
    store: S,
    codec: C,
}

impl<S> AgentManager<S, OpenAiCodec> {
    pub fn new_openai(store: S) -> Self {
        Self {
            store,
            codec: OpenAiCodec,
        }
    }
}

impl<S, C> AgentManager<S, C> {
    pub fn new(store: S, codec: C) -> Self {
        Self { store, codec }
    }

    pub fn store(&self) -> &S {
        &self.store
    }
}

impl<S, C> AgentManager<S, C>
where
    S: Store,
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
                parent_session_id: None,
                parent_checkpoint_id: None,
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

    pub async fn fork_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)> {
        self.store
            .fork_session_from_checkpoint(parent_checkpoint_id, name)
            .await
    }
}

impl<S, C> AgentManager<S, C>
where
    S: Store,
{
    pub async fn checkpoint_before_run<M, R>(
        &self,
        session_id: Uuid,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        M: IMessage + Clone + 'static,
        R: IResolver<Message = M> + Send,
        C: ContextSnapshotCodec<M>,
    {
        let run_id = Uuid::new_v4();
        self.create_agent_checkpoint(session_id, Some(run_id), CheckpointKind::BeforeRun, agent)
            .await
    }

    pub async fn checkpoint_after_run<M, R>(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        M: IMessage + Clone + 'static,
        R: IResolver<Message = M> + Send,
        C: ContextSnapshotCodec<M>,
    {
        self.create_agent_checkpoint(session_id, Some(run_id), CheckpointKind::AfterRun, agent)
            .await
    }

    pub fn decode_checkpoint<M>(&self, checkpoint: &Checkpoint) -> anyhow::Result<Context<M>>
    where
        M: IMessage + Clone + 'static,
        C: ContextSnapshotCodec<M>,
    {
        self.codec.decode_context(&checkpoint.snapshot)
    }

    pub fn snapshot_from_agent<M, R>(&self, agent: &Agent<M, R>) -> anyhow::Result<ContextSnapshot>
    where
        M: IMessage + Clone + 'static,
        R: IResolver<Message = M> + Send,
        C: ContextSnapshotCodec<M>,
    {
        self.codec.encode_context(agent.context())
    }

    async fn create_agent_checkpoint<M, R>(
        &self,
        session_id: Uuid,
        run_id: Option<Uuid>,
        kind: CheckpointKind,
        agent: &Agent<M, R>,
    ) -> anyhow::Result<Checkpoint>
    where
        M: IMessage + Clone + 'static,
        R: IResolver<Message = M> + Send,
        C: ContextSnapshotCodec<M>,
    {
        let snapshot = self.snapshot_from_agent(agent)?;
        self.store
            .create_checkpoint(NewCheckpoint {
                session_id,
                run_id,
                kind,
                snapshot,
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

    use crate::ai::agent::persist::codec::OpenAiCodec;
    use crate::ai::agent::persist::entity::Status;
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

    #[async_trait::async_trait]
    impl Store for FakeStore {
        async fn create_session(&self, input: NewSession) -> anyhow::Result<Session> {
            let session = Session {
                id: Uuid::new_v4(),
                name: input.name,
                model: input.model,
                status: Status::Active,
                parent_session_id: input.parent_session_id,
                parent_checkpoint_id: input.parent_checkpoint_id,
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
                run_id: input.run_id,
                kind: input.kind,
                snapshot: input.snapshot,
                created_at: chrono::Utc::now(),
            };
            self.state.lock().await.checkpoints.push(checkpoint.clone());
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

        async fn fork_session_from_checkpoint(
            &self,
            parent_checkpoint_id: Uuid,
            name: Option<String>,
        ) -> anyhow::Result<(Session, Checkpoint)> {
            self.state
                .lock()
                .await
                .forks
                .push((parent_checkpoint_id, name.clone()));

            let parent = self.get_checkpoint(parent_checkpoint_id).await?;
            let session = self
                .create_session(NewSession {
                    name,
                    model: parent.snapshot.model.clone(),
                    parent_session_id: Some(parent.session_id),
                    parent_checkpoint_id: Some(parent.id),
                })
                .await?;
            let checkpoint = self
                .create_checkpoint(NewCheckpoint {
                    session_id: session.id,
                    run_id: None,
                    kind: CheckpointKind::Fork,
                    snapshot: parent.snapshot,
                })
                .await?;

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
        let manager = AgentManager::new(store, OpenAiCodec);

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
        let manager = AgentManager::new(store, OpenAiCodec);
        let session_id = Uuid::new_v4();
        let agent = test_agent();

        let before = manager
            .checkpoint_before_run(session_id, &agent)
            .await
            .unwrap();
        let after = manager
            .checkpoint_after_run(session_id, before.run_id.unwrap(), &agent)
            .await
            .unwrap();
        let restored: Context<ChatCompletionMessageParam> =
            manager.decode_checkpoint(&after).unwrap();

        assert_eq!(before.kind, CheckpointKind::BeforeRun);
        assert_eq!(after.kind, CheckpointKind::AfterRun);
        assert_eq!(before.run_id, after.run_id);
        assert_eq!(after.snapshot.model, "deepseek-v4-flash");
        assert_eq!(after.snapshot.messages.len(), 2);
        assert_eq!(restored.model(), "deepseek-v4-flash");
        assert_eq!(restored.messages().len(), 2);
    }

    #[tokio::test]
    async fn manager_forks_from_checkpoint_through_store() {
        let store = FakeStore::default();
        let state = Arc::clone(&store.state);
        let manager = AgentManager::new(store, OpenAiCodec);
        let session_id = Uuid::new_v4();
        let agent = test_agent();
        let checkpoint = manager
            .checkpoint_before_run(session_id, &agent)
            .await
            .unwrap();

        let (fork, fork_checkpoint) = manager
            .fork_from_checkpoint(checkpoint.id, Some("forked".to_string()))
            .await
            .unwrap();

        let state = state.lock().await;
        assert_eq!(state.forks.len(), 1);
        assert_eq!(state.forks[0].0, checkpoint.id);
        assert_eq!(fork.name.as_deref(), Some("forked"));
        assert_eq!(fork.parent_session_id, Some(checkpoint.session_id));
        assert_eq!(fork.parent_checkpoint_id, Some(checkpoint.id));
        assert_eq!(fork_checkpoint.kind, CheckpointKind::Fork);
        assert_eq!(fork_checkpoint.snapshot, checkpoint.snapshot);
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
        let _old_tools = agent.replace_tools(vec![]);
        let _ = std::fs::remove_file(&target_file);
    }
}
