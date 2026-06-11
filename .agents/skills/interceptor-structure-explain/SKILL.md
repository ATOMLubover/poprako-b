---
name: interceptor-structure-explain
description: Explains the current poprako-b-preview Agent interceptor, Context annotation, plugin, tool, compaction, and Agent state structure. Use whenever discussing or modifying generic Agent lifecycle hooks, interceptor behavior, tool call interception, Context annotations, Agent state, compaction, or plugin wiring in src/ai/agent* and resolver context code.
---

# interceptor-structure-explain

Use this skill to recall the structure and intended boundaries of the generic agent extension system.

## Three Layers

- `IInterceptor<S, M, A>` observes and can alter the agent evaluate lifecycle.
- `S` is agent-level mutable runtime state shared across hooks.
- `A` is per-message annotation stored beside each resolver-visible message in `Context<M, A>`.

Keep the distinction clear:

- Put LLM-visible content in `Context` messages.
- Put per-message metadata in `AnnotatedMessage.annotation`.
- Put cross-hook runtime data in `Agent.state`.
- Put reusable tool/interceptor bundles behind `IAgentPlugin`.

## Interceptor Lifecycle

`Agent::evaluate` runs hooks in this order:

```text
before_evaluate

loop:
  before_loop
  before_resolve
  resolve
  after_resolve
  before_tool_call / after_tool_call
  before_commit_messages
  commit messages
  after_loop

after_evaluate
```

Normal hooks return `InterceptorFlow`:

- `Continue`
- `Stop { output }`

Tool pre-call hooks return `ToolInterceptorFlow`:

- `Continue`
- `Skip { content }`
- `Stop { output }`

The registry runs interceptors in insertion order and stops the current hook chain on the first non-continue result.

## Tool Handling

`Agent` stores local tools by name and can also hold an optional `RemoteProxy`.

Tool resolution order:

1. Local tool with matching name.
2. Remote proxy if it advertises the tool name.
3. `ExecutionError::exec_fail("tool not found: ...")`.

`before_tool_call` can skip execution and fabricate a tool result. `after_tool_call` can inspect or rewrite the result before it becomes a tool message.

When tools are rebuilt, `Agent::refresh_tools` updates `Context.tool_defs`. Remote tool definitions are appended unless a local tool with the same name exists.

## State

`Agent<M, R, S, A>` owns `state: S`.

Every interceptor receives `&mut S`, so state is the place for data that needs to survive across hooks or solve iterations. It is not automatically visible to the resolver.

`BotAgentState` currently embeds inspiration plugin state. `BotMessageAnnotation` embeds inspiration per-message annotation.

## Annotation

`Context<M, A>` stores messages as:

```rust
Vec<AnnotatedMessage<M, A>>
```

Each item contains:

```rust
message: M,
annotation: A,
```

`message` is visible to the resolver. `annotation` is internal metadata unless some code explicitly converts it into a message.

Useful APIs:

- `push_message` uses `A::default()`.
- `push_annotated_message` preserves explicit metadata.
- `annotated_messages` exposes message plus annotation.
- `messages` exposes only resolver-visible messages.
- `inject_before_last` inserts an annotated message immediately before the latest message.
- `take_annotated_messages` / `set_annotated_messages` support compaction or persistence-style rewrites.

Memory, compaction, tracing, or persistence code may use these hooks and annotations, but they are examples of consumers, not rules baked into this structure.

## Compaction

`Agent` owns an optional `ICompact` implementation. `Agent::evaluate` pushes the incoming user message and calls `compact()` before `before_evaluate`.

The bot wiring currently uses `BotCompact` from the inspiration plugin.
