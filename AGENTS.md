# AGENTS.md

## Project

**poprako-b-preview** is the Bot-side app (`b`) of the PopRaKo (白杨子) series — a QQ chat bot built in Rust. It connects to QQ via the OneBot protocol through NapCat, maintains conversational memory, and executes tools through a pluggable LLM backend (DeepSeek by default).

## Series

| Code | Component | Description |
|------|-----------|-------------|
| `b` | poprako-b-preview | QQ Bot (this repo) |
| `s` | poprako-s | Business server |
| `w` | poprako-w | Web frontend |
| `n` | poprako-n | Windows desktop app |

An MCP-based memory service (separate repo, no embedder) is planned as an external dependency of `b`.

## Tech Stack

- **Rust** (edition 2024) — chosen over Bun/Python due to deployment on a server with limited memory and CPU.
- **DeepSeek** as the default LLM provider, accessed via OpenAI-compatible API.
- **NapCat** + **OneBot v11** for QQ integration (reverse WebSocket events, HTTP actions).
- Key crates: `openai-oxide`, `tokio`, `reqwest`, `serde`/`serde_json`, `onebot_v11`.

## Entrypoints

| File | Role |
|------|------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library root (enables `cargo test`) |

## Skills

- [poprako-b-overview](.agents/skills/poprako-b-overview/SKILL.md) — Architecture, key files, trigger mechanism, prompt system, memory shards.
- [poprako-conventions](~/.agents/skills/poprako-conventions/SKILL.md) — Coding conventions (naming, modules, visibility, errors, async).
- [poprako-resolver](~/.agents/skills/poprako-resolver/SKILL.md) — Resolver trait, Context, Message, Action, Tool types, OpenAiResolver.
- [poprako-http](~/.agents/skills/poprako-http/SKILL.md) — HttpClient and HttpError in `src/http.rs`.
- [check-use-braces](.agents/skills/check-use-braces/SKILL.md) — Rust import style linting.

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
```

Loaded via `dotenvy` at startup. Copy `.env.sample` and fill in credentials.
