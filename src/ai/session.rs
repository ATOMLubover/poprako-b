use std::marker::PhantomData;

use openai_oxide::types::chat::ChatCompletionMessageParam;
use uuid::Uuid;

use crate::ai::agent::Agent;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::session::persist::codec::{IContextSnapshotCodec, OpenAiCodec};
use crate::ai::session::persist::data_object::{
    Checkpoint, CheckpointContext, CheckpointKind, ContextSnapshot, NewCheckpoint, NewSession,
    Session,
};
use crate::ai::session::persist::storage::IStorage;

pub mod persist;

pub struct SessionManager<S, M, C>
where
    M: IMessage + 'static,
    C: IContextSnapshotCodec<M>,
{
    store: S,
    codec: C,
    message: PhantomData<fn() -> M>,
}

impl<S> SessionManager<S, ChatCompletionMessageParam, OpenAiCodec> {
    pub fn new_openai(store: S) -> Self {
        Self {
            store,
            codec: OpenAiCodec,
            message: PhantomData,
        }
    }
}

impl<S, M, C> SessionManager<S, M, C>
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

impl<S, M, C> SessionManager<S, M, C>
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

    pub async fn checkpoint_before_solution<AS, R, A>(
        &self,
        session_id: Uuid,
        agent: &Agent<M, R, AS, A>,
    ) -> anyhow::Result<Checkpoint>
    where
        AS: Send + Sync + 'static,
        M: Send + Sync,
        R: IResolver<Message = M> + Send,
        A: Default + Send + Sync + 'static,
    {
        let solution_id = Uuid::new_v4();
        self.create_agent_checkpoint(
            session_id,
            Some(solution_id),
            CheckpointKind::BeforeSolution,
            agent,
        )
        .await
    }

    pub async fn checkpoint_after_solution<AS, R, A>(
        &self,
        session_id: Uuid,
        solution_id: Uuid,
        agent: &Agent<M, R, AS, A>,
    ) -> anyhow::Result<Checkpoint>
    where
        AS: Send + Sync + 'static,
        M: Send + Sync,
        R: IResolver<Message = M> + Send,
        A: Default + Send + Sync + 'static,
    {
        self.create_agent_checkpoint(
            session_id,
            Some(solution_id),
            CheckpointKind::AfterSolution,
            agent,
        )
        .await
    }

    /// Encode agent context into a `ContextSnapshot` via the codec.
    pub fn encode_snapshot<AS, R, A>(
        &self,
        agent: &Agent<M, R, AS, A>,
    ) -> anyhow::Result<ContextSnapshot>
    where
        AS: Send + Sync + 'static,
        M: Send + Sync,
        R: IResolver<Message = M> + Send,
        A: Default + Send + Sync + 'static,
    {
        self.codec.encode_context(agent.context())
    }

    /// Decode a `ContextSnapshot` into a resolver `Context<M>`.
    pub fn decode_snapshot(&self, snapshot: &ContextSnapshot) -> anyhow::Result<Context<M>> {
        self.codec.decode_context(snapshot)
    }

    async fn create_agent_checkpoint<AS, R, A>(
        &self,
        session_id: Uuid,
        solution_id: Option<Uuid>,
        kind: CheckpointKind,
        agent: &Agent<M, R, AS, A>,
    ) -> anyhow::Result<Checkpoint>
    where
        AS: Send + Sync + 'static,
        M: Send + Sync,
        R: IResolver<Message = M> + Send,
        A: Default + Send + Sync + 'static,
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

    use std::collections::HashMap;
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use openai_oxide::types::chat::UserContent;

    use crate::ai::agent::AgentBuilder;
    use crate::ai::resolver::action::{Action, Reason};
    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::result::ResolveResult;
    use crate::ai::session::persist::data_object::{Message, PersistDiagnostics, Status};

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

        async fn resolve<A>(
            &mut self,
            _cx: &Context<Self::Message, A>,
        ) -> ResolveResult<Action<<Self::Message as IMessage>::ToolCall>>
        where
            A: Send + Sync + 'static,
        {
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
            state.contexts.insert(checkpoint.id, input.messages.clone());
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

        async fn persist_diagnostics(&self) -> anyhow::Result<PersistDiagnostics> {
            let state = self.state.lock().await;
            Ok(PersistDiagnostics {
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
            state.contexts.insert(checkpoint.id, parent_messages);
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
        let manager = SessionManager::new_openai(store);

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
        let manager = SessionManager::new_openai(store);
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

        let ctx = manager.load_checkpoint_context(after.id).await.unwrap();
        let restored: Context<ChatCompletionMessageParam> =
            manager.decode_snapshot(&ctx.snapshot).unwrap();

        assert_eq!(before.kind, CheckpointKind::BeforeSolution);
        assert_eq!(after.kind, CheckpointKind::AfterSolution);
        assert_eq!(before.solution_id, after.solution_id);
        assert_eq!(ctx.snapshot.model, "deepseek-v4-flash");
        assert_eq!(ctx.snapshot.messages.len(), 2);
        assert_eq!(restored.model(), "deepseek-v4-flash");
        assert_eq!(restored.message_count(), 2);
    }

    #[tokio::test]
    async fn manager_forks_from_checkpoint_through_store() {
        let store = FakeStore::default();
        let state = Arc::clone(&store.state);
        let manager = SessionManager::new_openai(store);
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
        assert_eq!(fork.forked_from_checkpoint_id, Some(checkpoint.id));
        assert_eq!(fork_checkpoint.kind, CheckpointKind::Fork);
        assert_eq!(fork_checkpoint.base_checkpoint_id, Some(checkpoint.id));

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
}
