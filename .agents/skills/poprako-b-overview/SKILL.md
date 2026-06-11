---
name: poprako-b-overview
description: |
  Project overview and architecture of poprako-b-preview, the QQ chat bot (白杨子).
  Use this skill whenever exploring or changing bot runtime flow, OneBot/NapCat
  integration, trigger or reply policy, prompt loading, memory shards, embedded
  tools, scheduled events, or agent wiring in this project.
---

# poprako-b-overview

## Architecture

```
NapCat / OneBot reverse WS
    -> BotServer
    -> BotApp
    -> BotState
    -> trigger / repeat / reply policy
    -> BotAgent
    -> DeepSeekAgent
    -> generic Agent evaluate loop
    -> DeepSeekResolver
    -> local tools / remote proxy
```

## Key source files

| File | Role |
|------|------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library root |
| `src/bot.rs` | Wires `BotAgent`, server config, event sources, and `BotServer::serve` |
| `src/bot/server.rs` | Reverse WebSocket server and event multiplexer |
| `src/bot/server/onebot.rs` | OneBot event/message adapter and command sender |
| `src/bot/app.rs` | Bot event dispatcher and message handling |
| `src/bot/policy/trigger.rs` | Wake-up trigger filter (`extract_user_text`) |
| `src/bot/policy/reply.rs` | Splits agent replies into OneBot commands |
| `src/bot/policy/repeat.rs` | Short repeat-message behavior |
| `src/bot/message.rs` | Internal channel message / command model |
| `src/bot/agent.rs` | `BotAgent` wiring for DeepSeek, prompts, tools, plugins, compaction |
| `src/bot/agent/prompt.rs` | Loads XML system prompt from `memory/prompts/system.yaml` |
| `src/bot/agent/plugin/inspiration/` | Inspiration knowledge injection, annotation, state, compaction |
| `src/ai/agent.rs` | Generic agent loop, local tools, remote proxy, compaction, interceptors |
| `src/ai/resolver.rs` + `src/ai/resolver/` | Provider-neutral resolver traits and message/action/tool/context types |
| `src/ai/resolver_impl/deepseek.rs` | Default DeepSeek resolver using `HttpClient` |
| `src/ai/resolver_impl/openai.rs` | OpenAI-compatible resolver using `openai-oxide` raw calls |
| `src/ai/session/persist/` | Session/checkpoint persistence codec, data objects, and storage |

## Trigger mechanism

`src/bot/policy/trigger.rs` — `extract_user_text` checks two conditions:

1. **`@bot` at beginning** — first message segment after any `Reply` segments must be `Mention { actor_id: self_id }`. Text segments after the mention are joined and trimmed.
2. **`/prk` prefix** — raw message starts with `/prk`. Text after the prefix is trimmed.

If neither matches, the message is ignored.

## Prompt system

System prompt is loaded at startup from `memory/prompts/system.yaml`. The manifest lists enabled embedded sections. Each section points to a `.txt` file and is rendered into XML via `SystemMessage`.

| File | Purpose |
|------|---------|
| `memory/prompts/system.yaml` | Ordered prompt manifest |
| `memory/prompts/persona.txt` | Core identity |
| `memory/prompts/directory.txt` | Prompt directory |
| `memory/prompts/scene.txt` | Runtime scene |
| `memory/prompts/input-format.txt` | User message envelope |
| `memory/prompts/injected-context.txt` | Runtime injected context format |
| `memory/prompts/knowledge-tools.txt` | Knowledge/tool instructions |
| `memory/prompts/response-style.txt` | Speaking style |
| `memory/prompts/safety.txt` | Account safety constraints |
| `memory/prompts/examples.txt` | Conversation examples |

`watch_system_prompt` polls the manifest and enabled prompt files every 30 seconds and reloads `messages[0]` when they change.

## Memory shards (`memory/shards/`)

File-based memory — each directory under `memory/shards/` is a named shard and contains `shard.md`:

```
memory/shards/
├── baiyang-overview/shard.md
├── dev-team/shard.md
├── how-to-create-shard/shard.md
├── localization-workflow/shard.md
├── poprako-role-system/shard.md
├── poprako-s/shard.md
├── poprako-w/shard.md
└── poprako-w-page-*/shard.md
```

The embedded memory-shard plugin provides tools for listing and recalling shards. `MEMORY_DIR` can override the default `memory` root.

## Agent Plugins And Tools

`BotAgent::new` currently builds a `DeepSeekAgent` with:

- `websearch_plugin()`
- `prks_plugin_from_env()`
- `inspiration_plugin(memory_dir)`
- `memory_shard_plugin(memory_dir)`
- optional remote tools from `RemoteProxy::from_local_config()`

Local tool definitions are copied into `Context.tool_defs`; remote tool names are skipped if they conflict with local tool names.

## Environment

```env
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.deepseek.com/v1
ACCOUNT=your-qq-number
NAPCAT_REVERSE_WS_HOST=0.0.0.0
NAPCAT_REVERSE_WS_PORT=8081
NAPCAT_REVERSE_WS_SUFFIX=onebot/v11
RUST_LOG=poprako_b_preview=info
NAPCAT_ACCESS_TOKEN=optional-token
DEVELOPER=optional-developer-qq-number
MEMORY_DIR=memory
```

Loaded via `dotenvy` at startup.
