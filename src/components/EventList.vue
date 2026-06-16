<script setup lang="ts">
import { computed } from 'vue'

interface StoredEvent {
  seq: number
  raw_json: string
  received_at: string
}

const props = defineProps<{
  events: StoredEvent[]
}>()

// 事件类型配置
const eventConfig: Record<string, { icon: string; color: string; label: string }> = {
  // Gateway
  'gateway:start': { icon: '🚀', color: '#4caf50', label: '网关启动' },
  'gateway:shutdown': { icon: '⏹️', color: '#f44336', label: '网关关闭' },
  // Session
  'session:start': { icon: '💬', color: '#2196f3', label: '会话开始' },
  'session:end': { icon: '🔚', color: '#9e9e9e', label: '会话结束' },
  'session:reset': { icon: '🔄', color: '#ff9800', label: '会话重置' },
  // Agent
  'agent:start': { icon: '🤖', color: '#4caf50', label: 'Agent 开始' },
  'agent:step': { icon: '⚡', color: '#ffc107', label: 'Agent 步骤' },
  'agent:end': { icon: '✅', color: '#4caf50', label: 'Agent 结束' },
  'agent:error': { icon: '❌', color: '#f44336', label: 'Agent 错误' },
  // Message
  'message:user': { icon: '👤', color: '#2196f3', label: '用户消息' },
  'message:start': { icon: '📝', color: '#9c27b0', label: '消息开始' },
  'message:delta': { icon: '✍️', color: '#9c27b0', label: '消息流' },
  'message:complete': { icon: '📨', color: '#4caf50', label: '消息完成' },
  'message:interim': { icon: '⏳', color: '#ff9800', label: '临时消息' },
  'message:stream_end': { icon: '📩', color: '#4caf50', label: '流结束' },
  // Tool
  'tool:start': { icon: '🔧', color: '#ff9800', label: '工具开始' },
  'tool:progress': { icon: '⚙️', color: '#ffc107', label: '工具进度' },
  'tool:complete': { icon: '🔨', color: '#4caf50', label: '工具完成' },
  'tool:error': { icon: '💥', color: '#f44336', label: '工具错误' },
  'tool:generating': { icon: '🎯', color: '#9c27b0', label: '工具生成' },
  // System
  'system:status': { icon: '📊', color: '#2196f3', label: '系统状态' },
  'system:warning': { icon: '⚠️', color: '#ff9800', label: '系统警告' },
  'system:error': { icon: '🚨', color: '#f44336', label: '系统错误' },
  // Subagent
  'subagent:start': { icon: '🔄', color: '#00bcd4', label: '子Agent开始' },
  'subagent:complete': { icon: '🔁', color: '#4caf50', label: '子Agent完成' },
  // Chain
  'chain:start': { icon: '⛓️', color: '#795548', label: '链开始' },
  'chain:end': { icon: '🔗', color: '#4caf50', label: '链结束' },
  // Thinking
  'thinking:delta': { icon: '💭', color: '#e91e63', label: '思考中' },
  'reasoning:available': { icon: '🧠', color: '#9c27b0', label: '推理完成' },
  // Approval
  'approval:request': { icon: '🙋', color: '#ff9800', label: '请求审批' },
  'approval:result': { icon: '👍', color: '#4caf50', label: '审批结果' },
}

function getEventConfig(eventType: string) {
  return eventConfig[eventType] || { icon: '📌', color: '#666', label: eventType }
}

function parseEvent(json: string) {
  try {
    return JSON.parse(json)
  } catch {
    return { raw: json }
  }
}

function formatTime(iso: string) {
  try {
    return new Date(iso).toLocaleTimeString()
  } catch {
    return iso
  }
}

