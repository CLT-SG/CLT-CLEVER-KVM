<template>
  <div class="log-viewer">
    <div class="log-toggle">
      <button @click="toggleLogs" class="text-button">
        {{ showLogs ? '▲ Hide Logs' : '▼ Show Logs' }}
      </button>
      <button v-if="showLogs" @click="refreshLogs" class="text-button refresh">
        Refresh
      </button>
    </div>
    
    <div v-if="showLogs" class="logs-container">
      <div class="log-section">
        <h3>Error Log</h3>
        <pre class="log-content">{{ errorLog || 'No errors logged' }}</pre>
      </div>
      
      <div class="log-section">
        <h3>Debug Log</h3>
        <pre class="log-content">{{ debugLog || 'No debug logs available' }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';
import { invoke } from "@tauri-apps/api/tauri";

const showLogs = ref(false);
const debugLog = ref("");
const errorLog = ref("");

async function toggleLogs() {
  showLogs.value = !showLogs.value;
  if (showLogs.value) {
    await refreshLogs();
  }
}

async function refreshLogs() {
  try {
    const [debug, error] = await invoke("get_logs");
    debugLog.value = debug;
    errorLog.value = error;
  } catch (error) {
    console.error(`Failed to load logs: ${error}`);
  }
}
</script>

<style scoped>
.log-viewer {
  max-width: 800px;
  margin: 0 auto;
}

.log-toggle {
  margin-bottom: 1rem;
  display: flex;
  align-items: center;
}

.text-button {
  background: none;
  border: none;
  color: #3498db;
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0;
  text-decoration: underline;
}

.text-button.refresh {
  margin-left: 1rem;
}

.logs-container {
  margin-top: 1rem;
}

.log-section {
  margin-bottom: 1.5rem;
}

.log-content {
  background-color: #f8f9fa;
  padding: 1rem;
  border-radius: 4px;
  font-family: monospace;
  font-size: 0.8rem;
  white-space: pre-wrap;
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid #eee;
}

h3 {
  color: #2c3e50;
  font-size: 1.2rem;
  margin-top: 1.5rem;
  margin-bottom: 0.5rem;
}

h3:first-child {
  margin-top: 0;
}
</style>
