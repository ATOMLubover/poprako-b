use async_trait::async_trait;
use uuid::Uuid;

pub mod rdb;

use crate::ai::agent::persist::data_object::Checkpoint;
use crate::ai::agent::persist::data_object::CheckpointContext;
use crate::ai::agent::persist::data_object::NewCheckpoint;
use crate::ai::agent::persist::data_object::NewSession;
use crate::ai::agent::persist::data_object::Session;

#[async_trait]
pub trait IStorage {
    // ── Session ──────────────────────────────────────────────────────────

    async fn create_session(&self, input: NewSession) -> anyhow::Result<Session>;

    async fn get_session(&self, session_id: Uuid) -> anyhow::Result<Session>;

    async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session>;

    // ── Checkpoint ───────────────────────────────────────────────────────

    /// Create a checkpoint with incremental or reset semantics.
    ///
    /// The storage layer will:
    /// 1. Lock the session and find the latest checkpoint (or use
    ///    `forked_from_checkpoint_id` as the base).
    /// 2. Reconstruct the base message sequence.
    /// 3. If the input messages form a prefix-extension of the base, store
    ///    only the suffix refs with this checkpoint as the parent.
    /// 4. Otherwise, create a *reset* checkpoint (`base_checkpoint_id = NULL`)
    ///    that references all current messages.
    async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint>;

    /// Return checkpoint metadata only — does not materialise the context.
    async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint>;

    /// List checkpoint metadata for a session, ordered by creation time.
    async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>>;

    /// Load the full context for a checkpoint by walking the base chain and
    /// joining `agent_checkpoint_messages` + `agent_messages`.
    async fn load_checkpoint_context(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<CheckpointContext>;

    // ── Fork ─────────────────────────────────────────────────────────────

    /// Fork a session from an existing checkpoint.
    ///
    /// Creates a new `agent_sessions` row with `forked_from_checkpoint_id`
    /// pointing to the parent checkpoint, and a single `Fork` checkpoint
    /// whose `base_checkpoint_id` is the parent checkpoint.
    /// No messages are duplicated — the fork checkpoint has zero local refs.
    async fn fork_session_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)>;
}
