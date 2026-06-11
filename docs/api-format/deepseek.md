# DeepSeek V4 API Format

本文只记录当前项目关心的 DeepSeek V4 Flash / V4 Pro Chat Completions API 格式。

资料来源：

- DeepSeek Chat Completions: https://api-docs.deepseek.com/zh-cn/api/create-chat-completion/
- DeepSeek 思考模式: https://api-docs.deepseek.com/zh-cn/guides/thinking_mode/
- DeepSeek Tool Calls: https://api-docs.deepseek.com/zh-cn/guides/tool_calls/
- OpenAI API Reference: https://platform.openai.com/docs/api-reference

## Scope

目标模型：

- `deepseek-v4-flash`
- `deepseek-v4-pro`

目标接口：

```http
POST https://api.deepseek.com/chat/completions
```

项目环境中可继续使用：

```env
OPENAI_BASE_URL=https://api.deepseek.com/v1
```

SDK 会在 base URL 后拼接 `/chat/completions`。

## Request Body

最小请求：

```json
{
  "model": "deepseek-v4-flash",
  "messages": [
    {
      "role": "user",
      "content": "你好"
    }
  ]
}
```

V4 Pro 思考模式请求：

```json
{
  "model": "deepseek-v4-pro",
  "messages": [
    {
      "role": "user",
      "content": "分析这个问题"
    }
  ],
  "reasoning_effort": "high",
  "thinking": {
    "type": "enabled"
  }
}
```

常用根字段：

| 字段 | 类型 | V4 说明 |
|------|------|---------|
| `model` | string | `deepseek-v4-flash` 或 `deepseek-v4-pro`。 |
| `messages` | array | 对话消息数组。 |
| `stream` | boolean | 是否使用 SSE 流式返回。 |
| `max_tokens` | integer | 单次回答最大长度，思考内容也计入输出长度。 |
| `tools` | array | Function/tool calling 定义。 |
| `tool_choice` | string/object | 工具选择策略，常见为 `"auto"`。 |
| `response_format` | object | JSON Output 等格式约束。 |
| `stop` | string/array | 停止序列。 |
| `temperature` | number | 思考模式下不生效。 |
| `top_p` | number | 思考模式下不生效。 |
| `presence_penalty` | number | 思考模式下不生效。 |
| `frequency_penalty` | number | 思考模式下不生效。 |
| `logprobs` | boolean | 按模型/模式支持情况使用。 |
| `top_logprobs` | integer | 按模型/模式支持情况使用。 |
| `reasoning_effort` | string | V4 思考强度控制，`high` 或 `max`。 |
| `thinking` | object | V4 思考模式开关，形如 `{ "type": "enabled" }` 或 `{ "type": "disabled" }`。 |

`thinking` 是 DeepSeek 扩展字段。使用 OpenAI SDK 时，官方示例把它放入 `extra_body`；本项目使用 raw JSON 时可直接放在请求根对象。

## Thinking Control

V4 的思考模式由两个字段控制：

```json
{
  "thinking": {
    "type": "enabled"
  },
  "reasoning_effort": "high"
}
```

规则：

- `thinking.type` 可为 `enabled` 或 `disabled`。
- 默认思考开关为 `enabled`。
- 思考模式下普通请求默认 effort 为 `high`。
- 复杂 Agent 类请求可能自动使用 `max`。
- `reasoning_effort` 主要使用 `high` / `max`。
- 兼容映射：`low`、`medium` 会映射为 `high`；`xhigh` 会映射为 `max`。
- `reasoning_effort` 是请求根字段，不写入 message history。

## Messages

常规多轮消息：

```json
[
  {
    "role": "system",
    "content": "你是白杨子。"
  },
  {
    "role": "user",
    "content": "查一下今天要做什么"
  },
  {
    "role": "assistant",
    "content": "我先读取记忆。"
  }
]
```

工具调用后的上下文：

```json
[
  {
    "role": "assistant",
    "content": "",
    "reasoning_content": "我需要先读取 todo shard。",
    "tool_calls": [
      {
        "id": "call_0",
        "type": "function",
        "function": {
          "name": "recall_memory_shard",
          "arguments": "{\"shard_name\":\"todo\"}"
        }
      }
    ]
  },
  {
    "role": "tool",
    "tool_call_id": "call_0",
    "content": "{\"items\":[\"...\"]}"
  }
]
```

`reasoning_content` 是 V4 思考模式输出字段，与 `content` 同级。它不是普通回复文本，不能混入 `content`。

## Reasoning Content Rules

V4 思考模式下，DeepSeek 对 `reasoning_content` 的多轮拼接有两种规则：

| 场景 | 后续请求是否回传 `reasoning_content` |
|------|--------------------------------------|
| 两个 `user` 消息之间没有工具调用 | 不需要。即使传入也会被忽略。 |
| 两个 `user` 消息之间发生了工具调用 | 需要。该轮中间 assistant 的 `reasoning_content` 必须参与上下文拼接，并在后续 user 交互轮次继续回传。 |

工具调用场景中，assistant 消息应保留这几个字段：

```json
{
  "role": "assistant",
  "content": "",
  "reasoning_content": "模型本轮思考内容",
  "tool_calls": []
}
```

当前项目只补 `reasoning_content: ""`。这能满足字段形状，但如果 V4 Pro/Flash 在工具调用链路中依赖真实 reasoning 状态，空字符串会丢失上下文。

## Tool Definition

