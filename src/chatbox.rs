use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context as _;
use axum::{Json, Router};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::ai::agent_impl::openai::{OpenAiAgent, OpenAiAgentBuilder};
use crate::ai::resolver_impl::openai::OpenAiResolver;
use crate::ai::session::SessionManager;
use crate::ai::session::persist::codec::{IContextSnapshotCodec, OpenAiCodec};
use crate::ai::session::persist::data_object::{
    Checkpoint, CheckpointKind, ContextSnapshot, Message, NewCheckpoint, Session,
};
use crate::ai::session::persist::storage::IStorage;
use crate::ai::session::persist::storage::rdb::RdbStorage;

type Manager = SessionManager<RdbStorage, ChatCompletionMessageParam, OpenAiCodec>;
#[derive(Clone)]
struct AppState {
    manager: Arc<Manager>,
    snapshots: Arc<Mutex<HashMap<Uuid, ContextSnapshot>>>,
    model: String,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
    details: String,
}

impl ApiError {
    fn internal(details: impl ToString) -> Self {
        Self {
            error: "internal_error".to_string(),
            details: details.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ForkRequest {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CheckoutRequest {
    checkpoint_id: Uuid,
}

#[derive(Debug, Serialize)]
struct CheckpointSummary {
    checkpoint: Checkpoint,
    message_count: usize,
    local_ref_count: i64,
}

#[derive(Debug, Serialize)]
struct SessionSummary {
    session: Session,
    latest_checkpoint: Option<CheckpointSummary>,
}

#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    session: Session,
    checkpoint: CheckpointSummary,
}

#[derive(Debug, Serialize)]
struct MessageResponse {
    assistant: String,
    before_checkpoint: CheckpointSummary,
    after_checkpoint: CheckpointSummary,
}

#[derive(Debug, Serialize)]
struct CheckpointListResponse {
    checkpoints: Vec<CheckpointSummary>,
}

#[derive(Debug, Serialize)]
struct CheckpointContextResponse {
    checkpoint: Checkpoint,
    messages: Vec<Message>,
    message_count: usize,
    local_ref_count: i64,
}

#[derive(Debug, Serialize)]
struct ForkResponse {
    session: Session,
    checkpoint: CheckpointSummary,
}

#[derive(Debug, Serialize)]
struct CheckoutResponse {
    session_id: Uuid,
    checkpoint: CheckpointSummary,
}

#[derive(Debug, Serialize)]
struct PersistDebugResponse {
    session_count: i64,
    checkpoint_count: i64,
    message_count: i64,
    checkpoint_local_ref_count: i64,
}

fn initial_snapshot(model: &str) -> ContextSnapshot {
    ContextSnapshot {
        model: model.to_string(),
        messages: vec![Message::System {
            content: "You are a concise assistant in a local checkpoint persistence demo."
                .to_string(),
        }],
    }
}

fn build_agent(snapshot: ContextSnapshot) -> anyhow::Result<OpenAiAgent> {
    let context = OpenAiCodec.decode_context(&snapshot)?;
    Ok(OpenAiAgentBuilder::new(context, OpenAiResolver::from_env()).build())
}

async fn checkpoint_summary(
    state: &AppState,
    checkpoint: Checkpoint,
) -> Result<CheckpointSummary, ApiError> {
    let context = state
        .manager
        .load_checkpoint_context(checkpoint.id)
        .await
        .map_err(ApiError::internal)?;
    let local_ref_count = state
        .manager
        .store()
        .checkpoint_local_ref_count(checkpoint.id)
        .await
        .map_err(ApiError::internal)?;

    Ok(CheckpointSummary {
        checkpoint,
        message_count: context.snapshot.messages.len(),
        local_ref_count,
    })
}

async fn latest_checkpoint_summary(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<CheckpointSummary>, ApiError> {
    let checkpoints = state
        .manager
        .list_checkpoints(session_id)
        .await
        .map_err(ApiError::internal)?;
    match checkpoints.last().cloned() {
        Some(checkpoint) => Ok(Some(checkpoint_summary(state, checkpoint).await?)),
        None => Ok(None),
    }
}

async fn persist_raw_checkpoint(
    state: &AppState,
    session_id: Uuid,
    solution_id: Option<Uuid>,
    kind: CheckpointKind,
    snapshot: ContextSnapshot,
) -> Result<CheckpointSummary, ApiError> {
    let checkpoint = state
        .manager
        .store()
        .create_checkpoint(NewCheckpoint {
            session_id,
            solution_id,
            kind,
            model: snapshot.model,
            messages: snapshot.messages,
        })
        .await
        .map_err(ApiError::internal)?;
    checkpoint_summary(state, checkpoint).await
}

async fn snapshot_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<ContextSnapshot, ApiError> {
    if let Some(snapshot) = state.snapshots.lock().await.get(&session_id).cloned() {
        return Ok(snapshot);
    }

    let session = state
        .manager
        .store()
        .get_session(session_id)
        .await
        .map_err(ApiError::internal)?;
    let snapshot = match state
        .manager
        .list_checkpoints(session.id)
        .await
        .map_err(ApiError::internal)?
        .last()
        .cloned()
    {
        Some(checkpoint) => {
            let context = state
                .manager
                .load_checkpoint_context(checkpoint.id)
                .await
                .map_err(ApiError::internal)?;
            context.snapshot
        }
        None => initial_snapshot(session.model.as_str()),
    };

    state
        .snapshots
        .lock()
        .await
        .insert(session_id, snapshot.clone());
    Ok(snapshot)
}

async fn list_sessions(State(state): State<AppState>) -> ApiResult<Vec<SessionSummary>> {
    let sessions = state
        .manager
        .store()
        .list_sessions()
        .await
        .map_err(ApiError::internal)?;
    let mut summaries = Vec::with_capacity(sessions.len());

    for session in sessions {
        let latest_checkpoint = latest_checkpoint_summary(&state, session.id).await?;
        summaries.push(SessionSummary {
            session,
            latest_checkpoint,
        });
    }

    Ok(Json(summaries))
}

async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSessionRequest>,
) -> ApiResult<CreateSessionResponse> {
    let session = state
        .manager
        .create_session(state.model.clone(), input.name)
        .await
        .map_err(ApiError::internal)?;
    let snapshot = initial_snapshot(state.model.as_str());
    let checkpoint = persist_raw_checkpoint(
        &state,
        session.id,
        None,
        CheckpointKind::BeforeSolution,
        snapshot.clone(),
    )
    .await?;

    state.snapshots.lock().await.insert(session.id, snapshot);

    Ok(Json(CreateSessionResponse {
        session,
        checkpoint,
    }))
}

async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<SendMessageRequest>,
) -> ApiResult<MessageResponse> {
    let snapshot = snapshot_for_session(&state, session_id).await?;
    let mut agent = build_agent(snapshot).map_err(ApiError::internal)?;

    let user_message = ChatCompletionMessageParam::User {
        content: UserContent::Text(input.content),
        name: None,
    };

    let before_snapshot = state
        .manager
        .encode_snapshot(&agent)
        .map_err(ApiError::internal)?;
    let before = persist_raw_checkpoint(
        &state,
        session_id,
        Some(Uuid::new_v4()),
        CheckpointKind::BeforeSolution,
        before_snapshot,
    )
    .await?
    .checkpoint;
    let solution_id = before
        .solution_id
        .ok_or_else(|| ApiError::internal("before checkpoint did not create a solution id"))?;
    let assistant = agent
        .evaluate(user_message)
        .await
        .ok_or_else(|| ApiError::internal("LLM did not produce a final assistant response"))?;
    let after_snapshot = state
        .manager
        .encode_snapshot(&agent)
        .map_err(ApiError::internal)?;
    let after = persist_raw_checkpoint(
        &state,
        session_id,
        Some(solution_id),
        CheckpointKind::AfterSolution,
        after_snapshot.clone(),
    )
    .await?
    .checkpoint;
    state
        .snapshots
        .lock()
        .await
        .insert(session_id, after_snapshot);

    Ok(Json(MessageResponse {
        assistant,
        before_checkpoint: checkpoint_summary(&state, before).await?,
        after_checkpoint: checkpoint_summary(&state, after).await?,
    }))
}

async fn list_session_checkpoints(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<CheckpointListResponse> {
    let checkpoints = state
        .manager
        .list_checkpoints(session_id)
        .await
        .map_err(ApiError::internal)?;
    let mut summaries = Vec::with_capacity(checkpoints.len());
    for checkpoint in checkpoints {
        summaries.push(checkpoint_summary(&state, checkpoint).await?);
    }
    Ok(Json(CheckpointListResponse {
        checkpoints: summaries,
    }))
}

async fn checkpoint_context(
    State(state): State<AppState>,
    Path(checkpoint_id): Path<Uuid>,
) -> ApiResult<CheckpointContextResponse> {
    let context = state
        .manager
        .load_checkpoint_context(checkpoint_id)
        .await
        .map_err(ApiError::internal)?;
    let local_ref_count = state
        .manager
        .store()
        .checkpoint_local_ref_count(checkpoint_id)
        .await
        .map_err(ApiError::internal)?;
    let message_count = context.snapshot.messages.len();

    Ok(Json(CheckpointContextResponse {
        checkpoint: context.checkpoint,
        messages: context.snapshot.messages,
        message_count,
        local_ref_count,
    }))
}

async fn fork_checkpoint(
    State(state): State<AppState>,
    Path(checkpoint_id): Path<Uuid>,
    Json(input): Json<ForkRequest>,
) -> ApiResult<ForkResponse> {
    let (session, checkpoint) = state
        .manager
        .fork_from_checkpoint(checkpoint_id, input.name)
        .await
        .map_err(ApiError::internal)?;
    let context = state
        .manager
        .load_checkpoint_context(checkpoint.id)
        .await
        .map_err(ApiError::internal)?;
    let agent = build_agent(context.snapshot).map_err(ApiError::internal)?;
    state.snapshots.lock().await.insert(
        session.id,
        state
            .manager
            .encode_snapshot(&agent)
            .map_err(ApiError::internal)?,
    );

    Ok(Json(ForkResponse {
        session,
        checkpoint: checkpoint_summary(&state, checkpoint).await?,
    }))
}

async fn checkout_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<CheckoutRequest>,
) -> ApiResult<CheckoutResponse> {
    let context = state
        .manager
        .load_checkpoint_context(input.checkpoint_id)
        .await
        .map_err(ApiError::internal)?;
    if context.checkpoint.session_id != session_id {
        return Err(ApiError {
            error: "checkpoint_session_mismatch".to_string(),
            details: format!(
                "checkpoint {} belongs to session {}",
                context.checkpoint.id, context.checkpoint.session_id
            ),
        });
    }

    let checkpoint = context.checkpoint.clone();
    let snapshot = context.snapshot;
    state.snapshots.lock().await.insert(session_id, snapshot);

    Ok(Json(CheckoutResponse {
        session_id,
        checkpoint: checkpoint_summary(&state, checkpoint).await?,
    }))
}

async fn debug_persist(State(state): State<AppState>) -> ApiResult<PersistDebugResponse> {
    let diagnostics = state
        .manager
        .store()
        .persist_diagnostics()
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(PersistDebugResponse {
        session_count: diagnostics.session_count,
        checkpoint_count: diagnostics.checkpoint_count,
        message_count: diagnostics.message_count,
        checkpoint_local_ref_count: diagnostics.checkpoint_local_ref_count,
    }))
}

pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let storage = RdbStorage::from_env().await?;
    let model = std::env::var("CHATBOX_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
    let addr = std::env::var("CHATBOX_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse::<SocketAddr>()
        .context("CHATBOX_ADDR must be a socket address")?;

    let state = AppState {
        manager: Arc::new(SessionManager::new_openai(storage)),
        snapshots: Arc::new(Mutex::new(HashMap::new())),
        model,
    };

    let static_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("chatbox_static");
    let app = Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{session_id}/messages", post(send_message))
        .route(
            "/api/sessions/{session_id}/checkpoints",
            get(list_session_checkpoints),
        )
        .route(
            "/api/sessions/{session_id}/checkout",
            post(checkout_session),
        )
        .route(
            "/api/checkpoints/{checkpoint_id}/context",
            get(checkpoint_context),
        )
        .route(
            "/api/checkpoints/{checkpoint_id}/fork",
            post(fork_checkpoint),
        )
        .route("/api/debug/persist", get(debug_persist))
        .fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("chatbox listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
