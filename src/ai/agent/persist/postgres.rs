use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ai::agent::persist::entity::Checkpoint;
use crate::ai::agent::persist::entity::CheckpointKind;
use crate::ai::agent::persist::entity::ContextSnapshot;
use crate::ai::agent::persist::entity::Message;
use crate::ai::agent::persist::entity::NewCheckpoint;
use crate::ai::agent::persist::entity::NewSession;
use crate::ai::agent::persist::entity::Session;
use crate::ai::agent::persist::entity::Status;
use crate::ai::agent::persist::store::Store;

#[derive(Clone)]
pub struct Storage {
    pool: PgPool,
}

impl Storage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL not set in environment"))?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        Ok(Self::new(pool))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl Store for Storage {
    async fn create_session(&self, input: NewSession) -> anyhow::Result<Session> {
        let session = new_session(input);
        let status = session.status.as_str();
        let row = sqlx::query!(
            r#"
            INSERT INTO agent_sessions (
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            "#,
            session.id,
            session.name,
            session.model,
            status,
            session.parent_session_id,
            session.parent_checkpoint_id,
            session.created_at,
            session.updated_at,
        )
        .fetch_one(&self.pool)
        .await?;

        session_from_fields(
            row.id,
            row.name,
            row.model,
            row.status,
            row.parent_session_id,
            row.parent_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
    }

    async fn get_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            FROM agent_sessions
            WHERE id = $1
            "#,
            session_id,
        )
        .fetch_one(&self.pool)
        .await?;

        session_from_fields(
            row.id,
            row.name,
            row.model,
            row.status,
            row.parent_session_id,
            row.parent_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
    }

    async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
        let now = Utc::now();
        let status = Status::Archived.as_str();
        let row = sqlx::query!(
            r#"
            UPDATE agent_sessions
            SET
                status = $2,
                updated_at = $3
            WHERE id = $1
            RETURNING
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            "#,
            session_id,
            status,
            now,
        )
        .fetch_one(&self.pool)
        .await?;

        session_from_fields(
            row.id,
            row.name,
            row.model,
            row.status,
            row.parent_session_id,
            row.parent_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
    }

    async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint> {
        let checkpoint = new_checkpoint(input);
        let kind = checkpoint.kind.as_str();
        let messages = Json(checkpoint.snapshot.messages);
        let row = sqlx::query!(
            r#"
            INSERT INTO agent_checkpoints (
                id,
                session_id,
                run_id,
                kind,
                model,
                messages,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                session_id,
                run_id,
                kind,
                model,
                messages AS "messages: Json<Vec<Message>>",
                created_at
            "#,
            checkpoint.id,
            checkpoint.session_id,
            checkpoint.run_id,
            kind,
            checkpoint.snapshot.model,
            messages as Json<Vec<Message>>,
            checkpoint.created_at,
        )
        .fetch_one(&self.pool)
        .await?;

        checkpoint_from_fields(
            row.id,
            row.session_id,
            row.run_id,
            row.kind,
            row.model,
            row.messages,
            row.created_at,
        )
    }

    async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                run_id,
                kind,
                model,
                messages AS "messages: Json<Vec<Message>>",
                created_at
            FROM agent_checkpoints
            WHERE id = $1
            "#,
            checkpoint_id,
        )
        .fetch_one(&self.pool)
        .await?;

        checkpoint_from_fields(
            row.id,
            row.session_id,
            row.run_id,
            row.kind,
            row.model,
            row.messages,
            row.created_at,
        )
    }

    async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                run_id,
                kind,
                model,
                messages AS "messages: Json<Vec<Message>>",
                created_at
            FROM agent_checkpoints
            WHERE session_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
            session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                checkpoint_from_fields(
                    row.id,
                    row.session_id,
                    row.run_id,
                    row.kind,
                    row.model,
                    row.messages,
                    row.created_at,
                )
            })
            .collect()
    }

    async fn fork_session_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)> {
        let mut tx = self.pool.begin().await?;

        let parent_row = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                run_id,
                kind,
                model,
                messages AS "messages: Json<Vec<Message>>",
                created_at
            FROM agent_checkpoints
            WHERE id = $1
            "#,
            parent_checkpoint_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        let parent_checkpoint = checkpoint_from_fields(
            parent_row.id,
            parent_row.session_id,
            parent_row.run_id,
            parent_row.kind,
            parent_row.model,
            parent_row.messages,
            parent_row.created_at,
        )?;

        let session = new_session(NewSession {
            name,
            model: parent_checkpoint.snapshot.model.clone(),
            parent_session_id: Some(parent_checkpoint.session_id),
            parent_checkpoint_id: Some(parent_checkpoint.id),
        });
        let status = session.status.as_str();
        let session_row = sqlx::query!(
            r#"
            INSERT INTO agent_sessions (
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                name,
                model,
                status,
                parent_session_id,
                parent_checkpoint_id,
                created_at,
                updated_at
            "#,
            session.id,
            session.name,
            session.model,
            status,
            session.parent_session_id,
            session.parent_checkpoint_id,
            session.created_at,
            session.updated_at,
        )
        .fetch_one(&mut *tx)
        .await?;
        let session = session_from_fields(
            session_row.id,
            session_row.name,
            session_row.model,
            session_row.status,
            session_row.parent_session_id,
            session_row.parent_checkpoint_id,
            session_row.created_at,
            session_row.updated_at,
        )?;

        let checkpoint = new_checkpoint(NewCheckpoint {
            session_id: session.id,
            run_id: None,
            kind: CheckpointKind::Fork,
            snapshot: parent_checkpoint.snapshot,
        });
        let kind = checkpoint.kind.as_str();
        let messages = Json(checkpoint.snapshot.messages);
        let checkpoint_row = sqlx::query!(
            r#"
            INSERT INTO agent_checkpoints (
                id,
                session_id,
                run_id,
                kind,
                model,
                messages,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                session_id,
                run_id,
                kind,
                model,
                messages AS "messages: Json<Vec<Message>>",
                created_at
            "#,
            checkpoint.id,
            checkpoint.session_id,
            checkpoint.run_id,
            kind,
            checkpoint.snapshot.model,
            messages as Json<Vec<Message>>,
            checkpoint.created_at,
        )
        .fetch_one(&mut *tx)
        .await?;
        let checkpoint = checkpoint_from_fields(
            checkpoint_row.id,
            checkpoint_row.session_id,
            checkpoint_row.run_id,
            checkpoint_row.kind,
            checkpoint_row.model,
            checkpoint_row.messages,
            checkpoint_row.created_at,
        )?;

        tx.commit().await?;

        Ok((session, checkpoint))
    }
}

fn new_session(input: NewSession) -> Session {
    let now = Utc::now();
    Session {
        id: Uuid::new_v4(),
        name: input.name,
        model: input.model,
        status: Status::Active,
        parent_session_id: input.parent_session_id,
        parent_checkpoint_id: input.parent_checkpoint_id,
        created_at: now,
        updated_at: now,
    }
}

fn new_checkpoint(input: NewCheckpoint) -> Checkpoint {
    Checkpoint {
        id: Uuid::new_v4(),
        session_id: input.session_id,
        run_id: input.run_id,
        kind: input.kind,
        snapshot: input.snapshot,
        created_at: Utc::now(),
    }
}

fn session_from_fields(
    id: Uuid,
    name: Option<String>,
    model: String,
    status: String,
    parent_session_id: Option<Uuid>,
    parent_checkpoint_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> anyhow::Result<Session> {
    Ok(Session {
        id,
        name,
        model,
        status: Status::try_from(status.as_str())?,
        parent_session_id,
        parent_checkpoint_id,
        created_at,
        updated_at,
    })
}

fn checkpoint_from_fields(
    id: Uuid,
    session_id: Uuid,
    run_id: Option<Uuid>,
    kind: String,
    model: String,
    messages: Json<Vec<Message>>,
    created_at: DateTime<Utc>,
) -> anyhow::Result<Checkpoint> {
    Ok(Checkpoint {
        id,
        session_id,
        run_id,
        kind: CheckpointKind::try_from(kind.as_str())?,
        snapshot: ContextSnapshot {
            model,
            messages: messages.0,
        },
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(model: &str) -> ContextSnapshot {
        ContextSnapshot {
            model: model.to_string(),
            messages: vec![
                Message::System {
                    content: "system".to_string(),
                },
                Message::User {
                    content: "hello".to_string(),
                },
                Message::Assistant {
                    content: Some("calling".to_string()),
                    refusal: None,
                    tool_calls: Some(vec![crate::ai::agent::persist::entity::ToolCall {
                        id: "call_1".to_string(),
                        name: "lookup".to_string(),
                        args: "{\"q\":\"poprako\"}".to_string(),
                    }]),
                },
                Message::Tool {
                    tool_call_id: "call_1".to_string(),
                    content: "result".to_string(),
                },
            ],
        }
    }

    async fn storage() -> Storage {
        dotenvy::dotenv().ok();
        Storage::from_env().await.expect("storage should connect")
    }

    async fn cleanup(storage: &Storage, prefix: &str) {
        let pattern = format!("{}%", prefix);
        sqlx::query!(
            r#"
            UPDATE agent_sessions
            SET
                parent_session_id = NULL,
                parent_checkpoint_id = NULL
            WHERE name LIKE $1
            "#,
            pattern,
        )
        .execute(storage.pool())
        .await
        .expect("should clear test parent references");

        sqlx::query!(
            r#"
            DELETE FROM agent_sessions
            WHERE name LIKE $1
            "#,
            pattern,
        )
        .execute(storage.pool())
        .await
        .expect("should delete test sessions");
    }

    #[tokio::test]
    async fn session_lifecycle() {
        let storage = storage().await;
        let prefix = format!("test-session-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                parent_session_id: None,
                parent_checkpoint_id: None,
            })
            .await
            .unwrap();
        let loaded = storage.get_session(session.id).await.unwrap();
        let archived = storage.archive_session(session.id).await.unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.name, session.name);
        assert_eq!(loaded.model, "deepseek-v4-flash");
        assert_eq!(loaded.status, Status::Active);
        assert_eq!(archived.status, Status::Archived);
        assert!(archived.updated_at >= archived.created_at);

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn checkpoint_lifecycle_and_ordering() {
        let storage = storage().await;
        let prefix = format!("test-checkpoint-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                parent_session_id: None,
                parent_checkpoint_id: None,
            })
            .await
            .unwrap();
        let run_id = Uuid::new_v4();
        let before = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                run_id: Some(run_id),
                kind: CheckpointKind::BeforeRun,
                snapshot: snapshot("deepseek-v4-flash"),
            })
            .await
            .unwrap();
        let after_snapshot = snapshot("deepseek-v4-flash");
        let after = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                run_id: Some(run_id),
                kind: CheckpointKind::AfterRun,
                snapshot: after_snapshot.clone(),
            })
            .await
            .unwrap();

        let loaded = storage.get_checkpoint(after.id).await.unwrap();
        let checkpoints = storage.list_checkpoints(session.id).await.unwrap();

        assert_eq!(loaded.id, after.id);
        assert_eq!(loaded.snapshot, after_snapshot);
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].id, before.id);
        assert_eq!(checkpoints[1].id, after.id);
        assert_eq!(checkpoints[0].run_id, Some(run_id));
        assert_eq!(checkpoints[1].run_id, Some(run_id));

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn fork_copies_checkpoint_snapshot() {
        let storage = storage().await;
        let prefix = format!("test-fork-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                parent_session_id: None,
                parent_checkpoint_id: None,
            })
            .await
            .unwrap();
        let parent_snapshot = snapshot("deepseek-v4-flash");
        let parent_checkpoint = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                run_id: Some(Uuid::new_v4()),
                kind: CheckpointKind::AfterRun,
                snapshot: parent_snapshot.clone(),
            })
            .await
            .unwrap();

        let (fork, fork_checkpoint) = storage
            .fork_session_from_checkpoint(parent_checkpoint.id, Some(format!("{}fork", prefix)))
            .await
            .unwrap();
        let loaded_fork = storage.get_session(fork.id).await.unwrap();
        let loaded_checkpoint = storage.get_checkpoint(fork_checkpoint.id).await.unwrap();

        assert_eq!(loaded_fork.parent_session_id, Some(session.id));
        assert_eq!(loaded_fork.parent_checkpoint_id, Some(parent_checkpoint.id));
        assert_eq!(loaded_fork.model, parent_snapshot.model);
        assert_eq!(loaded_checkpoint.kind, CheckpointKind::Fork);
        assert_eq!(loaded_checkpoint.run_id, None);
        assert_eq!(loaded_checkpoint.snapshot, parent_snapshot);

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn checkpoint_requires_existing_session() {
        let storage = storage().await;
        let result = storage
            .create_checkpoint(NewCheckpoint {
                session_id: Uuid::new_v4(),
                run_id: None,
                kind: CheckpointKind::BeforeRun,
                snapshot: snapshot("deepseek-v4-flash"),
            })
            .await;

        assert!(result.is_err());
    }
}
