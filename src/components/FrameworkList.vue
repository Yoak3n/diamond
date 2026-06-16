<script setup lang="ts">
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

const props = defineProps<{
  frameworks: DetectedFramework[]
  loading: boolean
}>()

const emit = defineEmits<{
  detect: []
  register: [frameworkId: string]
  unregister: [frameworkId: string]
}>()

function detectFrameworks() {
  emit('detect')
}

function registerHook(frameworkId: string) {
  emit('register', frameworkId)
}

function unregisterHook(frameworkId: string) {
  emit('unregister', frameworkId)
}
</script>

<template>
  <section class="card">
    <h2>Detected Frameworks</h2>
    <button @click="detectFrameworks" :disabled="loading" class="btn btn-small">
      ↻ Scan
    </button>

    <div v-if="frameworks.length === 0" class="empty">
      No frameworks detected. Install Hermes, OpenClaw, Claude Code, or Codex CLI.
    </div>

    <div v-for="fw in frameworks" :key="fw.id" class="framework-item">
      <div class="fw-header">
        <span class="fw-name">{{ fw.name }}</span>
        <span v-if="fw.version" class="fw-version">v{{ fw.version }}</span>
        <span :class="['dot', fw.running ? 'dot-on' : 'dot-off']"></span>
        <span v-if="fw.hook_registered" class="badge badge-green">Hooked</span>
      </div>
      <div class="fw-path">{{ fw.install_path }}</div>
      <div class="fw-actions">
        <button
          v-if="!fw.hook_registered"
          @click="registerHook(fw.id)"
          :disabled="loading"
          class="btn btn-primary btn-small"
        >
          🔗 Register Hook
        </button>
        <button
          v-else
          @click="unregisterHook(fw.id)"
          :disabled="loading"
          class="btn btn-danger btn-small"
        >
          ✕ Remove Hook
        </button>
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

.dot {
  width: 10px; height: 10px; border-radius: 50%; display: inline-block;
}
.dot-on { background: #4caf50; box-shadow: 0 0 6px #4caf50; }
.dot-off { background: #666; }

.badge {
  font-size: 11px; padding: 2px 8px; border-radius: 10px; font-weight: 600;
}
.badge-green { background: #0a3d0a; color: #4caf50; }

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
.btn-small { padding: 4px 12px; font-size: 12px; }

.framework-item {
  background: #0a0a1a;
  border-radius: 8px;
  padding: 12px 16px;
  margin-top: 8px;
}
.fw-header { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
.fw-name { font-weight: 600; }
.fw-version { color: #888; font-size: 12px; }
.fw-path { color: #666; font-size: 12px; margin-bottom: 8px; word-break: break-all; }
.fw-actions { display: flex; gap: 8px; }
</style>
