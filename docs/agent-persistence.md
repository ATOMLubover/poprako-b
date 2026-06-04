# Agent 持久化：增量 Checkpoint 链

## 概述

将原先"checkpoint 存完整 `ContextSnapshot.messages`" 改为三层语义：

- **Message**：不可变的消息内容原子，只存一次，用 SHA-256 hash 去重。
- **Checkpoint**：不可变的上下文位置，指向 `base_checkpoint_id`，只存相对 base 新增的有序 message 引用。
- **Session**：可继续写入的会话分支，通过 `forked_from_checkpoint_id` 记录起点。

## 数据库表

### agent_messages

消息内容表。同一内容（相同 role + payload）只存一条，由 `payload_hash` UNIQUE 约束保证。

| 列 | 类型 | 说明 |
|---|---|---|
| `id` | UUID PK | 由 `payload_hash` 前 16 字节导出的确定性 UUID |
| `payload_hash` | BYTEA UNIQUE | serde_json 规范序列化后的 SHA-256 |
| `role` | TEXT | `system` / `user` / `assistant` / `tool` |
| `payload` | JSONB | 完整消息的 JSON |
| `created_at` | TIMESTAMPTZ | |

### agent_sessions

会话（分支）表。

| 列 | 类型 | 说明 |
|---|---|---|
| `id` | UUID PK | |
| `name` | TEXT? | 可选名称 |
| `model` | TEXT | 默认模型 |
| `status` | TEXT | `active` / `archived` |
| `forked_from_checkpoint_id` | UUID? | fork 起点 checkpoint |
| `created_at` | TIMESTAMPTZ | |
| `updated_at` | TIMESTAMPTZ | |

### agent_checkpoints

检查点元数据表，**不存储消息内容**。

| 列 | 类型 | 说明 |
|---|---|---|
| `id` | UUID PK | |
| `session_id` | UUID FK | 所属会话 |
| `solution_id` | UUID? | before/after 配对 ID |
| `kind` | TEXT | `before_solution` / `after_solution` / `fork` |
| `model` | TEXT | 该检查点使用的模型 |
| `base_checkpoint_id` | UUID? | 自引用父检查点；NULL = reset 根 |
| `created_at` | TIMESTAMPTZ | |

索引：`(session_id, created_at, id)`

### agent_checkpoint_messages

增量消息引用 join 表。

| 列 | 类型 | 说明 |
|---|---|---|
| `checkpoint_id` | UUID FK | |
| `position` | INTEGER | 在基序列中的位置 |
| `message_id` | UUID FK | 指向 `agent_messages.id` |

主键：`(checkpoint_id, position)`

## Checkpoint 增量 / Reset 逻辑

创建 checkpoint 时（在一个 DB 事务内）：

1. `SELECT ... FOR UPDATE` 锁目标 session。
2. 查找该 session 最新 checkpoint 的 id 作为 base；若无，使用 `session.forked_from_checkpoint_id`。
3. 重建 base 的完整 message 序列。
4. **增量路径**：若当前 context 以 base 为前缀，则本 checkpoint 只保存 suffix 的 message refs，`base_checkpoint_id` 指向父。
5. **Reset 路径**：若非前缀（compact、system prompt reload 或其他上下文重写），`base_checkpoint_id = NULL`，引用当前全部 messages。

### 加载上下文

`load_checkpoint_context(checkpoint_id)`：
1. 沿 `base_checkpoint_id` 链从 root 到目标逐级读取各 checkpoint 的本地 refs。
2. join `agent_checkpoint_messages` + `agent_messages` 获取实际消息。
3. 按 checkpoint 链顺序 + position 拼接完整 message 序列。
4. 使用目标 checkpoint 的 `model` 生成 `ContextSnapshot`。

### Fork

`fork_session_from_checkpoint(parent_checkpoint_id, name)`：
1. 创建新 session，`forked_from_checkpoint_id` = parent checkpoint。
2. 创建 `kind = fork` 的 checkpoint，`base_checkpoint_id` = parent checkpoint。
3. 该 fork checkpoint 无本地 message refs——加载时沿 base 链追溯父 checkpoint 的完整上下文。

不复制任何消息。

## 迁移文件

4 个 migration，每个只创建一张表（按依赖顺序）：

| 文件 | 内容 |
|---|---|
| `20260604130000_create-agent-messages` | agent_messages |
| `20260604130100_create-agent-sessions` | agent_sessions |
| `20260604130200_create-agent-checkpoints` | agent_checkpoints + 索引 + FK |
| `20260604130300_create-agent-checkpoint-messages` | agent_checkpoint_messages |

正反迁移均完备，`sqlx migrate revert` 可逆序回滚。

## 代码层

### 数据对象 (`src/ai/agent/persist/data_object.rs`)

- `Session`：`forked_from_checkpoint_id` 替代 `parent_session_id` + `parent_checkpoint_id`。
- `Checkpoint`：纯元数据，不包含 `snapshot`。新增 `base_checkpoint_id`。
- `ContextSnapshot`：保留为 codec 边界类型，非存储实体。
- `CheckpointContext`：`{ checkpoint: Checkpoint, snapshot: ContextSnapshot }`——加载 checkpoint 上下文时的返回类型。
- `hash_message(&Message) -> Vec<u8>`：SHA-256 哈希工具。

### 存储接口 (`src/ai/agent/persist/storage.rs`)

```rust
async fn create_checkpoint(&self, input: NewCheckpoint) -> anyhow::Result<Checkpoint>;
async fn get_checkpoint(&self, checkpoint_id: Uuid) -> anyhow::Result<Checkpoint>;
async fn list_checkpoints(&self, session_id: Uuid) -> anyhow::Result<Vec<Checkpoint>>;
async fn load_checkpoint_context(&self, checkpoint_id: Uuid) -> anyhow::Result<CheckpointContext>;
async fn fork_session_from_checkpoint(&self, parent_checkpoint_id: Uuid, name: Option<String>)
    -> anyhow::Result<(Session, Checkpoint)>;
```

- `get_checkpoint` / `list_checkpoints` 返回 metadata，**不隐式加载完整上下文**。
- 需要完整上下文时显式调用 `load_checkpoint_context`。
- `NewCheckpoint` 接受 `messages: Vec<Message>`，存储层内部决定增量/reset。

### AgentManager 使用方式

```rust
// 创建 checkpoint（存储层自动决定增量/reset）
let cp = manager.checkpoint_before_solution(session_id, &agent).await?;

// 加载完整上下文
let ctx = manager.load_checkpoint_context(cp.id).await?;
let context: Context<M> = manager.decode_snapshot(&ctx.snapshot)?;

// Fork
let (fork_session, fork_cp) = manager.fork_from_checkpoint(cp.id, Some("分支名")).await?;
```

## 假设

- agent_messages 是内容原子，不表达"某用户在某时间发了第几次相同内容"——顺序由 checkpoint refs 表达。
- compact 不作为删除历史的操作持久化；它只会导致 reset checkpoint。
- bot 层 QQ 路由 session 不纳入本次重设计。
