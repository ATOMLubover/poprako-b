use async_trait::async_trait;
use uuid::Uuid;

use crate::ai::agent::persist::entity::Checkpoint;
use crate::ai::agent::persist::entity::NewCheckpoint;
use crate::ai::agent::persist::entity::NewSession;
use crate::ai::agent::persist::entity::Session;

#[async_trait]
pub trait Store: Send + Sync {
    async fn create_session(&self, input: NewSession) -> anyhow::Result<Session>;

    async fn get_session(&self, session_id: Uuid) -> anyhow::Result<Session>;

    async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session>;

    async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint>;

    async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint>;

    async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>>;

    async fn fork_session_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)>;
}