工具定义沿用 OpenAI Chat Completions 的 `tools` 形状：

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "recall_memory_shard",
        "description": "读取指定记忆 shard",
        "parameters": {
          "type": "object",
          "properties": {
            "shard_name": {
              "type": "string"
            }
          },
          "required": ["shard_name"],
          "additionalProperties": false
        }
      }
    }
  ],
  "tool_choice": "auto"
}
```

`strict` 模式示例：

```json
{
  "type": "function",
  "function": {
    "name": "recall_memory_shard",
    "strict": true,
    "description": "读取指定记忆 shard",
    "parameters": {
      "type": "object",
      "properties": {
        "shard_name": {
          "type": "string"
        }
      },
      "required": ["shard_name"],
      "additionalProperties": false
    }
  }
}
```

V4 的思考模式与非思考模式均可使用 `strict` 工具调用。

## Non-Streaming Response

普通文本返回：

```json
{
  "id": "chatcmpl-...",
  "object": "chat.completion",
  "created": 1710000000,
  "model": "deepseek-v4-flash",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "你好，我是白杨子。"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  },
  "system_fingerprint": "fp_..."
}
```

思考模式返回：

```json
{
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "reasoning_content": "先判断用户意图，再组织回答。",
        "content": "我会先确认任务范围。"
      },
      "finish_reason": "stop"
    }
  ]
}
```

工具调用返回：

```json
{
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "",
        "reasoning_content": "需要调用记忆工具读取 todo。",
        "tool_calls": [
          {
            "id": "call_0",
            "type": "function",
            "function": {
              "name": "recall_memory_shard",
              "arguments": "{\"shard_name\":\"todo\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ]
}
```

`finish_reason` 常见值：

| 值 | 含义 |
|----|------|
| `stop` | 正常结束。 |
| `length` | 达到上下文长度或 `max_tokens` 限制。 |
| `tool_calls` | 模型要求调用工具。 |
| `content_filter` | 输出触发过滤。 |
| `insufficient_system_resource` | DeepSeek 后端推理资源不足，请求被打断。 |

## Streaming Response

流式返回使用 SSE，每个事件形如：

```text
data: {...}
```

最后以：

```text
data: [DONE]
```

结束。

普通内容增量：

```json
{
  "id": "chatcmpl-...",
  "object": "chat.completion.chunk",
  "created": 1710000000,
  "model": "deepseek-v4-pro",
  "choices": [
    {
      "index": 0,
      "delta": {
        "role": "assistant",
        "content": "你好"
      },
      "finish_reason": null
    }
  ],
  "usage": null,
  "system_fingerprint": "fp_..."
}
```

思考内容增量：

```json
{
  "choices": [
    {
      "index": 0,
      "delta": {
        "reasoning_content": "先判断"
      },
      "finish_reason": null
    }
  ]
}
```

最终回答增量：

```json
{
  "choices": [
    {
      "index": 0,
      "delta": {
        "content": "结论是..."
      },
      "finish_reason": null
    }
  ]
}
```

工具调用增量：

```json
{
  "choices": [
    {
      "index": 0,
      "delta": {
        "tool_calls": [
          {
            "index": 0,
            "id": "call_0",
            "type": "function",
            "function": {
              "name": "recall_memory_shard",
              "arguments": "{\"shard_name\""
            }
          }
        ]
      },
      "finish_reason": null
    }
  ]
}
```

流式聚合要求：

- 追加聚合 `delta.reasoning_content`。
- 追加聚合 `delta.content`。
- 按 `tool_calls[index]` 聚合工具调用。
- 对 `tool_calls[index].function.arguments` 做字符串追加，直到 `finish_reason` 为 `tool_calls`。

## OpenAI Comparison

用户问题：“OpenAI 的接口里也有这些吗？”

结论：

| 字段 | DeepSeek V4 | OpenAI |
|------|-------------|--------|
| `reasoning_effort` | 有，V4 思考强度字段，常用 `high` / `max`。 | 有，但属于 OpenAI reasoning 模型参数体系，语义和可用位置不能直接等同 DeepSeek。 |
| `reasoning_content` | 有，assistant message / delta 字段；工具调用思考链路中可能必须回传。 | 没有 DeepSeek 这种同名 Chat Completions 协议字段。 |
| `thinking` | 有，V4 思考模式开关。 | 不是 OpenAI Chat Completions 标准字段。 |

所以本项目里这三个字段都应被视为 DeepSeek provider-specific 字段，而不是通用 OpenAI 抽象字段。

## Implementation Notes

当前项目状态：

- `src/ai/resolver_impl/openai.rs` 使用 `create_raw`，可以注入 SDK 类型没有覆盖的 DeepSeek V4 字段。
- 发送前会给 assistant 消息补 `reasoning_content: ""`。
- `build_action` 只读取 `content`、`refusal`、`tool_calls`，没有保留 `reasoning_content`。
- `src/ai/session/persist/codec.rs` 不持久化 `reasoning_content`。

对 V4 Flash / V4 Pro 的风险：

- 纯文本多轮不要求回传 `reasoning_content`，当前实现基本可用。
- 工具调用思考链路要求保留并回传真实 `reasoning_content`，当前实现会丢失该状态。
- 如果后续接入真正 OpenAI endpoint，应避免发送 DeepSeek 专属 `thinking` / `reasoning_content`。

建议：

- 在 resolver 层明确 provider/model capability：DeepSeek V4 才注入 `thinking`、`reasoning_effort`、`reasoning_content`。
- `Action` 或 provider metadata 中保留 `reasoning_content`，不要混入 `content`。
- 持久化 assistant 消息时保存 `reasoning_content`，至少覆盖工具调用链路。
