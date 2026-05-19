# AGENTS.md

## Project

**poprako-b-preview** is a Rust AI application built around a pluggable LLM resolver framework. It defines a generic `Resolver` trait that abstracts chat completion calls behind a uniform interface, enabling different backends (OpenAI, DeepSeek, etc.) to be swapped in without changing application code.

## Architecture (layers)

```
Resolver trait  ←──  OpenAiResolver (openai-oxide → OpenAI-compatible HTTP API)
    ↑
  Context        ←──  Messages + Tools  (conversation state)
    ↑
Application      ←──  consumes Resolver, builds Context, processes Actions
```

The stack has three tiers:
1. **Application layer** — owns the conversation loop, builds `Context`, calls `resolve()`, and acts on the returned `Action`.
2. **Resolver layer** — the `Resolver` trait and its `OpenAiResolver` implementation. Translates internal types (`Message`, `Tool`) into provider-specific API payloads, and maps responses back into a unified `Action`.
3. **HTTP layer** — a thin `HttpClient` wrapper around `reqwest` for direct REST calls (currently unused by the resolver, which delegates HTTP to `openai-oxide`).

## Key design decisions

- **Proprietary message/tool types** — The codebase defines its own `Message`, `Tool`, `Action`, and error types rather than coupling directly to any AI SDK. Resolver implementations are responsible for translation.
- **Resolver trait is async and mutable** — `async fn resolve(&mut self, cx: &mut Context)` allows implementations to hold connection pools or state, and the caller to mutate `Context` between turns.
- **Real-API integration tests** — No mocking. Tests require `.env` with valid credentials and hit the actual endpoint, verifying end-to-end behavior.

## Entrypoints

| File | Role |
|------|------|
| `src/main.rs` | Binary (stub) |
| `src/lib.rs` | Library root, enables `cargo test` |

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
```

Loaded via `dotenvy` at startup.
