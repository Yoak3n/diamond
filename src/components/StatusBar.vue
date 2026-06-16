<script setup lang="ts">
const props = defineProps<{
  message: string
  type: 'success' | 'error' | 'info'
}>()
</script>

<template>
  <div v-if="message" :class="['status-bar', type]">
    {{ message }}
  </div>
</template>

<style scoped>
.status-bar {
  padding: 10px 16px;
  border-radius: 8px;
  margin-bottom: 16px;
  font-size: 14px;
}
.status-bar.success { background: #0a3d0a; color: #4caf50; }
.status-bar.error { background: #3d0a0a; color: #f44336; }
.status-bar.info { background: #0a2a3d; color: #2196f3; }
</style>
