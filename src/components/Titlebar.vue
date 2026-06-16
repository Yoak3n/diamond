<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'

async function minimizeWindow() {
  await invoke('window_minimize')
}

async function toggleMaximize() {
  await invoke('window_toggle_maximize')
}

async function closeWindow() {
  await invoke('window_close')
}
</script>

<template>
  <div class="titlebar">
    <div class="titlebar-drag" data-tauri-drag-region>
      <span class="titlebar-icon">💎</span>
      <span class="titlebar-title">Diamond</span>
    </div>
    <div class="titlebar-controls">
      <button class="titlebar-btn" @click="minimizeWindow" title="最小化">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <rect x="1" y="5.5" width="10" height="1" fill="currentColor"/>
        </svg>
      </button>
      <button class="titlebar-btn" @click="toggleMaximize" title="最大化">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <rect x="1.5" y="1.5" width="9" height="9" stroke="currentColor" stroke-width="1" fill="none"/>
        </svg>
      </button>
      <button class="titlebar-btn titlebar-btn-close" @click="closeWindow" title="关闭">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path d="M1 1L11 11M11 1L1 11" stroke="currentColor" stroke-width="1.5"/>
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 32px;
  width: 100%;
  background: #0f0f23;
  user-select: none;
  -webkit-user-select: none;
}

.titlebar-drag {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 100%;
  padding-left: 12px;
}

.titlebar-icon {
  font-size: 14px;
}

.titlebar-title {
  font-size: 12px;
  color: #888;
  font-weight: 500;
}

.titlebar-controls {
  display: flex;
  gap: 0;
}

.titlebar-btn {
  width: 46px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #999;
  cursor: pointer;
  transition: all 0.15s;
}

.titlebar-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.titlebar-btn-close:hover {
  background: #e81123;
  color: white;
}
</style>
