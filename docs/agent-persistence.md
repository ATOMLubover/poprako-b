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
- Unit and integration tests for data objects, codecs, manager behavior, and
  PostgreSQL storage.

## Module Layout

```text
src/ai/agent.rs
src/ai/agent/persist.rs
src/ai/agent/persist/data_object.rs
src/ai/agent/persist/codec.rs
src/ai/agent/persist/storage.rs
src/ai/agent/persist/storage/rdb.rs
src/ai/agent/persist/storage/rdb/entity.rs
```

Responsibilities:

- `agent.rs`
  - Owns `Agent`, `AgentBuilder`, and `AgentManager`.
  - `AgentManager` orchestrates session creation, checkpoint creation, checkpoint
    loading, checkpoint decoding, and fork operations.
- `persist/data_object.rs`
  - Defines agent-layer persistence data objects: `Session`, `Checkpoint`,
    `ContextSnapshot`, `Message`, `ToolCall`, `Status`, `CheckpointKind`,
    `NewSession`, and `NewCheckpoint`.
  - These are the types used by `AgentManager`, codecs, and `IStorage`.
- `persist/codec.rs`
  - Defines `IMessageSnapshotCodec` and `IContextSnapshotCodec`.
  - Implements OpenAI message snapshot conversion via `OpenAiCodec`.
- `persist/storage.rs`
  - Defines the storage trait `IStorage`.
- `persist/storage/rdb.rs`
  - Implements `IStorage` with PostgreSQL in `RdbStorage`.
  - Uses `sqlx::query!` with raw multiline SQL strings for compile-time checked
    SQL.
- `persist/storage/rdb/entity.rs`
  - Defines RDB-only storage entities and database value mapping.
  - This module is private to the PostgreSQL implementation and is not exposed
    to `AgentManager` or external callers.

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

`AgentManager<S, M, C>` is generic over:

- `S: IStorage`
- `M: IMessage`
- `C: IContextSnapshotCodec<M>`

The codec type is constrained on the manager type itself; there is no
unconstrained codec parameter and no default generic parameter.

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

`persist::storage::IStorage` is the storage trait.
`persist::storage::rdb::RdbStorage` is the PostgreSQL implementation.

Construction:

```rust
let storage = poprako_b_preview::ai::agent::persist::storage::rdb::RdbStorage::from_env().await?;
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

- RDB entity enum-to-database string mapping
- data object JSON serialization
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
