# Agent Persistence

This document describes the persistence features currently implemented for the
agent layer.

The implementation is scoped to `src/ai/agent`. It does not persist bot state,
QQ message routing, resolver clients, tool instances, or tool definitions.

## What Is Implemented

- Persistent sessions for agent conversations.
- Persistent checkpoints for agent context snapshots.
- Forking a new session from an existing checkpoint.
- Snapshot encoding and decoding for OpenAI chat messages.
- PostgreSQL storage backed by `sqlx`.
- Reversible SQL migrations created with `sqlx migrate add -r`.
- Unit and integration tests for entities, codecs, manager behavior, and
  PostgreSQL storage.

## Module Layout

```text
src/ai/agent.rs
src/ai/agent/persist.rs
src/ai/agent/persist/entity.rs
src/ai/agent/persist/codec.rs
src/ai/agent/persist/store.rs
src/ai/agent/persist/postgres.rs
```

Responsibilities:

- `agent.rs`
  - Owns `Agent`, `AgentBuilder`, and `AgentManager`.
  - `AgentManager` orchestrates session creation, checkpoint creation, checkpoint
    loading, checkpoint decoding, and fork operations.
- `persist/entity.rs`
  - Defines persistence-domain data types: `Session`, `Checkpoint`,
    `ContextSnapshot`, `Message`, `ToolCall`, `Status`, `CheckpointKind`,
    `NewSession`, and `NewCheckpoint`.
- `persist/codec.rs`
  - Defines `MessageSnapshotCodec` and `ContextSnapshotCodec`.
  - Implements OpenAI message snapshot conversion via `OpenAiCodec`.
- `persist/store.rs`
  - Defines the storage trait `Store`.
- `persist/postgres.rs`
  - Implements `Store` with PostgreSQL in `Storage`.
  - Uses `sqlx::query!` with raw multiline SQL strings for compile-time checked
    SQL.

## Stored Data

The persisted context snapshot contains only:

- `model`
- `messages`

`tool_defs` are intentionally not stored. On resume or fork, tools are expected
to be rebuilt from the current runtime tool registry. Existing context messages
still preserve assistant tool-call names and tool result messages, so the model
can infer prior tool activity from history.

Message variants:

- `system { content }`
- `user { content }`
- `assistant { content, refusal, tool_calls }`
- `tool { tool_call_id, content }`

Tool calls store:

- `id`
- `name`
- `args`

`args` is stored as the raw argument string produced by the model.

## Database Schema

Migrations live under `migrations/`:

- `create-session-table`
- `create-checkpoint-table`

Tables:

- `agent_sessions`
  - session identity, model, status, parent session/checkpoint references, and
    timestamps.
- `agent_checkpoints`
  - checkpoint identity, session reference, optional run id, checkpoint kind,
    model, JSONB messages, and timestamp.

The checkpoint migration also adds the `agent_sessions.parent_checkpoint_id`
foreign key after both tables exist.

## AgentManager API

`AgentManager<S, C>` is generic over:

- `S: Store`
- `C: ContextSnapshotCodec<M>`

There is no default generic parameter.

Implemented operations:

- `new(store, codec)`
- `new_openai(store)`
- `store()`
- `create_session(model, name)`
- `archive_session(session_id)`
- `load_checkpoint(checkpoint_id)`
- `list_checkpoints(session_id)`
- `fork_from_checkpoint(parent_checkpoint_id, name)`
- `checkpoint_before_run(session_id, agent)`
- `checkpoint_after_run(session_id, run_id, agent)`
- `decode_checkpoint(checkpoint)`
- `snapshot_from_agent(agent)`

`checkpoint_before_run` creates a fresh `run_id`. The caller passes that same
`run_id` into `checkpoint_after_run` to pair the before/after checkpoints.

## Fork Semantics

Forking is implemented by copying the parent checkpoint snapshot into a new
session.

The forked session records:

- `parent_session_id`
- `parent_checkpoint_id`

The fork operation also creates an initial `fork` checkpoint under the new
session. The operation is executed in one PostgreSQL transaction.

There is no merge implementation.

## PostgreSQL Storage

`persist::Storage` is the PostgreSQL implementation.

Construction:

```rust
let storage = poprako_b_preview::ai::agent::persist::Storage::from_env().await?;
```

`from_env` reads:

```env
DATABASE_URL=postgresql://...
```

The implementation uses:

- `sqlx::query!`
- raw multiline SQL strings with `r#"... "#`
- JSONB storage for `Vec<Message>`
- transactions for fork

## Tests

Implemented tests cover:

- entity enum-to-database string mapping
- entity JSON serialization
- context snapshot JSON round trip
- OpenAI message codec round trip
- assistant tool-call preservation
- omission of `tool_defs` from snapshots
- `AgentManager` create/archive/checkpoint/decode/fork behavior using a fake
  store
- PostgreSQL session lifecycle
- PostgreSQL checkpoint lifecycle and ordering
- PostgreSQL fork behavior
- PostgreSQL foreign-key failure for checkpoints without sessions

The full test suite currently passes:

```text
cargo test
70 passed
```

## Current Non-Goals

The current implementation does not provide:

- bot-layer session routing
- automatic persistence inside `BotAgent::try_respond`
- resolver client persistence
- tool instance persistence
- tool definition persistence
- event-sourcing replay
- branch merge
- tool result cache
