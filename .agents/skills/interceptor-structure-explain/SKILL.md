---
name: interceptor-structure-explain
description: Explains the current poprako-b-preview Agent interceptor, Context annotation, and Agent state structure. Use when discussing or modifying Agent lifecycle hooks, interceptor behavior, Context annotations, or Agent state in src/ai/agent.rs, src/ai/agent/interceptor.rs, or src/ai/resolver/context.rs.
---

# interceptor-structure-explain

Use this skill to recall the structure and intended boundaries of the generic agent extension system.

## Three Layers

- `Interceptor<S, M, A>` observes and can alter the agent solve lifecycle.
- `S` is agent-level mutable runtime state shared across hooks.
- `A` is per-message annotation stored beside each resolver-visible message in `Context<M, A>`.

Keep the distinction clear:

- Put LLM-visible content in `Context` messages.
- Put per-message metadata in `AnnotatedMessage.annotation`.
- Put cross-hook runtime data in `Agent.state`.

## Interceptor Lifecycle

`Agent::solve` runs hooks in this order:

```text
before_solve

loop:
  before_loop
  before_resolve
  resolve
  after_resolve
  before_tool_call / after_tool_call
  before_commit_messages
  commit messages
  after_loop

after_solve
```

Normal hooks return `InterceptorFlow`:

- `Continue`
- `Stop { output }`

Tool pre-call hooks return `ToolInterceptorFlow`:

- `Continue`
- `Skip { content }`
- `Stop { output }`

The registry runs interceptors in insertion order and stops the current hook chain on the first non-continue result.

## State

`Agent<M, R, S, A>` owns `state: S`.

Every interceptor receives `&mut S`, so state is the place for data that needs to survive across hooks or solve iterations. It is not automatically visible to the resolver.

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

Memory, compaction, tracing, or persistence code may use these hooks and annotations, but they are examples of consumers, not rules baked into this structure.

