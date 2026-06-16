# Agent Hook Hub - WebSocket Protocol

## 概述

Agent Hook Hub 是一个 WebSocket 服务器，用于接收、存储和广播来自各种 AI Agent 框架的事件。其他项目可以通过 WebSocket 连接到 Hub 来实时接收事件流。

## 连接方式

### WebSocket 端点

| 端点 | 用途 | 说明 |
|------|------|------|
| `ws://host:9210/hook` | Agent 接入 | Agent 框架连接此端点发送事件 |
| `ws://host:9210/view` | Viewer 接入 | 监控/展示工具连接此端点接收事件广播 |

### 默认地址

```
ws://127.0.0.1:9210/hook
ws://127.0.0.1:9210/view
```

---

## 事件消息格式

### 基础结构

所有事件消息都是 JSON 格式，包含以下基础字段：

```json
{
  "event": "string",        // 事件类型（必填）
  "framework": "string",    // 来源框架标识（必填）
  "session_id": "string",   // 会话 ID（必填）
  "turn_id": "string",      // 轮次 ID（可选）
  "timestamp": "ISO-8601",  // 时间戳（必填）
  // ... 事件特定的数据字段
}
```

### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `event` | string | ✅ | 事件类型，使用 `category:action` 格式 |
| `framework` | string | ✅ | 来源框架标识，如 `hermes`, `langchain`, `claude-code` |
| `session_id` | string | ✅ | 会话/对话 ID，用于关联同一会话的事件 |
| `turn_id` | string | ❌ | 轮次 ID，用于关联同一用户输入的事件 |
| `timestamp` | string | ✅ | ISO-8601 格式的时间戳 |

---

## 事件类型

### 1. Gateway 生命周期

#### `gateway:start`
网关启动事件。

