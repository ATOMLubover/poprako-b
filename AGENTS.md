# AGENTS.md

## Project

**poprako-b-preview** is the Bot-side app (`b`) of the PopRaKo (白杨子) series. It is a Rust QQ chat bot that receives OneBot v11 reverse WebSocket events from NapCat, routes them through a bot event loop, maintains agent context, and executes local / remote tools through a pluggable LLM backend. DeepSeek is the production default.

## Series

| Code | Component | Description |
|------|-----------|-------------|
| `b` | poprako-b-preview | QQ Bot (this repo) |
| `s` | poprako-s | Business server |
| `w` | poprako-w | Web frontend |
| `n` | poprako-n | Windows desktop app |

Long-term memory is currently file-backed under `memory/`. Agent session persistence is implemented through PostgreSQL migrations and `src/ai/session/persist/`; an external MCP memory service may replace or augment parts of this later.

## Tech Stack

- **Rust** (edition 2024) — chosen over Bun/Python due to deployment on a server with limited memory and CPU.
- **DeepSeek** as the default LLM provider, using the local HTTP layer and OpenAI-compatible chat-completions schema.
- **OpenAI-compatible resolver** remains available through `openai-oxide`.
- **NapCat** + **OneBot v11** for QQ integration (reverse WebSocket events, HTTP actions).
- Key crates: `tokio`, `axum`, `onebot_v11`, `reqwest`, `openai-oxide`, `serde`/`serde_json`, `serde_yaml`, `sqlx`, `uuid`, `quick-xml`, `tracing`.

## Entrypoints

| File | Role |
|------|------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library root (enables `cargo test`) |
| `src/bin/chatbox.rs` | Local chatbox binary |

## Architecture Pointers

| Area | Files |
|------|-------|
| Bot event loop | `src/bot.rs`, `src/bot/server.rs`, `src/bot/app.rs`, `src/bot/event.rs` |
| OneBot adapter | `src/bot/server/onebot.rs`, `src/bot/message.rs` |
| Trigger / reply policy | `src/bot/policy/trigger.rs`, `src/bot/policy/reply.rs`, `src/bot/policy/repeat.rs` |
| Bot agent wiring | `src/bot/agent.rs`, `src/bot/agent/plugin/`, `src/bot/agent/tool/` |
| Generic agent loop | `src/ai/agent.rs`, `src/ai/agent/interceptor.rs`, `src/ai/agent/tool/`, `src/ai/agent/plugin/` |
| Resolver abstraction | `src/ai/resolver.rs`, `src/ai/resolver/` |
| Provider implementations | `src/ai/resolver_impl/deepseek.rs`, `src/ai/resolver_impl/openai.rs` |
| Session persistence | `src/ai/session.rs`, `src/ai/session/persist/`, `migrations/` |
| Prompt and memory data | `memory/prompts/`, `memory/shards/`, `memory/inspirations/` |

## Skills

- [poprako-b-overview](.agents/skills/poprako-b-overview/SKILL.md) — Architecture, bot event flow, trigger mechanism, prompt system, memory shards, runtime env.
- [poprako-resolver](.agents/skills/poprako-resolver/SKILL.md) — Resolver trait, Context, Message, Action, Tool definitions/calls, DeepSeek/OpenAI resolver implementations.
- [poprako-http](.agents/skills/poprako-http/SKILL.md) — `HttpClient` and `HttpError` in `src/http.rs`, including bearer-token JSON API calls used by DeepSeek.
- [interceptor-structure-explain](.agents/skills/interceptor-structure-explain/SKILL.md) — Generic agent lifecycle hooks, state, and per-message annotation boundaries.
- [check-use-braces](.agents/skills/check-use-braces/SKILL.md) — Rust import style linting.

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
ACCOUNT=your-qq-number
NAPCAT_REVERSE_WS_HOST=0.0.0.0
NAPCAT_REVERSE_WS_PORT=8081
NAPCAT_REVERSE_WS_SUFFIX=onebot/v11
NAPCAT_ACCESS_TOKEN=optional-token
DEVELOPER=optional-developer-qq-number
MEMORY_DIR=memory
RUST_LOG=info
```

Loaded via `dotenvy` at startup. There is currently no tracked `.env.sample`; create a local `.env` with the variables above as needed.
