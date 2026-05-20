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

## Tech Stack & Rationale

- **Rust** (edition 2024) — chosen over Bun/Python due to deployment on a server with limited memory and CPU.
- **DeepSeek** as the default LLM provider, accessed via OpenAI-compatible API.
- **NapCat** + **OneBot v11** for QQ integration (WebSocket events, HTTP actions).
- Key crates: `openai-oxide` (LLM client), `tokio` (async runtime), `reqwest` (HTTP), `serde`/`serde_json`.

## Architecture (layers)

```
NapCat/OneBot  ──→  Bot (event filter, context holder)
                       │
                       ├── Agent (wake: loop / single-trigger)
                       │      └── Resolver (pluggable trait)
                       │             └── OpenAiResolver → DeepSeek
                       │
                       └── Tools (static enum dispatch)
```

- **Bot** — Owns the OneBot connection, filters incoming events, holds `Context`. See `poprako-onebot-setup`.
- **Agent** — Wakes selectively (long-running loop or single-turn trigger) inside the Bot's context. See `poprako-agent-loop`.
- **Resolver** — Pluggable trait that translates proprietary types to provider-specific API payloads. See `poprako-resolver-design`.
- **Tools** — Compile-time enum defining available tools with static dispatch. See `poprako-tool-system`.

For deployment specifics, see `poprako-deployment`.

## Entrypoints

| File | Role |
|------|------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library root (enables `cargo test`) |

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
```

Loaded via `dotenvy` at startup. Copy `.env.sample` and fill in credentials.