```json
{
  "event": "gateway:start",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### `gateway:shutdown`
网关关闭事件。

---

### 2. Session 生命周期

#### `session:start`
会话开始事件。

```json
{
  "event": "session:start",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### `session:end`
会话结束事件。

#### `session:reset`
会话重置事件。

---

### 3. Agent 生命周期

#### `agent:start`
Agent 开始处理请求。

```json
{
  "event": "agent:start",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### `agent:step`
Agent 执行步骤（每次迭代）。

| 字段 | 类型 | 说明 |
|------|------|------|
| `iteration` | number | 当前迭代次数 |
| `prev_tools` | array | 之前使用的工具列表 |

```json
{
  "event": "agent:step",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "iteration": 2,
  "prev_tools": ["terminal", "search"]
}
```

#### `agent:end`
Agent 处理完成。

| 字段 | 类型 | 说明 |
|------|------|------|
| `response` | string | Agent 的最终响应（可选） |

#### `agent:error`
Agent 处理出错。

| 字段 | 类型 | 说明 |
|------|------|------|
| `error` | string | 错误信息 |

---

### 4. Message 消息流

#### `message:user`
用户发送的消息。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | string | 用户消息文本 |

```json
{
  "event": "message:user",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "text": "请帮我分析这段代码"
}
```

#### `message:start`
消息开始生成。

#### `message:delta`
消息流式输出（逐 token）。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | string | 增量文本内容 |

```json
{
  "event": "message:delta",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "text": "好的，"
}
```

#### `message:complete`
消息生成完成。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | string | 完整的消息文本 |

#### `message:interim`
临时/中间消息。

#### `message:stream_end`
消息流结束。

---

### 5. Thinking 思考/推理

#### `thinking:delta`
思考过程流式输出。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | string | 思考内容增量 |

```json
{
  "event": "thinking:delta",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "text": "让我分析一下这个问题..."
}
```

#### `reasoning:available`
推理结果可用。

---

### 6. Tool 工具执行

#### `tool:start`
工具开始执行。

| 字段 | 类型 | 说明 |
|------|------|------|
| `tool_name` | string | 工具名称 |
| `tool_input` | object | 工具输入参数 |

```json
{
  "event": "tool:start",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "tool_name": "terminal",
  "tool_input": {
    "command": "ls -la"
  }
}
```

#### `tool:progress`
工具执行进度。

#### `tool:complete`
工具执行完成。

| 字段 | 类型 | 说明 |
|------|------|------|
| `tool_name` | string | 工具名称 |
| `tool_input` | object | 工具输入参数 |
| `tool_response` | object/string | 工具输出结果 |
| `tool_call_id` | string | 工具调用 ID |
| `duration_ms` | number | 执行耗时（毫秒） |

```json
{
  "event": "tool:complete",
  "framework": "hermes",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "tool_name": "terminal",
  "tool_input": {
    "command": "ls -la"
  },
  "tool_response": "total 0\ndrwxr-xr-x  2 user staff  64 Jan  1 00:00 .",
  "tool_call_id": "call_abc123",
  "duration_ms": 150
}
```

#### `tool:error`
工具执行出错。

| 字段 | 类型 | 说明 |
|------|------|------|
| `tool_name` | string | 工具名称 |
| `error` | string | 错误信息 |

#### `tool:generating`
工具正在生成输出。

#### `tool:guardrail_halt`
工具被安全防护拦截。

---

### 7. Approval 审批

#### `approval:request`
请求用户审批。

| 字段 | 类型 | 说明 |
|------|------|------|
| `tool_name` | string | 需要审批的工具 |
| `tool_input` | object | 工具参数 |
| `message` | string | 审批提示信息 |

```json
{
  "event": "approval:request",
  "framework": "claude-code",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "tool_name": "terminal",
  "tool_input": {
    "command": "rm -rf /tmp/test"
  },
  "message": "此操作将删除文件，是否允许？"
}
```

#### `approval:result`
审批结果。

| 字段 | 类型 | 说明 |
|------|------|------|
| `approved` | boolean | 是否批准 |
| `reason` | string | 拒绝原因（可选） |

---

### 8. System 系统

#### `system:status`
系统状态更新。

| 字段 | 类型 | 说明 |
|------|------|------|
| `message` | string | 状态信息 |

#### `system:warning`
系统警告。

#### `system:error`
系统错误。

#### `system:compression`
上下文压缩事件。

---

### 9. Sub-agent 子代理

#### `subagent:start`
子代理启动。

#### `subagent:progress`
子代理进度更新。

#### `subagent:complete`
子代理完成。

#### `subagent:tool`
子代理工具调用。

#### `subagent:thinking`
子代理思考过程。

---

### 10. Chain 链 (LangChain)

#### `chain:start`
链开始执行。

#### `chain:end`
链执行完成。

---

### 11. Memory & Skill 记忆与技能

#### `memory:saved`
记忆保存。

#### `memory:loaded`
记忆加载。

#### `skill:loaded`
技能加载。

#### `skill:saved`
技能保存。

---

### 12. Cron & Background 定时与后台

#### `cron:job_start`
定时任务开始。

#### `cron:job_end`
定时任务结束。

#### `background:started`
后台任务开始。

#### `background:finished`
后台任务完成。

---

### 13. 自定义事件

#### `custom:<name>`
框架特定的自定义事件。

```json
{
  "event": "custom:my_special_event",
  "framework": "my-framework",
  "session_id": "sess_123",
  "timestamp": "2024-01-01T00:00:00Z",
  "custom_field": "value"
}
```

---

## Hub 管理消息

### 连接握手

Agent 连接后应首先发送身份信息：

```json
{
  "framework": "hermes",
  "session_id": "sess_123"
}
```

Hub 响应：

```json
{
  "type": "hub:connected",
  "session_id": "agent_xxx",
  "seq": 42
}
```

### 心跳

Hub 会定期发送 ping：

```json
{
  "event": "hub:ping"
}
```

客户端应响应：

```json
{
  "event": "hub:pong",
  "framework": "hermes"
}
```

### 重播请求

Viewer 连接后可以请求重播历史事件：

```json
{
  "cmd": "replay",
  "after_seq": 100
}
```

---

## REST API

除了 WebSocket，Hub 还提供 REST API：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/status` | GET | 获取 Hub 状态 |
| `/api/clients` | GET | 获取连接的客户端列表 |
| `/api/events?limit=N&after_seq=N` | GET | 查询事件 |
| `/api/events/latest?n=N` | GET | 获取最新 N 条事件 |
| `/api/emit` | POST | 通过 HTTP 发送事件（WebSocket 的降级方案） |

### 状态响应示例

```json
{
  "running": true,
  "connected_clients": 3,
  "stored_events": 150,
  "current_seq": 420
}
```

---

## 框架映射

不同框架的事件名称会被自动映射到统一格式：

### Claude Code

| 原始事件 | 统一事件 |
|----------|----------|
| `SessionStart` | `session:start` |
| `SessionEnd` | `session:end` |
| `PreToolUse` | `tool:start` |
| `PostToolUse` | `tool:complete` |
| `Stop` | `agent:end` |
| `StopFailure` | `agent:error` |
| `PreCompact` | `system:compression` |
| `SubagentStop` | `subagent:complete` |
| `UserPromptSubmit` | `message:user` |
| `PermissionRequest` | `approval:request` |

### Hermes

| 原始事件 | 统一事件 |
|----------|----------|
| `assistant:response` | `message:complete` |
| `assistant:interim` | `message:interim` |
| `thinking:output` | `thinking:delta` |
| `reasoning:output` | `reasoning:available` |

### Codex

| 原始事件 | 统一事件 |
|----------|----------|
| `SessionStart` | `session:start` |
| `SessionEnd` | `session:end` |
| `PreToolUse` | `tool:start` |
| `PostToolUse` | `tool:complete` |
| `Stop` | `agent:end` |

### 通用适配器

支持 `hook:`、`callback:`、`handle:` 前缀自动转换：

- `hook:tool_start` → `tool:start`
- `callback:message_delta` → `message:delta`
- `handle:agent_step` → `agent:step`

---

## 示例代码

### JavaScript/TypeScript

```typescript
const ws = new WebSocket('ws://127.0.0.1:9210/view');

ws.onopen = () => {
  console.log('Connected to Hub');
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  
  if (data.event === 'hub:ping') {
    ws.send(JSON.stringify({ event: 'hub:pong', framework: 'my-viewer' }));
    return;
  }
  
  console.log(`[${data.event}] ${data.framework}:`, data);
};

// 处理特定事件
function handleEvent(data: any) {
  switch (data.event) {
    case 'tool:start':
      console.log(`Tool started: ${data.tool_name}`);
      break;
    case 'tool:complete':
      console.log(`Tool completed: ${data.tool_name} (${data.duration_ms}ms)`);
      break;
    case 'message:delta':
      process.stdout.write(data.text);
      break;
    case 'agent:end':
      console.log('Agent finished');
      break;
  }
}
```

### Python

```python
import asyncio
import websockets
import json

async def connect_to_hub():
    async with websockets.connect('ws://127.0.0.1:9210/view') as ws:
        print('Connected to Hub')
        
        async for message in ws:
            data = json.loads(message)
            
            # 响应心跳
            if data.get('event') == 'hub:ping':
                await ws.send(json.dumps({
                    'event': 'hub:pong',
                    'framework': 'my-python-viewer'
                }))
                continue
            
            print(f"[{data['event']}] {data['framework']}: {data}")

asyncio.run(connect_to_hub())
```

### Rust

```rust
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let (mut ws, _) = connect_async("ws://127.0.0.1:9210/view").await.unwrap();
    
    while let Some(msg) = ws.next().await {
        if let Ok(text) = msg.unwrap().into_text() {
            let data: Value = serde_json::from_str(&text).unwrap();
            
            // 响应心跳
            if data["event"] == "hub:ping" {
                let pong = serde_json::json!({
                    "event": "hub:pong",
                    "framework": "rust-viewer"
                });
                ws.send(tokio_tungstenite::tungstenite::Message::Text(pong.to_string())).await.unwrap();
                continue;
            }
            
            println!("[{}] {}: {:?}", data["event"], data["framework"], data);
        }
    }
}
```

---

## 注意事项

1. **事件顺序**：事件按 `seq` 字段排序，确保全局有序
2. **重连机制**：建议实现指数退避重连
3. **历史重播**：Viewer 可使用 `after_seq` 参数获取错过的事件
4. **数据持久化**：当前为内存存储，重启后事件丢失
5. **并发限制**：单个 Hub 实例建议不超过 100 个并发连接
