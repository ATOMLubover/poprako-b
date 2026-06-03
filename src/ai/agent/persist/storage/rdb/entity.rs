use chrono::DateTime;
use chrono::Utc;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ai::agent::persist::data_object;
use crate::ai::agent::persist::data_object::ContextSnapshot;
use crate::ai::agent::persist::data_object::Message;
use crate::ai::agent::persist::data_object::NewCheckpoint;
use crate::ai::agent::persist::data_object::NewSession;

pub(super) struct Session {
    pub(super) id: Uuid,
    pub(super) name: Option<String>,
    pub(super) model: String,
    pub(super) status: SessionStatus,
    pub(super) parent_session_id: Option<Uuid>,
    pub(super) parent_checkpoint_id: Option<Uuid>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

pub(super) enum SessionStatus {
    Active,
    Archived,
}

impl SessionStatus {
    pub(super) fn db_value(&self) -> &'static str {
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

impl Session {
    pub(super) fn new(input: NewSession) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: input.name,
            model: input.model,
            status: SessionStatus::Active,
            parent_session_id: input.parent_session_id,
            parent_checkpoint_id: input.parent_checkpoint_id,
            created_at: now,
            updated_at: now,
        }
    }

    pub(super) fn from_db(
        id: Uuid,
        name: Option<String>,
        model: String,
        status: String,
        parent_session_id: Option<Uuid>,
        parent_checkpoint_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            name,
            model,
            status: SessionStatus::from_db(status.as_str())?,
            parent_session_id,
            parent_checkpoint_id,
            created_at,
            updated_at,
        })
    }

    pub(super) fn into_data_object(self) -> data_object::Session {
        data_object::Session {
            id: self.id,
            name: self.name,
            model: self.model,
            status: self.status.to_data_object(),
            parent_session_id: self.parent_session_id,
            parent_checkpoint_id: self.parent_checkpoint_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub(super) struct Checkpoint {
    pub(super) id: Uuid,
    pub(super) session_id: Uuid,
    pub(super) run_id: Option<Uuid>,
    pub(super) kind: CheckpointKind,
    pub(super) snapshot: ContextSnapshot,
    pub(super) created_at: DateTime<Utc>,
}

pub(super) enum CheckpointKind {
    BeforeRun,
    AfterRun,
    Fork,
}

impl CheckpointKind {
    pub(super) fn db_value(&self) -> &'static str {
        match self {
            Self::BeforeRun => "before_run",
            Self::AfterRun => "after_run",
            Self::Fork => "fork",
        }
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "before_run" => Ok(Self::BeforeRun),
            "after_run" => Ok(Self::AfterRun),
            "fork" => Ok(Self::Fork),
            other => Err(anyhow::anyhow!("unknown checkpoint kind: {}", other)),
        }
    }

    fn from_data_object(kind: data_object::CheckpointKind) -> Self {
        match kind {
            data_object::CheckpointKind::BeforeRun => Self::BeforeRun,
            data_object::CheckpointKind::AfterRun => Self::AfterRun,
            data_object::CheckpointKind::Fork => Self::Fork,
        }
    }

    fn to_data_object(&self) -> data_object::CheckpointKind {
        match self {
            Self::BeforeRun => data_object::CheckpointKind::BeforeRun,
            Self::AfterRun => data_object::CheckpointKind::AfterRun,
            Self::Fork => data_object::CheckpointKind::Fork,
        }
    }
}

impl Checkpoint {
    pub(super) fn new(input: NewCheckpoint) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            run_id: input.run_id,
            kind: CheckpointKind::from_data_object(input.kind),
            snapshot: input.snapshot,
            created_at: Utc::now(),
        }
    }

    pub(super) fn from_db(
        id: Uuid,
        session_id: Uuid,
        run_id: Option<Uuid>,
        kind: String,
        model: String,
        messages: Json<Vec<Message>>,
        created_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            session_id,
            run_id,
            kind: CheckpointKind::from_db(kind.as_str())?,
            snapshot: ContextSnapshot {
                model,
                messages: messages.0,
            },
            created_at,
        })
    }

    pub(super) fn into_data_object(self) -> data_object::Checkpoint {
        data_object::Checkpoint {
            id: self.id,
            session_id: self.session_id,
            run_id: self.run_id,
            kind: self.kind.to_data_object(),
            snapshot: self.snapshot,
            created_at: self.created_at,
        }
    }
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
        assert_eq!(CheckpointKind::BeforeRun.db_value(), "before_run");
        assert_eq!(CheckpointKind::AfterRun.db_value(), "after_run");
        assert_eq!(CheckpointKind::Fork.db_value(), "fork");
        assert!(matches!(
            CheckpointKind::from_db("before_run").unwrap(),
            CheckpointKind::BeforeRun
        ));
        assert!(matches!(
            CheckpointKind::from_db("after_run").unwrap(),
            CheckpointKind::AfterRun
        ));
        assert!(matches!(
            CheckpointKind::from_db("fork").unwrap(),
            CheckpointKind::Fork
        ));
        assert!(CheckpointKind::from_db("snapshot").is_err());
    }
}
