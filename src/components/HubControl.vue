<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface HubStatus {
  running: boolean
  connected_clients: number
  stored_events: number
  current_seq: number
}

const props = defineProps<{
  status: HubStatus
  wsUrl: string
  loading: boolean
}>()

const emit = defineEmits<{
  start: []
  stop: []
  refresh: []
}>()

async function startHub() {
  emit('start')
}

async function stopHub() {
  emit('stop')
}

async function refresh() {
  emit('refresh')
}
</script>

<template>
  <section class="card">
    <h2>Hub Server</h2>
    <div class="hub-controls">
      <div class="hub-info">
        <span :class="['dot', status.running ? 'dot-on' : 'dot-off']"></span>
        <span>{{ status.running ? 'Running' : 'Stopped' }}</span>
        <span v-if="status.running" class="hub-details">
          · {{ status.connected_clients }} clients · {{ status.stored_events }} events · seq {{ status.current_seq }}
        </span>
      </div>
      <div class="hub-actions">
        <button @click="startHub" :disabled="loading || status.running" class="btn btn-primary">
          ▶ Start
        </button>
        <button @click="stopHub" :disabled="loading || !status.running" class="btn btn-danger">
          ■ Stop
        </button>
        <button @click="refresh" :disabled="loading" class="btn">
          ↻ Refresh
        </button>
      </div>
    </div>
    <div v-if="wsUrl" class="hub-url">
      <span class="label">WS URL:</span>
      <code>{{ wsUrl }}</code>
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

.hub-controls { display: flex; justify-content: space-between; align-items: center; }
.hub-info { display: flex; align-items: center; gap: 8px; }
.hub-details { color: #888; font-size: 13px; }
.hub-actions { display: flex; gap: 8px; }
.hub-url { margin-top: 12px; font-size: 13px; }
.hub-url code { color: #00d4ff; background: #0a0a1a; padding: 2px 8px; border-radius: 4px; }
.label { color: #888; margin-right: 8px; }

.dot {
  width: 10px; height: 10px; border-radius: 50%; display: inline-block;
}
.dot-on { background: #4caf50; box-shadow: 0 0 6px #4caf50; }
.dot-off { background: #666; }

.btn {
  padding: 8px 16px;
  border: 1px solid #333;
  border-radius: 8px;
  background: #1a1a2e;
  color: #e0e0e0;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.2s;
}
.btn:hover:not(:disabled) { background: #2a2a4e; border-color: #00d4ff; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-primary { background: #0a2a3d; border-color: #2196f3; color: #64b5f6; }
.btn-primary:hover:not(:disabled) { background: #1a3a5d; }
.btn-danger { background: #3d0a0a; border-color: #f44336; color: #ef9a9a; }
.btn-danger:hover:not(:disabled) { background: #5d1a1a; }
</style>
