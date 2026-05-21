---
name: check-use-braces
description: Checks Rust code for non-leaf curly braces in `use` statements (e.g. `a::{b, c::d}`), which are not allowed. Only `a::b::{c, d}` style is acceptable. Triggers when working on Rust formatting, refactoring, or style linting in this project.
---

# check-use-braces

Enforces a project-specific Rust import style: **`{}` in `use` statements must only appear at the final leaf path segment.** Items inside `{}` must be simple names without `::`.

## Rule

| ✅ Allowed | ❌ Rejected |
|---|---|
| `use a::b::{c, d};` | `use a::{b, c::d};` |
| `use a::b::c;` | `use a::{b::c, d::e};` |
| `use a::{b, c};` _(both leaves)_ | `use a::{b, c::d};` |

## Usage

Run from the project root:

```bash
# Scan all Rust source files
./.agents/skills/check-use-braces/scripts/check_use_braces.sh

# Scan specific files
./.agents/skills/check-use-braces/scripts/check_use_braces.sh src/ai/agent/openai.rs
```

## Expected output

```
✓ All use statements pass — no non-leaf braces found.
```

Or on violations:

```
src/ai/agent/openai.rs:3 - VIOLATION
    use crate::ai::{agent::Agent, resolver::openai::OpenAiResolver};
✗ Found 1 file(s) with violations.
```

## How the script works

1. Finds all `.rs` files under `src/` (excluding `target/`).
2. For each file, folds multi-line `use … ;` statements onto one line.
3. If the statement contains `{…}`, extracts the content between braces.
4. Splits on `,` and checks if **any** item contains `::`.
5. Reports any such occurrence — because `{}` at a non-leaf level violates the rule.
