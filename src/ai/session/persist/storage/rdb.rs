use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

mod entity;

use entity::{
    CheckpointEntity, ReconstructedCheckpoint, SessionEntity, SessionStatus, upsert_row,
};

use crate::ai::session::persist::data_object::{
    Checkpoint, CheckpointContext, CheckpointKind, CheckpointMessageRef, ContextSnapshot, Message,
    NewCheckpoint, NewSession, PersistDiagnostics, Session,
};
use crate::ai::session::persist::storage::IStorage;

#[derive(Clone)]
pub struct RdbStorage {
    pool: PgPool,
}

impl RdbStorage {
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

// ── Internal helpers ─────────────────────────────────────────────────────────

fn prefix_match(base: &[Message], current: &[Message]) -> bool {
    if current.len() < base.len() {
        return false;
    }
    base.iter().zip(current.iter()).all(|(a, b)| a == b)
}

/// Upsert a message into `agent_messages` and return its id.
async fn upsert_message(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    message: &Message,
) -> anyhow::Result<Uuid> {
    let (id, hash) = upsert_row(message);
    let role = message.role();
    let payload = serde_json::to_value(message)
        .map_err(|e| anyhow::anyhow!("failed to serialize message: {}", e))?;
    let now = Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO agent_messages (id, payload_hash, role, payload, created_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (payload_hash) DO UPDATE
            SET payload_hash = EXCLUDED.payload_hash
        "#,
        id,
        &hash,
        role,
        &payload,
        now,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(id)
}

/// Insert refs into `agent_checkpoint_messages`.
async fn insert_message_refs(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    checkpoint_id: Uuid,
    refs: &[(i32, Uuid)],
) -> anyhow::Result<()> {
    for (position, message_id) in refs {
        sqlx::query!(
            r#"
            INSERT INTO agent_checkpoint_messages (checkpoint_id, position, message_id)
            VALUES ($1, $2, $3)
            "#,
            checkpoint_id,
            *position,
            *message_id,
        )
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Load the ordered message sequence for a checkpoint by following the
/// base chain.  Returns the concatenated `Vec<Message>`.
async fn load_message_sequence(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    checkpoint_id: Uuid,
) -> anyhow::Result<Vec<Message>> {
    let mut chain: Vec<(Uuid, Option<Uuid>)> = Vec::new();
    let mut current = Some(checkpoint_id);

    // Walk up the base chain.
    while let Some(cid) = current {
        let row = sqlx::query!(
            r#"
            SELECT id, base_checkpoint_id
            FROM agent_checkpoints
            WHERE id = $1
            "#,
            cid,
        )
        .fetch_optional(&mut **transaction)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                return Err(anyhow::anyhow!(
                    "checkpoint {} not found while loading message sequence",
                    cid
                ));
            }
        };

        chain.push((row.id, row.base_checkpoint_id));
        current = row.base_checkpoint_id;
    }

    // chain is now [target, parent, grandparent, ..., root]
    // We need to build from root down to target, collecting local refs at each step.
    chain.reverse();

    let mut messages: Vec<Message> = Vec::new();
    for (cid, _) in &chain {
        let rows = sqlx::query!(
            r#"
            SELECT acm.position, acm.message_id, am.payload AS "payload: serde_json::Value"
            FROM agent_checkpoint_messages acm
            JOIN agent_messages am ON am.id = acm.message_id
            WHERE acm.checkpoint_id = $1
            ORDER BY acm.position ASC
            "#,
            cid,
        )
        .fetch_all(&mut **transaction)
        .await?;

        // Position-based: we build a vec of (pos, payload) then sort and collect.
        let mut local: Vec<(i32, serde_json::Value)> =
            rows.into_iter().map(|r| (r.position, r.payload)).collect();
        local.sort_by_key(|(pos, _)| *pos);

        for (_, payload) in local {
            let msg: Message = serde_json::from_value(payload)
                .map_err(|e| anyhow::anyhow!("failed to decode stored message: {}", e))?;
            messages.push(msg);
        }
    }

    Ok(messages)
}

/// Load the full reconstructed checkpoint (entity + snapshot).
async fn reconstruct_checkpoint(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    checkpoint_id: Uuid,
) -> anyhow::Result<ReconstructedCheckpoint> {
    let row = sqlx::query!(
        r#"
        SELECT
            id,
            session_id,
            solution_id,
            kind,
            model,
            base_checkpoint_id,
            created_at
        FROM agent_checkpoints
        WHERE id = $1
        "#,
        checkpoint_id,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let mut entity = CheckpointEntity::from_db(
        row.id,
        row.session_id,
        row.solution_id,
        row.kind,
        row.model,
        row.base_checkpoint_id,
        row.created_at,
    )?;

    // Load local refs.
    let ref_rows = sqlx::query!(
        r#"
        SELECT position, message_id
        FROM agent_checkpoint_messages
        WHERE checkpoint_id = $1
        ORDER BY position ASC
        "#,
        checkpoint_id,
    )
    .fetch_all(&mut **transaction)
    .await?;

    entity.message_refs = ref_rows
        .into_iter()
        .map(|r| CheckpointMessageRef {
            position: r.position,
            message_id: r.message_id,
        })
        .collect();

    let messages = load_message_sequence(transaction, checkpoint_id).await?;
    let snapshot = ContextSnapshot {
        model: entity.model.clone(),
        messages,
    };

    Ok(ReconstructedCheckpoint { entity, snapshot })
}

// ── IStorage impl ────────────────────────────────────────────────────────────

#[async_trait]
impl IStorage for RdbStorage {
    async fn create_session(&self, input: NewSession) -> anyhow::Result<Session> {
        let session = SessionEntity::new(input);
        let status = session.status.db_value();
        let row = sqlx::query!(
            r#"
            INSERT INTO agent_sessions (
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            "#,
            session.id,
            session.name,
            session.model,
            status,
            session.forked_from_checkpoint_id,
            session.created_at,
            session.updated_at,
        )
        .fetch_one(&self.pool)
        .await?;

        SessionEntity::from_db(
            row.id,
            row.name,
            row.model,
            row.status,
            row.forked_from_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
        .map(SessionEntity::into_data_object)
    }

    async fn get_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            FROM agent_sessions
            WHERE id = $1
            "#,
            session_id,
        )
        .fetch_one(&self.pool)
        .await?;

        SessionEntity::from_db(
            row.id,
            row.name,
            row.model,
            row.status,
            row.forked_from_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
        .map(SessionEntity::into_data_object)
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            FROM agent_sessions
            WHERE status = 'active'
            ORDER BY updated_at DESC, created_at DESC, id DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                SessionEntity::from_db(
                    row.id,
                    row.name,
                    row.model,
                    row.status,
                    row.forked_from_checkpoint_id,
                    row.created_at,
                    row.updated_at,
                )
                .map(SessionEntity::into_data_object)
            })
            .collect()
    }

    async fn archive_session(&self, session_id: Uuid) -> anyhow::Result<Session> {
        let now = Utc::now();
        let status = SessionStatus::Archived.db_value();
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
                forked_from_checkpoint_id,
                created_at,
                updated_at
            "#,
            session_id,
            status,
            now,
        )
        .fetch_one(&self.pool)
        .await?;

        SessionEntity::from_db(
            row.id,
            row.name,
            row.model,
            row.status,
            row.forked_from_checkpoint_id,
            row.created_at,
            row.updated_at,
        )
        .map(SessionEntity::into_data_object)
    }

    async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint> {
        let mut transaction = self.pool.begin().await?;

        // Lock the session to prevent concurrent checkpoint creation.
        let _session = sqlx::query!(
            r#"
            SELECT id FROM agent_sessions
            WHERE id = $1
            FOR UPDATE
            "#,
            input.session_id,
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| anyhow::anyhow!("session {} not found", input.session_id))?;

        // Determine the base checkpoint.
        let base_id: Option<Uuid> = {
            let row = sqlx::query!(
                r#"
                SELECT id
                FROM agent_checkpoints
                WHERE session_id = $1
                ORDER BY created_at DESC, id DESC
                LIMIT 1
                "#,
                input.session_id,
            )
            .fetch_optional(&mut *transaction)
            .await?;

            row.map(|r| r.id)
        };

        // Build the base message sequence.
        let base_messages: Vec<Message> = if let Some(bid) = base_id {
            load_message_sequence(&mut transaction, bid).await?
        } else {
            Vec::new()
        };

        // Decide: incremental extension or reset.
        let (final_base_id, refs_to_store): (Option<Uuid>, Vec<(i32, Uuid)>) =
            if prefix_match(&base_messages, &input.messages) {
                // Incremental: only store the suffix.
                let suffix = &input.messages[base_messages.len()..];
                let mut refs = Vec::with_capacity(suffix.len());
                for (i, message) in suffix.iter().enumerate() {
                    let msg_id = upsert_message(&mut transaction, message).await?;
                    refs.push((base_messages.len() as i32 + i as i32, msg_id));
                }
                (base_id, refs)
            } else {
                // Reset: store all messages as locals on a root checkpoint.
                let mut refs = Vec::with_capacity(input.messages.len());
                for (i, message) in input.messages.iter().enumerate() {
                    let msg_id = upsert_message(&mut transaction, message).await?;
                    refs.push((i as i32, msg_id));
                }
                (None, refs)
            };

        // Insert the checkpoint row.
        let checkpoint_entity = {
            let mut entity = CheckpointEntity::new(input);
            entity.base_checkpoint_id = final_base_id;
            entity
        };
        let kind = checkpoint_entity.kind.db_value();
        sqlx::query!(
            r#"
            INSERT INTO agent_checkpoints (
                id,
                session_id,
                solution_id,
                kind,
                model,
                base_checkpoint_id,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            checkpoint_entity.id,
            checkpoint_entity.session_id,
            checkpoint_entity.solution_id,
            kind,
            checkpoint_entity.model,
            checkpoint_entity.base_checkpoint_id,
            checkpoint_entity.created_at,
        )
        .execute(&mut *transaction)
        .await?;

        // Insert message refs.
        insert_message_refs(&mut transaction, checkpoint_entity.id, &refs_to_store).await?;

        transaction.commit().await?;

        Ok(checkpoint_entity.into_data_object())
    }

    async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                solution_id,
                kind,
                model,
                base_checkpoint_id,
                created_at
            FROM agent_checkpoints
            WHERE id = $1
            "#,
            checkpoint_id,
        )
        .fetch_one(&self.pool)
        .await?;

        CheckpointEntity::from_db(
            row.id,
            row.session_id,
            row.solution_id,
            row.kind,
            row.model,
            row.base_checkpoint_id,
            row.created_at,
        )
        .map(CheckpointEntity::into_data_object)
    }

