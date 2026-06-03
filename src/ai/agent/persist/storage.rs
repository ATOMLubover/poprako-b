use async_trait::async_trait;
use uuid::Uuid;

pub mod rdb;

use crate::ai::agent::persist::data_object::Checkpoint;
use crate::ai::agent::persist::data_object::NewCheckpoint;
use crate::ai::agent::persist::data_object::NewSession;
use crate::ai::agent::persist::data_object::Session;

#[async_trait]
pub trait IStorage {
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
