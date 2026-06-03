use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: Option<String>,
    pub model: String,
    pub status: Status,
    pub parent_session_id: Option<Uuid>,
    pub parent_checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub kind: CheckpointKind,
    pub snapshot: ContextSnapshot,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    BeforeRun,
    AfterRun,
    Fork,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub model: String,
    pub messages: Vec<Message>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub name: Option<String>,
    pub model: String,
    pub parent_session_id: Option<Uuid>,
    pub parent_checkpoint_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NewCheckpoint {
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub kind: CheckpointKind,
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
