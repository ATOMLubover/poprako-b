---
name: poprako-resolver
description: |
  Internal skill for working on the poprako-b-preview resolver layer.
  Covers the Resolver trait, Context, Message, Action, Tool types, and the
  OpenAiResolver implementation. Use this whenever touching files under
  src/ai/resolver/ or src/ai/message.rs.
---

# Poprako Resolver Layer

## Files

```
src/ai/
├── mod.rs                   # pub mod message; pub mod resolver;
├── message.rs               # Message enum
└── resolver/
    ├── mod.rs               # Resolver trait, Context, ResolveError
    ├── action.rs            # Action struct, Reason enum
    ├── tool.rs              # Tool, Parameters, Property, ToolCall
    └── openai.rs            # OpenAiResolver impl
```

## Core Types

### `Message` (`src/ai/message.rs`)

```rust
#[derive(Debug)]
pub enum Message {
    System(String),
    User(String),
    Assistant(String),
}
```

Project-internal message type. Decoupled from any AI SDK so Resolver implementations handle translation.

### `Context` (`src/ai/resolver/mod.rs`)

```rust
pub struct Context {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
}
```

Holds everything needed for one `resolve()` call: the model name, the full conversation history, and available tool definitions. Construct via `Context::new(model, messages)` — tools default to empty.

### `Resolver` trait (`src/ai/resolver/mod.rs`)

```rust
#[async_trait]
pub trait Resolver: Send {
    async fn resolve(&mut self, cx: &mut Context) -> ResolveResult<Action>;
}
```

The central abstraction. Takes a mutable reference to `Context` and returns an `Action`. Implementations are `Send` so they can be passed across threads. `&mut self` allows implementations to hold connection pools or rate-limit state.

### `Action` (`src/ai/resolver/action.rs`)

```rust
pub struct Action {
    pub reason: Reason,                      // Finish | Length | ToolCall | Unknown(String)
    pub content: Option<String>,             // text response
    pub refusal: Option<String>,             // content-filter refusal
    pub tool_calls: Option<Vec<ToolCall>>,   // requested tool calls
}
```

The unified output of a resolver. Consumers switch on `reason` to decide what to do next (append text, execute tools, handle error).

### `Tool` / `ToolCall` (`src/ai/resolver/tool.rs`)

`Tool` defines a function the model can request. `ToolCall` is the model's invocation of one. Currently only deserialized from openai-oxide's tool call types; not yet serialized into requests by `build_history`.

### `ResolveError` (`src/ai/resolver/mod.rs`)

```rust
#[derive(Debug)]
pub enum ResolveError {
    NoResponse,
    ApiError { status: u16, message: String },
    RequestError(String),
    JsonError(String),
    Other(String),
}
```

Normalizes all provider errors into a single type.

## `OpenAiResolver` (`src/ai/resolver/openai.rs`)

The only Resolver implementation. Key methods:

| Method | Purpose |
|--------|---------|
| `from_env()` | Reads `OPENAI_API_KEY` and `OPENAI_BASE_URL` from env. Ensures trailing slash on base URL. Constructs an `openai_oxide::OpenAI` client. |
| `resolve(cx)` | Builds `ChatCompletionRequest` from `Context`, calls `client.chat().completions().create()`, extracts first choice → `Action`. |
| `build_history(cx)` | Maps internal `Message` to openai-oxide's `ChatCompletionMessageParam` variants. |
| `map_err(err)` | Converts `OpenAIError` into `ResolveError`. |
| `build_action(choice)` | Maps `ChatCompletionChoice` fields (finish reason, content, tool calls) into an `Action`. |

The resolver only processes the first choice from the API response. Tool calls are mapped from openai-oxide's `ToolCall` type to the project's own `ToolCall`.

## Testing

Tests are real API calls (no mocking). They live in `#[cfg(test)] mod tests` at the bottom of `openai.rs`:

- `single_turn_conversation` — one User message, expects `Reason::Finish` + non-empty content.
- `three_turn_conversation_with_context` — multi-message history (System + User + Assistant + User), verifies the model tracks conversational context.

Both use model `deepseek-v4-flash`. `.env` credentials required.
