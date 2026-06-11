# PopRaKo-B

Bot-side app (`b`) of the PopRaKo / 白杨子 series. This repo runs the QQ bot: it accepts OneBot v11 reverse WebSocket events from NapCat, decides whether a message should wake the agent, calls DeepSeek through the resolver layer, and sends replies / scheduled messages back to QQ.

## What Is In This Repo

| Area | Files |
|------|-------|
| Entrypoints | `src/main.rs`, `src/lib.rs`, `src/bin/chatbox.rs` |
| Bot runtime | `src/bot.rs`, `src/bot/server.rs`, `src/bot/app.rs`, `src/bot/state.rs` |
| OneBot adapter | `src/bot/server/onebot.rs`, `src/bot/message.rs` |
| Agent core | `src/ai/agent.rs`, `src/ai/agent/interceptor.rs`, `src/ai/agent/tool/` |
| Resolver layer | `src/ai/resolver.rs`, `src/ai/resolver/`, `src/ai/resolver_impl/` |
| Session persistence | `src/ai/session.rs`, `src/ai/session/persist/`, `migrations/` |
| Prompt / memory | `memory/prompts/`, `memory/shards/`, `memory/inspirations/` |
| Agent-facing docs | `AGENTS.md`, `.agents/skills/`, `docs/` |

## Runtime Flow

```text
NapCat / OneBot reverse WS
  -> BotServer
  -> BotApp
  -> trigger / repeat / reply policies
  -> BotAgent
  -> generic Agent evaluate loop
  -> DeepSeekResolver
  -> local / remote tools
```

The normal wake-up paths are a leading mention of the bot or a `/prk` prefix. Pure text messages are also kept in short repeat-policy history.

## Configuration

Create a local `.env` as needed:

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

`OPENAI_*` variables are used by both resolver implementations. The default bot wiring uses `DeepSeekResolver` with model `deepseek-v4-flash`.

## Development

```bash
cargo check
cargo test
./.agents/skills/check-use-braces/scripts/check_use_braces.sh
```

Database-backed agent session persistence uses the SQL migrations in `migrations/` and the storage code under `src/ai/session/persist/`.
