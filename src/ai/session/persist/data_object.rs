use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Session ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: Option<String>,
    pub model: String,
    pub status: Status,
    pub forked_from_checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistDiagnostics {
    pub session_count: i64,
    pub checkpoint_count: i64,
    pub message_count: i64,
    pub checkpoint_local_ref_count: i64,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub name: Option<String>,
    pub model: String,
    pub forked_from_checkpoint_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

/// Codec-only message representation (the JSON form that is hashed and stored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        refusal: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl Message {
    pub fn role(&self) -> &'static str {
        match self {
            Message::System { .. } => "system",
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
            Message::Tool { .. } => "tool",
        }
    }
}

// ── Stored message (content atom) ────────────────────────────────────────────

/// An immutable message content atom stored exactly once, keyed by a
/// SHA-256 hash of its canonical JSON payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: Uuid,
    pub payload_hash: Vec<u8>,
    pub role: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Compute the SHA-256 hash of a canonical JSON payload.
/// Used for content-addressed deduplication of messages.
pub fn hash_message(message: &Message) -> Vec<u8> {
    use sha2::{Digest as _, Sha256};

    let canonical = serde_json::to_vec(message).expect("message serialization should not fail");
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    hasher.finalize().to_vec()
}

// ── Checkpoint metadata ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    BeforeSolution,
    AfterSolution,
    Fork,
}

/// Lightweight checkpoint record.  Context reconstruction happens through
/// `agent_checkpoint_messages` and the base chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub session_id: Uuid,
    pub solution_id: Option<Uuid>,
    pub kind: CheckpointKind,
    pub model: String,
    pub base_checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointMessageRef {
    pub position: i32,
    pub message_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct NewCheckpoint {
    pub session_id: Uuid,
    pub solution_id: Option<Uuid>,
    pub kind: CheckpointKind,
    pub model: String,
    /// The encoded messages that make up the current agent context.
    /// Storage will decide whether to create an incremental or reset checkpoint.
    pub messages: Vec<Message>,
}

// ── Context snapshot (codec boundary) ────────────────────────────────────────

/// Full materialised context returned when loading a checkpoint.
/// Kept as a codec boundary type — it is not stored as a single entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub model: String,
    pub messages: Vec<Message>,
}

// ── Checkpoint context (storage return type) ─────────────────────────────────

/// The result of loading a checkpoint: metadata + reconstructed snapshot.
#[derive(Debug, Clone)]
pub struct CheckpointContext {
    pub checkpoint: Checkpoint,
    pub snapshot: ContextSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serializes_with_role_tag() {
        let message = Message::Assistant {
            content: Some("calling".to_string()),
            refusal: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                name: "lookup".to_string(),
                args: "{}".to_string(),
            }]),
        };

        let json = serde_json::to_value(&message).unwrap();

        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "calling");
        assert_eq!(json["tool_calls"][0]["id"], "call_1");
        assert_eq!(json["tool_calls"][0]["name"], "lookup");
        assert_eq!(json["tool_calls"][0]["args"], "{}");
    }

    #[test]
    fn message_role_maps_correctly() {
        assert_eq!(
            (Message::System {
                content: "s".into()
            })
            .role(),
            "system"
        );
        assert_eq!(
            (Message::User {
                content: "u".into()
            })
            .role(),
            "user"
        );
        assert_eq!(
            (Message::Assistant {
                content: Some("a".into()),
                refusal: None,
                tool_calls: None,
            })
            .role(),
            "assistant"
        );
        assert_eq!(
            (Message::Tool {
                tool_call_id: "id".into(),
                content: "t".into()
            })
            .role(),
            "tool"
        );
    }

    #[test]
    fn context_snapshot_round_trips_json() {
        let snapshot = ContextSnapshot {
            model: "deepseek-v4-flash".to_string(),
            messages: vec![
                Message::System {
                    content: "system".to_string(),
                },
                Message::User {
                    content: "hello".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: ContextSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, snapshot);
    }
}
