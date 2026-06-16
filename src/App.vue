<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Titlebar, HubControl, FrameworkList, EventList, StatusBar } from './components'

interface DetectedFramework {
  id: string
  name: string
  install_path: string
  config_path: string | null
  running: boolean
  version: string | null
  python_path: string | null
  hook_registered: boolean
}

interface HubStatus {
  running: boolean
  connected_clients: number
  stored_events: number
  current_seq: number
}

interface StoredEvent {
  seq: number
  raw_json: string
  received_at: string
}

interface RegistrationResult {
  framework_id: string
  success: boolean
  message: string
  hook_path: string | null
}

// State
const frameworks = ref<DetectedFramework[]>([])
const hubStatus = ref<HubStatus>({ running: false, connected_clients: 0, stored_events: 0, current_seq: 0 })
const events = ref<StoredEvent[]>([])
const hubWsUrl = ref('')
const loading = ref(false)
const statusMessage = ref('')
const statusType = ref<'success' | 'error' | 'info'>('info')

// Auto-refresh interval
let refreshInterval: number | null = null

// ── Hub Controls ──

async function startHub() {
  loading.value = true
  try {
    const result = await invoke<string>('hub_start')
    showStatus(result, 'success')
    await refreshAll()
  } catch (e) {
    showStatus(String(e), 'error')
  } finally {
    loading.value = false
  }
}

async function stopHub() {
  loading.value = true
  try {
    const result = await invoke<string>('hub_stop')
    showStatus(result, 'success')
    await refreshAll()
  } catch (e) {
    showStatus(String(e), 'error')
  } finally {
    loading.value = false
  }
}

// ── Framework Detection ──

async function detectFrameworks() {
  loading.value = true
  try {
    frameworks.value = await invoke<DetectedFramework[]>('detect_frameworks')
    showStatus(`Found ${frameworks.value.length} framework(s)`, 'info')
  } catch (e) {
    showStatus(String(e), 'error')
  } finally {
    loading.value = false
  }
}

// ── Hook Registration ──

async function registerHook(frameworkId: string) {
  loading.value = true
  try {
    const result = await invoke<RegistrationResult>('register_hook', {
      frameworkId,
      hubUrl: hubWsUrl.value || 'ws://127.0.0.1:9210/hook'
    })
    showStatus(result.message, result.success ? 'success' : 'error')
    await detectFrameworks()
  } catch (e) {
    showStatus(String(e), 'error')
  } finally {
    loading.value = false
  }
}

async function unregisterHook(frameworkId: string) {
  loading.value = true
  try {
    const result = await invoke<RegistrationResult>('unregister_hook', { frameworkId })
    showStatus(result.message, result.success ? 'success' : 'error')
    await detectFrameworks()
  } catch (e) {
    showStatus(String(e), 'error')
  } finally {
    loading.value = false
  }
}

// ── Data Refresh ──

async function refreshStatus() {
  try {
    hubStatus.value = await invoke<HubStatus>('hub_status')
    hubWsUrl.value = await invoke<string>('hub_ws_url')
  } catch (e) {
    console.error('Failed to refresh status:', e)
  }
}

async function refreshEvents() {
  try {
    events.value = await invoke<StoredEvent[]>('hub_events', { limit: 100, afterSeq: null })
  } catch (e) {
    console.error('Failed to refresh events:', e)
  }
}

async function refreshAll() {
  await Promise.all([refreshStatus(), refreshEvents()])
}

function showStatus(msg: string, type: 'success' | 'error' | 'info') {
  statusMessage.value = msg
  statusType.value = type
  setTimeout(() => { statusMessage.value = '' }, 5000)
}

onMounted(async () => {
  await refreshAll()
  await detectFrameworks()
  refreshInterval = window.setInterval(refreshAll, 3000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})
</script>

<template>
  <Titlebar />
  <div class="app">
    <StatusBar :message="statusMessage" :type="statusType" />
    <HubControl
      :status="hubStatus"
      :wsUrl="hubWsUrl"
      :loading="loading"
      @start="startHub"
      @stop="stopHub"
      @refresh="refreshAll"
    />
    <FrameworkList
      :frameworks="frameworks"
      :loading="loading"
      @detect="detectFrameworks"
      @register="registerHook"
      @unregister="unregisterHook"
    />
    <EventList :events="events" />
  </div>
</template>

<style>
/* Global styles - fill entire window */
html, body {
  margin: 0;
  padding: 0;
  background: #1a1a2e;
  overflow-x: hidden;
}

/* Custom scrollbar */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}

/* Firefox scrollbar */
* {
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.15) rgba(0, 0, 0, 0.2);
}
</style>

<style scoped>
* { box-sizing: border-box; margin: 0; padding: 0; }

.app {
  width: 100%;
  max-width: 900px;
  margin: 0 auto;
  padding: 16px 24px;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  color: #e0e0e0;
  min-height: 100vh;
}
</style>
