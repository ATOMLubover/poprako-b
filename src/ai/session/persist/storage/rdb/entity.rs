use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::ai::session::persist::data_object::{
    self, CheckpointKind, ContextSnapshot, Message, NewCheckpoint, NewSession, hash_message,
};

// ── Session ──────────────────────────────────────────────────────────────────

pub enum SessionStatus {
    Active,
    Archived,
}

impl SessionStatus {
    pub fn db_value(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            other => Err(anyhow::anyhow!("unknown session status: {}", other)),
        }
    }

    fn to_data_object(&self) -> data_object::Status {
        match self {
            Self::Active => data_object::Status::Active,
            Self::Archived => data_object::Status::Archived,
        }
    }
}

pub struct SessionEntity {
    pub id: Uuid,
    pub name: Option<String>,
    pub model: String,
    pub status: SessionStatus,
    pub forked_from_checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SessionEntity {
    pub fn new(input: NewSession) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: input.name,
            model: input.model,
            status: SessionStatus::Active,
            forked_from_checkpoint_id: input.forked_from_checkpoint_id,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn from_db(
        id: Uuid,
        name: Option<String>,
        model: String,
        status: String,
        forked_from_checkpoint_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            name,
            model,
            status: SessionStatus::from_db(status.as_str())?,
            forked_from_checkpoint_id,
            created_at,
            updated_at,
        })
    }

    pub fn into_data_object(self) -> data_object::Session {
        data_object::Session {
            id: self.id,
            name: self.name,
            model: self.model,
            status: self.status.to_data_object(),
            forked_from_checkpoint_id: self.forked_from_checkpoint_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

// ── Stored message ───────────────────────────────────────────────────────────

pub fn upsert_row(message: &Message) -> (Uuid, Vec<u8>) {
    let hash = hash_message(message);
    // Derive a deterministic UUID from the hash so that we always get the
    // same id for the same payload — this plays nicely with `ON CONFLICT`.
    let id = Uuid::from_slice(&hash[..16]).expect("hash-derived UUID should be valid");
    (id, hash)
}

// ── Checkpoint ───────────────────────────────────────────────────────────────

pub enum CheckpointKindValue {
    BeforeSolution,
    AfterSolution,
    Fork,
}

impl CheckpointKindValue {
    pub fn db_value(&self) -> &'static str {
        match self {
            Self::BeforeSolution => "before_solution",
            Self::AfterSolution => "after_solution",
            Self::Fork => "fork",
        }
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "before_solution" => Ok(Self::BeforeSolution),
            "after_solution" => Ok(Self::AfterSolution),
            "fork" => Ok(Self::Fork),
            other => Err(anyhow::anyhow!("unknown checkpoint kind: {}", other)),
        }
    }

    fn from_data_object(kind: CheckpointKind) -> Self {
        match kind {
            CheckpointKind::BeforeSolution => Self::BeforeSolution,
            CheckpointKind::AfterSolution => Self::AfterSolution,
            CheckpointKind::Fork => Self::Fork,
        }
    }

    fn to_data_object(&self) -> CheckpointKind {
        match self {
            Self::BeforeSolution => CheckpointKind::BeforeSolution,
            Self::AfterSolution => CheckpointKind::AfterSolution,
            Self::Fork => CheckpointKind::Fork,
        }
    }
}

pub struct CheckpointEntity {
    pub id: Uuid,
    pub session_id: Uuid,
    pub solution_id: Option<Uuid>,
    pub kind: CheckpointKindValue,
    pub model: String,
    pub base_checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// Local message refs (only the suffix beyond the base).
    pub message_refs: Vec<data_object::CheckpointMessageRef>,
}

impl CheckpointEntity {
    pub fn new(input: NewCheckpoint) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            solution_id: input.solution_id,
            kind: CheckpointKindValue::from_data_object(input.kind),
            model: input.model,
            base_checkpoint_id: None,
            created_at: Utc::now(),
            message_refs: Vec::new(),
        }
    }

    pub fn from_db(
        id: Uuid,
        session_id: Uuid,
        solution_id: Option<Uuid>,
        kind: String,
        model: String,
        base_checkpoint_id: Option<Uuid>,
        created_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            session_id,
            solution_id,
            kind: CheckpointKindValue::from_db(kind.as_str())?,
            model,
            base_checkpoint_id,
            created_at,
            message_refs: Vec::new(),
        })
    }

    pub fn into_data_object(self) -> data_object::Checkpoint {
        data_object::Checkpoint {
            id: self.id,
            session_id: self.session_id,
            solution_id: self.solution_id,
            kind: self.kind.to_data_object(),
            model: self.model,
            base_checkpoint_id: self.base_checkpoint_id,
            created_at: self.created_at,
        }
    }
}

// ── Context reconstruction helpers ───────────────────────────────────────────

pub struct ReconstructedCheckpoint {
    pub entity: CheckpointEntity,
    pub snapshot: ContextSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_maps_database_values() {
        assert_eq!(SessionStatus::Active.db_value(), "active");
        assert_eq!(SessionStatus::Archived.db_value(), "archived");
        assert!(matches!(
            SessionStatus::from_db("active").unwrap(),
            SessionStatus::Active
        ));
        assert!(matches!(
            SessionStatus::from_db("archived").unwrap(),
            SessionStatus::Archived
        ));
        assert!(SessionStatus::from_db("deleted").is_err());
    }

    #[test]
    fn checkpoint_kind_maps_database_values() {
        assert_eq!(
            CheckpointKindValue::BeforeSolution.db_value(),
            "before_solution"
        );
        assert_eq!(
            CheckpointKindValue::AfterSolution.db_value(),
            "after_solution"
        );
        assert_eq!(CheckpointKindValue::Fork.db_value(), "fork");
        assert!(matches!(
            CheckpointKindValue::from_db("before_solution").unwrap(),
            CheckpointKindValue::BeforeSolution
        ));
        assert!(matches!(
            CheckpointKindValue::from_db("after_solution").unwrap(),
            CheckpointKindValue::AfterSolution
        ));
        assert!(matches!(
            CheckpointKindValue::from_db("fork").unwrap(),
            CheckpointKindValue::Fork
        ));
        assert!(CheckpointKindValue::from_db("snapshot").is_err());
    }

    #[test]
    fn stored_message_id_is_deterministic() {
        let msg = Message::System {
            content: "test".to_string(),
        };
        let (id1, hash1) = upsert_row(&msg);
        let (id2, hash2) = upsert_row(&msg);
        assert_eq!(id1, id2);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn stored_message_id_differs_for_different_content() {
        let msg1 = Message::User {
            content: "hello".to_string(),
        };
        let msg2 = Message::User {
            content: "world".to_string(),
        };
        let (id1, _) = upsert_row(&msg1);
        let (id2, _) = upsert_row(&msg2);
        assert_ne!(id1, id2);
    }
}
