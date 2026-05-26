---
name: how-to-create-shard
description: Guidelines for creating memory shards — format, size limits, naming, and best practices
tags:
  - meta
  - shard-creation
  - guidelines
---

## Shard Structure

A memory shard is a file at `memory/shards/{name}/shard.md` with YAML frontmatter + Markdown body:

```
---
name: Display Name
description: Short summary of what this shard covers
tags:
  - tag1
  - tag2
---
(Markdown body — max 1024 characters)
```

## Writing Rules

1. **Body ≤ 1024 characters**. Longer content will be rejected by generate_memory_shard.
2. **Concise and factual**. Every sentence should carry useful knowledge. No fluff.
3. **Well-structured**. Use headings, bullet points, or short paragraphs for quick scanning.
4. **Self-contained**. Readable without other shards — cross-references are hints, not requirements.

## Naming

- `shard_name`: kebab-case directory name (e.g. `poprako-w`, `dev-team`)
- `display_name`: human-readable title for the frontmatter `name` field
- `tags`: comma-separated, lowercase, use relevant categories

## When To Create

Create a shard ONLY when explicitly instructed by LB or another developer.
Never create shards proactively — wait for a direct request like "add this to memory" or "create a shard for X".
