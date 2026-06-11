---
name: poprako-resolver
description: |
  Internal skill for working on the poprako-b-preview resolver layer.
  Covers provider-neutral Context, Message, Action, Tool definitions/calls,
  the IResolver trait, and DeepSeek/OpenAI resolver implementations. Use this
  whenever touching resolver traits, provider adapters, chat message mapping,
  tool schema/call mapping, or files under src/ai/resolver*.
---

# Poprako Resolver Layer

## Files

```
src/ai/
├── resolver.rs              # IResolver trait, ResolveError, ResolveResult
├── resolver/
│   ├── context.rs           # Context, AnnotatedMessage, ContextBuilder
│   ├── message.rs           # IMessage, MessageRef, MessageOwned, SystemMessage
│   ├── action.rs            # Action struct, Reason enum
│   └── tool.rs              # ToolDefination, ParamDef, PropDef, IToolCall
├── resolver_impl.rs
└── resolver_impl/
    ├── deepseek.rs          # Default DeepSeekResolver
    ├── deepseek/            # DeepSeek message/tool data objects and mappings
    ├── openai.rs            # OpenAiResolver
    └── openai/              # OpenAI message/tool/context mappings
```

## Core Types

### Messages (`src/ai/resolver/message.rs`)

The resolver layer is generic over message type. `IMessage` is the provider-neutral contract; provider impls supply concrete message types such as `DeepSeekMessage` or `openai_oxide::types::chat::ChatCompletionMessageParam`.

Important types:

- `MessageRef<'a, C>` — borrowed view of system/user/assistant/tool messages.
- `MessageOwned<C>` — owned message constructor used by bot wiring and agent commits.
- `SystemMessage` — renders embedded and plugin system sections into XML.
- `IMessage` — requires conversions from `Action`, `MessageRef`, and `MessageOwned`.

### `Context` (`src/ai/resolver/context.rs`)

```rust
pub struct Context<M, A = ()>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<AnnotatedMessage<M, A>>,
    tool_defs: Vec<ToolDefination>,
}
```

Holds everything needed for one resolve call: model, annotated conversation history, and available tool definitions. `messages()` exposes resolver-visible messages only; `annotated_messages()` preserves internal metadata.

Construct with `Context::new(model)` or `ContextBuilder::new(model)`.

### `IResolver` trait (`src/ai/resolver.rs`)

```rust
#[async_trait]
pub trait IResolver {
    type Message: IMessage + 'static;

    async fn resolve<A>(
        &mut self,
        cx: &Context<Self::Message, A>,
    ) -> ResolveResult<Action<<Self::Message as IMessage>::ToolCall>>
    where
        A: Send + Sync + 'static;
}
```

The central abstraction. It borrows `Context` immutably and returns an `Action`. `&mut self` allows provider implementations to hold client or rate-limit state.

### `Action` (`src/ai/resolver/action.rs`)

```rust
pub struct Action<C>
where
    C: IToolCall + std::fmt::Debug,
{
    pub reason: Reason,                      // Finish | Length | ToolCall | Unknown(String)
    pub content: Option<String>,             // text response
    pub refusal: Option<String>,             // content-filter refusal
    pub tool_calls: Option<Vec<C>>,          // requested tool calls
}
```

The unified output of a resolver. Consumers switch on `reason` to decide what to do next (append text, execute tools, handle error).

### `ToolDefination` / `IToolCall` (`src/ai/resolver/tool.rs`)

`ToolDefination` defines a function the model can request. The project keeps the historic spelling `Defination` in the type name.

`ParamDef` and `PropDef` represent the JSON schema sent to provider APIs. `IToolCall` is the trait every provider-specific tool-call type must implement:

```rust
pub trait IToolCall {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn args(&self) -> &str;
}
```

### `ResolveError` (`src/ai/resolver/mod.rs`)

```rust
#[derive(Debug)]
pub enum ResolveError {
    NoChoice { message: String },
    Api { status: u16, message: String },
    Network { message: String },
    JsonSerde { message: String },
    Unknown { message: String },
}
```

Normalizes all provider errors into a single type.

## `DeepSeekResolver` (`src/ai/resolver_impl/deepseek.rs`)

The default bot resolver. It uses `HttpClient` directly against `OPENAI_BASE_URL/chat/completions` and maps the response into `Action<DeepSeekToolCall>`.

Key points:

- `from_env()` reads `OPENAI_API_KEY` and `OPENAI_BASE_URL`; default base URL is `https://api.deepseek.com/v1`.
- Requests include `model`, `messages`, and optional `tools` + `tool_choice = "auto"`.
- Assistant history messages get `reasoning_content: ""` to satisfy DeepSeek V4 thinking-mode requirements.
- `build_action(choice)` maps `finish_reason`, content, refusal, and tool calls.
- Unit tests cover action mapping without network calls.

## `OpenAiResolver` (`src/ai/resolver_impl/openai.rs`)

OpenAI-compatible resolver that uses `openai-oxide` request types but sends the final payload with `create_raw()` so project-specific request shaping can still happen.

Key methods:

| Method | Purpose |
|--------|---------|
| `from_env()` | Reads `OPENAI_API_KEY` and `OPENAI_BASE_URL` from env. Strips trailing slash because `openai-oxide` concatenates paths directly. |
| `resolve(cx)` | Builds `ChatCompletionRequest`, maps tools, injects `reasoning_content`, calls `create_raw()`, extracts first choice. |
| `map_err(err)` | Converts `OpenAIError` into `ResolveError`. |
| `build_action(choice)` | Maps raw JSON choice fields into `Action<openai_oxide::ToolCall>`. |

Both resolvers only process the first choice from the API response.

## Testing

DeepSeek resolver tests are mostly unit tests around `build_action` and do not require network access.

OpenAI resolver tests include real API calls in `#[cfg(test)] mod tests` at the bottom of `src/ai/resolver_impl/openai.rs`:

- `single_turn_conversation` — one User message, expects `Reason::Finish` + non-empty content.
- `three_turn_conversation_with_context` — multi-message history (System + User + Assistant + User), verifies the model tracks conversational context.

Both use model `deepseek-v4-flash`. `.env` credentials required.