// 格式化事件数据，提取关键信息
function formatEventData(data: any, eventType?: string): string {
  if (!data || typeof data !== 'object') return ''

  const parts: string[] = []

  // 响应/消息文本（优先显示）
  if (data.response_text) parts.push(`回复: ${truncate(data.response_text, 120)}`)
  // 工具信息
  if (data.tool_name) parts.push(`工具: ${data.tool_name}`)
  if (data.tool) parts.push(`工具: ${data.tool}`)
  if (data.tool_input?.command) parts.push(`命令: ${truncate(data.tool_input.command, 80)}`)
  // 思考/推理内容
  if (data.text && (eventType === 'thinking:delta' || eventType === 'reasoning:available')) {
    parts.push(`思考: ${truncate(data.text, 200)}`)
  } else if (data.text) {
    parts.push(`文本: ${truncate(data.text, 80)}`)
  }
  if (data.content) parts.push(`内容: ${truncate(data.content, 80)}`)
  if (data.message) parts.push(`消息: ${truncate(data.message, 80)}`)
  // 错误
  if (data.error) parts.push(`错误: ${truncate(data.error, 60)}`)
  // 元信息
  if (data.model) parts.push(`模型: ${data.model}`)
  if (data.provider) parts.push(`提供商: ${data.provider}`)
  if (data.duration_ms) parts.push(`耗时: ${data.duration_ms}ms`)
  if (data.outcome) parts.push(`结果: ${data.outcome}`)
  if (data.user_message) parts.push(`用户: ${truncate(data.user_message, 80)}`)

  return parts.join(' · ')
}

function truncate(str: string, maxLen: number): string {
  if (!str) return ''
  return str.length > maxLen ? str.slice(0, maxLen) + '...' : str
}

const reversedEvents = computed(() => [...props.events].reverse())
</script>

<template>
  <section class="card">
    <h2>Events ({{ events.length }})</h2>
    <div v-if="events.length === 0" class="empty">
      No events yet. Start the Hub and register a hook to begin receiving events.
    </div>
    <div class="events-list">
      <div v-for="ev in reversedEvents" :key="ev.seq" class="event-item">
        <div class="event-header">
          <span class="event-icon">{{ getEventConfig(parseEvent(ev.raw_json).event).icon }}</span>
          <span class="event-seq">#{{ ev.seq }}</span>
          <span class="event-time">{{ formatTime(ev.received_at) }}</span>
          <span class="event-badge" :style="{ color: getEventConfig(parseEvent(ev.raw_json).event).color, borderColor: getEventConfig(parseEvent(ev.raw_json).event).color }">
            {{ getEventConfig(parseEvent(ev.raw_json).event).label }}
          </span>
          <span class="event-framework">{{ parseEvent(ev.raw_json).framework || '' }}</span>
        </div>
        <div class="event-summary" v-if="formatEventData(parseEvent(ev.raw_json).data, parseEvent(ev.raw_json).event)">
          {{ formatEventData(parseEvent(ev.raw_json).data, parseEvent(ev.raw_json).event) }}
        </div>
        <details class="event-details">
          <summary>详细数据</summary>
          <pre class="event-data">{{ JSON.stringify(parseEvent(ev.raw_json).data, null, 2) }}</pre>
        </details>
      </div>
    </div>
  </section>
</template>

<style scoped>
.card {
  background: #16213e;
  border-radius: 12px;
  padding: 20px;
  margin-bottom: 16px;
}
.card h2 { font-size: 18px; margin-bottom: 12px; color: #00d4ff; }

.empty { color: #666; font-style: italic; padding: 16px 0; }

.events-list { max-height: 500px; overflow-y: auto; }
.event-item {
  background: #0a0a1a;
  border-radius: 8px;
  padding: 12px 16px;
  margin-top: 8px;
  font-size: 13px;
  border-left: 3px solid #333;
  transition: border-color 0.2s;
}
.event-item:hover { border-left-color: #00d4ff; }

.event-header {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.event-icon { font-size: 16px; }
.event-seq { color: #666; font-family: monospace; font-size: 12px; min-width: 35px; }
.event-time { color: #888; font-family: monospace; font-size: 11px; }
.event-badge {
  font-size: 12px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
  border: 1px solid;
  background: rgba(255, 255, 255, 0.05);
}
.event-framework { color: #666; font-size: 11px; margin-left: auto; }

.event-summary {
  margin-top: 8px;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 4px;
  color: #aaa;
  font-size: 12px;
  line-height: 1.4;
}

.event-details {
  margin-top: 8px;
  font-size: 12px;
}
.event-details summary {
  color: #666;
  cursor: pointer;
  user-select: none;
  font-size: 11px;
}
.event-details summary:hover { color: #888; }
.event-data {
  margin-top: 6px;
  padding: 8px;
  background: #050510;
  border-radius: 4px;
  color: #aaa;
  font-size: 11px;
  font-family: 'Monaco', 'Menlo', monospace;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 200px;
  overflow-y: auto;
}
</style>
