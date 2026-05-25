---
name: poprako-b-overview
description: |
  Project overview and architecture of poprako-b-preview, the QQ chat bot (白杨子).
  Covers key source files, trigger mechanism, prompt system, memory shards, and
  event flow. Use when exploring the codebase, debugging bot behavior, modifying
  triggers or prompts, or understanding how components connect.
---

# poprako-b-overview

## Architecture

```
NapCat/OneBot  ──→  BotServer (reverse WS listener)
                       │
                   BotState (holds BotAgent)
                       │
               handle_group_message   ←  extract_user_text (trigger filter)
                       │
                   BotAgent.try_respond
                       │
                   OpenAiAgent  ──→  OpenAiResolver  ──→  DeepSeek API
                       │
                Tools: list_memory_shards / recall_memory_shard
```

## Key source files

| File | Role |
|------|------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library root |
| `src/bot/server.rs` | Reverse WebSocket server, event loop |
| `src/bot/handler.rs` | Group message handler, **trigger filter** (`extract_user_text`) |
| `src/bot/message.rs` | `Message` struct wrapping OneBot v11 group messages |
| `src/bot/agent.rs` | `BotAgent` — holds `OpenAiAgent`, orchestrates solve |
| `src/bot/agent/prompt.rs` | Loads system prompt from `memory/prompts/` |
| `src/ai/agent/openai.rs` | Generic OpenAi agent with compaction |
| `src/ai/resolver/openai.rs` | OpenAi resolver (translates to openai-oxide types) |

## Trigger mechanism

`src/bot/handler.rs` — `extract_user_text` checks two conditions:

1. **`@bot` at beginning** — First message segment (skipping Reply) must be `At { qq: self_id }`. Text after the @ is extracted.
2. **`/prk` prefix** — Raw message starts with `/prk`. Text after prefix is extracted.

If neither matches, the message is ignored.

## Prompt system

System prompt loaded at startup from:

| File | Purpose |
|------|---------|
| `memory/prompts/persona.txt` | Core identity, tone, style rules |
| `memory/prompts/rules.txt` | Behavioral constraints, tool-use protocol, examples |

Changes take effect on restart.

## Memory shards (`memory/shards/`)

File-based memory — each file is a named "shard" with factual knowledge:

```
memory/shards/
├── dev-team
├── group-overview
├── localization-workflow
├── poprako-s
├── poprako-w
├── poprako-w-page-comic-playground
├── poprako-w-page-error
├── poprako-w-page-login
├── poprako-w-page-member-list
├── poprako-w-page-settings
├── poprako-w-page-system-mail
├── poprako-w-page-translator
├── poprako-w-page-workspace
└── role-system
```

Tools: `list_memory_shards` lists available shards; `recall_memory_shard` reads one by name.

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
RUST_LOG=poprako_b_preview=info
NAPCAT_ACCESS_TOKEN=your-napcat-reverse-ws-token
ACCOUNT=your-qq-number
NAPCAT_UID=1000
NAPCAT_GID=1000
```

Loaded via `dotenvy` at startup.
