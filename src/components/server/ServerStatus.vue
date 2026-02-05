<template>
  <div class="server-status">
    <div class="status-section">
      <div class="status-indicator" :class="{ active: serverStatus }"></div>
      <p class="status-text">{{ serverStatus ? 'Running' : 'Stopped' }}</p>
    </div>
    
    <div v-if="serverStatus" class="server-info">
      <div class="info-header">
        <span class="info-label">Direct Access URL</span>
        <span class="info-badge">Local Network</span>
      </div>
      <div class="url-display">
        <span class="url">{{ displayUrl }}</span>
        <button class="icon-button" @click="openUrl" title="Open in browser">Open</button>
        <button class="icon-button" @click="copyUrl" title="Copy URL">Copy</button>
      </div>
      <p class="info-hint">
        Use this URL to connect directly from devices on the same network
      </p>
    </div>

    <div class="actions">
      <button 
        v-if="!serverStatus" 
        @click="startServer" 
        :disabled="loading"
        class="primary-button"
      >
        {{ loading ? 'Starting...' : 'Start Server' }}
      </button>
      <button 
        v-else 
        @click="stopServer" 
        :disabled="loading"
        class="danger-button"
      >
        {{ loading ? 'Stopping...' : 'Stop Server' }}
      </button>
    </div>

    <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
  </div>
</template>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  serverStatus: Boolean,
  serverUrl: String,
  loading: Boolean,
  errorMessage: String,
  startServer: Function,
  stopServer: Function,
  openUrl: Function,
  copyUrl: Function
});

// Computed property to display the KVM URL
const displayUrl = computed(() => {
  if (!props.serverUrl) return '';
  
  let url = props.serverUrl;
  // Ensure the displayed URL includes /kvm
  if (!url.endsWith('/kvm')) {
    url = url.replace(/\/$/, '') + '/kvm';
  }
  return url;
});
</script>

<style scoped>
.server-status {
  max-width: 600px;
  margin: 0 auto;
}

.status-section {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 2rem;
}

.status-indicator {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background-color: #e74c3c;
  margin-right: 12px;
}

.status-indicator.active {
  background-color: #2ecc71;
}

.status-text {
  font-size: 1.2rem;
  font-weight: 500;
  margin: 0;
}

.server-info {
  margin-bottom: 1.5rem;
  padding: 1rem;
  background-color: #f8f9fa;
  border-radius: 8px;
  border: 1px solid #e9ecef;
}

.info-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.info-label {
  font-weight: 500;
  color: #2c3e50;
}

.info-badge {
  font-size: 0.75rem;
  padding: 0.2rem 0.5rem;
  background: #e3f2fd;
  color: #1976d2;
  border-radius: 4px;
}

.url-display {
  display: flex;
  align-items: center;
  background-color: #fff;
  padding: 0.5rem;
  border-radius: 4px;
  border: 1px solid #dee2e6;
}

.url {
  flex: 1;
  font-family: monospace;
  font-size: 0.9rem;
  word-break: break-all;
  color: #2c3e50;
}

.info-hint {
  font-size: 0.8rem;
  color: #6c757d;
  margin: 0.5rem 0 0 0;
}

.icon-button {
  background: #e9ecef;
  border: 1px solid #dee2e6;
  cursor: pointer;
  font-size: 0.75rem;
  margin-left: 0.5rem;
  padding: 0.35rem 0.6rem;
  border-radius: 4px;
  color: #495057;
  font-weight: 500;
}

.icon-button:hover {
  background-color: #dee2e6;
}

.actions {
  display: flex;
  justify-content: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.primary-button {
  background-color: #3498db;
  color: white;
  border: none;
  padding: 0.75rem 2rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  min-width: 140px;
}

.primary-button:hover {
  background-color: #2980b9;
}

.danger-button {
  background-color: #e74c3c;
  color: white;
  border: none;
  padding: 0.75rem 2rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  min-width: 140px;
}

.danger-button:hover {
  background-color: #c0392b;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.error {
  color: #e74c3c;
  text-align: center;
  margin: 0;
}
</style>