    async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                solution_id,
                kind,
                model,
                base_checkpoint_id,
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
                CheckpointEntity::from_db(
                    row.id,
                    row.session_id,
                    row.solution_id,
                    row.kind,
                    row.model,
                    row.base_checkpoint_id,
                    row.created_at,
                )
                .map(CheckpointEntity::into_data_object)
            })
            .collect()
    }

    async fn load_checkpoint_context(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<CheckpointContext> {
        let mut transaction = self.pool.begin().await?;
        let reconstructed = reconstruct_checkpoint(&mut transaction, checkpoint_id).await?;
        transaction.commit().await?;

        Ok(CheckpointContext {
            checkpoint: reconstructed.entity.into_data_object(),
            snapshot: reconstructed.snapshot,
        })
    }

    async fn checkpoint_local_ref_count(&self, checkpoint_id: Uuid) -> anyhow::Result<i64> {
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM agent_checkpoint_messages
            WHERE checkpoint_id = $1
            "#,
            checkpoint_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.count)
    }

    async fn persist_diagnostics(&self) -> anyhow::Result<PersistDiagnostics> {
        let row = sqlx::query!(
            r#"
            SELECT
                (SELECT COUNT(*) FROM agent_sessions WHERE status = 'active') AS "session_count!",
                (SELECT COUNT(*) FROM agent_checkpoints) AS "checkpoint_count!",
                (SELECT COUNT(*) FROM agent_messages) AS "message_count!",
                (SELECT COUNT(*) FROM agent_checkpoint_messages) AS "checkpoint_local_ref_count!"
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(PersistDiagnostics {
            session_count: row.session_count,
            checkpoint_count: row.checkpoint_count,
            message_count: row.message_count,
            checkpoint_local_ref_count: row.checkpoint_local_ref_count,
        })
    }

    async fn fork_session_from_checkpoint(
        &self,
        parent_checkpoint_id: Uuid,
        name: Option<String>,
    ) -> anyhow::Result<(Session, Checkpoint)> {
        let mut transaction = self.pool.begin().await?;

        // Read parent checkpoint to get the model.
        let parent_row = sqlx::query!(
            r#"
            SELECT id, session_id, solution_id, kind, model, base_checkpoint_id, created_at
            FROM agent_checkpoints
            WHERE id = $1
            "#,
            parent_checkpoint_id,
        )
        .fetch_one(&mut *transaction)
        .await?;

        let parent_model = parent_row.model.clone();

        // Create fork session.
        let session = SessionEntity::new(NewSession {
            name,
            model: parent_model.clone(),
            forked_from_checkpoint_id: Some(parent_checkpoint_id),
        });
        let status = session.status.db_value();
        let session_row = sqlx::query!(
            r#"
            INSERT INTO agent_sessions (
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                name,
                model,
                status,
                forked_from_checkpoint_id,
                created_at,
                updated_at
            "#,
            session.id,
            session.name,
            session.model,
            status,
            session.forked_from_checkpoint_id,
            session.created_at,
            session.updated_at,
        )
        .fetch_one(&mut *transaction)
        .await?;
        let session = SessionEntity::from_db(
            session_row.id,
            session_row.name,
            session_row.model,
            session_row.status,
            session_row.forked_from_checkpoint_id,
            session_row.created_at,
            session_row.updated_at,
        )?
        .into_data_object();

        // Create fork checkpoint — base_checkpoint_id = parent, zero local refs.
        let checkpoint_entity = {
            let mut entity = CheckpointEntity::new(NewCheckpoint {
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::Fork,
                model: parent_model,
                messages: Vec::new(),
            });
            entity.base_checkpoint_id = Some(parent_checkpoint_id);
            entity
        };
        let kind = checkpoint_entity.kind.db_value();
        sqlx::query!(
            r#"
            INSERT INTO agent_checkpoints (
                id,
                session_id,
                solution_id,
                kind,
                model,
                base_checkpoint_id,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            checkpoint_entity.id,
            checkpoint_entity.session_id,
            checkpoint_entity.solution_id,
            kind,
            checkpoint_entity.model,
            checkpoint_entity.base_checkpoint_id,
            checkpoint_entity.created_at,
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok((session, checkpoint_entity.into_data_object()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::session::persist::data_object::{Status, ToolCall};

    fn test_snapshot(_model: &str) -> Vec<Message> {
        vec![
            Message::System {
                content: "system".to_string(),
            },
            Message::User {
                content: "hello".to_string(),
            },
            Message::Assistant {
                content: Some("calling".to_string()),
                refusal: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "lookup".to_string(),
                    args: "{\"q\":\"poprako\"}".to_string(),
                }]),
            },
            Message::Tool {
                tool_call_id: "call_1".to_string(),
                content: "result".to_string(),
            },
        ]
    }

    fn snapshot2(model: &str) -> Vec<Message> {
        let mut msgs = test_snapshot(model);
        msgs.push(Message::User {
            content: "follow-up".to_string(),
        });
        msgs.push(Message::Assistant {
            content: Some("response".to_string()),
            refusal: None,
            tool_calls: None,
        });
        msgs
    }

    async fn storage() -> RdbStorage {
        dotenvy::dotenv().ok();
        RdbStorage::from_env()
            .await
            .expect("storage should connect")
    }

    async fn cleanup(storage: &RdbStorage, prefix: &str) {
        let pattern = format!("{}%", prefix);
        sqlx::query!(
            r#"
            UPDATE agent_sessions
            SET forked_from_checkpoint_id = NULL
            WHERE name LIKE $1
            "#,
            pattern,
        )
        .execute(storage.pool())
        .await
        .expect("should clear test fork references");

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
                forked_from_checkpoint_id: None,
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
    async fn list_sessions_returns_active_sessions_by_updated_at_desc() {
        let storage = storage().await;
        let prefix = format!("test-list-session-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let older = storage
            .create_session(NewSession {
                name: Some(format!("{}older", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();
        let newer = storage
            .create_session(NewSession {
                name: Some(format!("{}newer", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();
        let archived = storage
            .create_session(NewSession {
                name: Some(format!("{}archived", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();
        storage.archive_session(archived.id).await.unwrap();

        sqlx::query!(
            r#"
            UPDATE agent_sessions
            SET updated_at = NOW() + INTERVAL '1 hour'
            WHERE id = $1
            "#,
            older.id,
        )
        .execute(storage.pool())
        .await
        .unwrap();

        let sessions = storage.list_sessions().await.unwrap();
        let listed: Vec<Uuid> = sessions
            .iter()
            .filter(|session| {
                session
                    .name
                    .as_deref()
                    .map(|name| name.starts_with(&prefix))
                    .unwrap_or(false)
            })
            .map(|session| session.id)
            .collect();

        assert_eq!(listed, vec![older.id, newer.id]);

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
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();
        let solution_id = Uuid::new_v4();
        let before = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: Some(solution_id),
                kind: CheckpointKind::BeforeSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: test_snapshot("deepseek-v4-flash"),
            })
            .await
            .unwrap();
        let after = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: Some(solution_id),
                kind: CheckpointKind::AfterSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: snapshot2("deepseek-v4-flash"),
            })
            .await
            .unwrap();

        let loaded = storage.get_checkpoint(after.id).await.unwrap();
        let checkpoints = storage.list_checkpoints(session.id).await.unwrap();

        assert_eq!(loaded.id, after.id);
        assert_eq!(loaded.model, "deepseek-v4-flash");
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].id, before.id);
        assert_eq!(checkpoints[1].id, after.id);
        assert_eq!(checkpoints[0].solution_id, Some(solution_id));
        assert_eq!(checkpoints[1].solution_id, Some(solution_id));

        // Verify incremental: after has base_checkpoint_id pointing to before.
        assert_eq!(after.base_checkpoint_id, Some(before.id));

        // Load context for after — should have all messages.
        let ctx = storage.load_checkpoint_context(after.id).await.unwrap();
        assert_eq!(ctx.snapshot.messages.len(), 6);

        // Verify no duplicate messages inserted: count unique message ids
        // referenced by this session's checkpoints.
        let msg_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT acm.message_id)::int4 AS "count!"
            FROM agent_checkpoint_messages acm
            JOIN agent_checkpoints ac ON ac.id = acm.checkpoint_id
            WHERE ac.session_id = $1
            "#,
            session.id,
        )
        .fetch_one(storage.pool())
        .await
        .unwrap();
        assert_eq!(msg_count, 6);

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn fork_does_not_duplicate_messages() {
        let storage = storage().await;
        let prefix = format!("test-fork-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();
        let parent_msgs = test_snapshot("deepseek-v4-flash");
        let parent_checkpoint = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: Some(Uuid::new_v4()),
                kind: CheckpointKind::AfterSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: parent_msgs.clone(),
            })
            .await
            .unwrap();

        let (fork, fork_checkpoint) = storage
            .fork_session_from_checkpoint(parent_checkpoint.id, Some(format!("{}fork", prefix)))
            .await
            .unwrap();
        let loaded_fork = storage.get_session(fork.id).await.unwrap();
        let loaded_checkpoint = storage.get_checkpoint(fork_checkpoint.id).await.unwrap();

        assert_eq!(
            loaded_fork.forked_from_checkpoint_id,
            Some(parent_checkpoint.id)
        );
        assert_eq!(loaded_fork.model, "deepseek-v4-flash");
        assert_eq!(loaded_checkpoint.kind, CheckpointKind::Fork);
        assert_eq!(loaded_checkpoint.solution_id, None);
        assert_eq!(
            loaded_checkpoint.base_checkpoint_id,
            Some(parent_checkpoint.id)
        );

        // Fork checkpoint should reconstruct the same context as parent.
        let fork_ctx = storage
            .load_checkpoint_context(fork_checkpoint.id)
            .await
            .unwrap();
        assert_eq!(fork_ctx.snapshot.messages, parent_msgs);

        // Verify fork did not insert any new messages — all message refs
        // should come from the parent session's checkpoints.
        let fork_msg_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT acm.message_id)::int4 AS "count!"
            FROM agent_checkpoint_messages acm
            JOIN agent_checkpoints ac ON ac.id = acm.checkpoint_id
            WHERE ac.session_id = $1
            "#,
            fork.id,
        )
        .fetch_one(storage.pool())
        .await
        .unwrap();
        assert_eq!(
            fork_msg_count, 0,
            "fork should have zero local message refs"
        );

        // Parent session still has the original 4 unique messages.
        let parent_msg_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT acm.message_id)::int4 AS "count!"
            FROM agent_checkpoint_messages acm
            JOIN agent_checkpoints ac ON ac.id = acm.checkpoint_id
            WHERE ac.session_id = $1
            "#,
            session.id,
        )
        .fetch_one(storage.pool())
        .await
        .unwrap();
        assert_eq!(parent_msg_count, 4);

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn reset_checkpoint_on_non_prefix_context() {
        let storage = storage().await;
        let prefix = format!("test-reset-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();

        let _first = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::BeforeSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: test_snapshot("deepseek-v4-flash"),
            })
            .await
            .unwrap();

        // Create a context that is NOT a prefix extension
        // (different tool call content).
        let changed = vec![
            Message::System {
                content: "system-new".to_string(),
            },
            Message::User {
                content: "hello".to_string(),
            },
        ];
        let second = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::AfterSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: changed.clone(),
            })
            .await
            .unwrap();

        // Reset checkpoint should have base_checkpoint_id = NULL.
        assert_eq!(second.base_checkpoint_id, None);

        // Load context — should be the new messages only.
        let ctx = storage.load_checkpoint_context(second.id).await.unwrap();
        assert_eq!(ctx.snapshot.messages, changed);

        cleanup(&storage, &prefix).await;
    }

    #[tokio::test]
    async fn checkpoint_requires_existing_session() {
        let storage = storage().await;
        let result = storage
            .create_checkpoint(NewCheckpoint {
                session_id: Uuid::new_v4(),
                solution_id: None,
                kind: CheckpointKind::BeforeSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: test_snapshot("deepseek-v4-flash"),
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn incremental_checkpoint_only_stores_suffix_refs() {
        let storage = storage().await;
        let prefix = format!("test-incr-{}-", Uuid::new_v4());
        cleanup(&storage, &prefix).await;

        let session = storage
            .create_session(NewSession {
                name: Some(format!("{}root", prefix)),
                model: "deepseek-v4-flash".to_string(),
                forked_from_checkpoint_id: None,
            })
            .await
            .unwrap();

        let first_msgs = test_snapshot("deepseek-v4-flash");
        let first = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::BeforeSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: first_msgs.clone(),
            })
            .await
            .unwrap();

        let second_msgs = snapshot2("deepseek-v4-flash");
        let second = storage
            .create_checkpoint(NewCheckpoint {
                session_id: session.id,
                solution_id: None,
                kind: CheckpointKind::AfterSolution,
                model: "deepseek-v4-flash".to_string(),
                messages: second_msgs.clone(),
            })
            .await
            .unwrap();

        // Second checkpoint should extend the first.
        assert_eq!(second.base_checkpoint_id, Some(first.id));

        // Load context — both should have the right messages.
        let first_ctx = storage.load_checkpoint_context(first.id).await.unwrap();
        let second_ctx = storage.load_checkpoint_context(second.id).await.unwrap();
        assert_eq!(first_ctx.snapshot.messages, first_msgs);
        assert_eq!(second_ctx.snapshot.messages, second_msgs);

        // Only suffix refs in second checkpoint's local refs.
        let ref_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::int4 AS "count!" FROM agent_checkpoint_messages WHERE checkpoint_id = $1"#,
            second.id,
        )
        .fetch_one(storage.pool())
        .await
        .unwrap();
        assert_eq!(ref_count, 2); // only the 2 new messages
        assert_eq!(
            storage.checkpoint_local_ref_count(second.id).await.unwrap(),
            2
        );

        cleanup(&storage, &prefix).await;
    }
}
