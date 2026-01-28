<template>
  <div class="server-status">
    <div class="status-section">
      <div class="status-indicator" :class="{ active: serverStatus }"></div>
      <p class="status-text">{{ serverStatus ? 'VNC Server Running' : 'VNC Server Stopped' }}</p>
    </div>
    
    <div v-if="serverStatus && vncInfo" class="server-info">
      <h4>VNC Connection Information</h4>
      <div class="info-item">
        <span class="label">VNC URL:</span>
        <div class="url-display">
          <code class="url">{{ vncInfo.vnc_url }}</code>
          <button class="copy-button" @click="copyUrl" title="Copy VNC URL">Copy</button>
        </div>
      </div>
      
      <div v-if="vncInfo.websockify_url" class="info-item">
        <span class="label">WebSocket URL:</span>
        <div class="url-display">
          <code class="url">{{ vncInfo.websockify_url }}</code>
          <button class="copy-button" @click="copyWebsockifyUrl" title="Copy WebSocket URL">Copy</button>
        </div>
        <span class="url-note">For NoVNC browser-based clients</span>
      </div>
      
      <div v-if="vncInfo.audio_url" class="info-item">
        <span class="label">Audio URL:</span>
        <div class="url-display">
          <code class="url">{{ vncInfo.audio_url }}</code>
          <button class="copy-button" @click="copyAudioUrl" title="Copy Audio URL">Copy</button>
        </div>
        <span class="url-note">Shared WebSocket audio stream (Opus encoded)</span>
      </div>
      
      <div class="info-item">
        <span class="label">Monitor:</span>
        <span class="value">
          {{ vncInfo.monitor_name || `Monitor ${vncInfo.monitor_id || 0}` }}
          <span v-if="vncInfo.width && vncInfo.height"> ({{ vncInfo.width }}x{{ vncInfo.height }})</span>
        </span>
      </div>
      
      <div class="info-item">
        <span class="label">Port:</span>
        <span class="value">{{ vncInfo.port }}</span>
      </div>
      
      <div v-if="vncInfo.clients_connected !== undefined" class="info-item">
        <span class="label">Connected Clients:</span>
        <span class="value">{{ vncInfo.clients_connected }}</span>
      </div>
      
      <div class="connection-help">
        <p><strong>How to connect:</strong></p>
        <ul>
          <li><strong>Desktop VNC clients:</strong> Use the VNC URL with TigerVNC, RealVNC, etc.</li>
          <li v-if="vncInfo.websockify_url"><strong>Browser (NoVNC):</strong> Use the WebSocket URL</li>
          <li v-if="vncInfo.audio_url"><strong>Audio:</strong> Separate WebSocket stream on port 6900</li>
        </ul>
      </div>
    </div>

    <div class="actions">
      <button 
        v-if="!serverStatus" 
        @click="startServer" 
        :disabled="loading"
        class="primary-button"
      >
        {{ loading ? 'Starting...' : 'Start VNC Server' }}
      </button>
      <button 
        v-else 
        @click="stopServer" 
        :disabled="loading"
        class="danger-button"
      >
        {{ loading ? 'Stopping...' : 'Stop VNC Server' }}
      </button>
    </div>

    <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
  </div>
</template>

<script setup>
const props = defineProps({
  serverStatus: Boolean,
  serverUrl: String,
  vncInfo: Object,
  loading: Boolean,
  errorMessage: String,
  startServer: Function,
  stopServer: Function,
  copyUrl: Function
});

function copyAudioUrl() {
  if (props.vncInfo && props.vncInfo.audio_url) {
    navigator.clipboard.writeText(props.vncInfo.audio_url);
  }
}

function copyWebsockifyUrl() {
  if (props.vncInfo && props.vncInfo.websockify_url) {
    navigator.clipboard.writeText(props.vncInfo.websockify_url);
  }
}
</script>

<style scoped>
.server-status {
  max-width: 700px;
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
  margin-bottom: 2rem;
  padding: 1.5rem;
  background-color: #f8f9fa;
  border-radius: 8px;
  border: 1px solid #dee2e6;
}

.server-info h4 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: #2c3e50;
  font-size: 1.1rem;
}

.info-item {
  margin-bottom: 1rem;
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
}

.info-item .label {
  font-weight: 600;
  min-width: 150px;
  color: #495057;
}

.info-item .value {
  color: #212529;
}

.url-display {
  display: flex;
  align-items: center;
  background-color: #ffffff;
  padding: 0.5rem;
  border-radius: 4px;
  border: 1px solid #ced4da;
  flex: 1;
}

.url-note {
  display: block;
  margin-top: 0.25rem;
  margin-left: 150px;
  font-size: 0.8rem;
  color: #6c757d;
  font-style: italic;
}

.url {
  flex: 1;
  font-family: 'Courier New', monospace;
  font-size: 0.9rem;
  word-break: break-all;
  background-color: #ffffff;
  padding: 0.25rem;
}

.copy-button {
  background-color: #007bff;
  color: white;
  border: none;
  cursor: pointer;
  font-size: 0.85rem;
  font-weight: 500;
  margin-left: 0.5rem;
  padding: 0.4rem 0.75rem;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.copy-button:hover {
  background-color: #0056b3;
}

.connection-help {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #e7f3ff;
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.connection-help p {
  margin: 0.5rem 0;
  font-size: 0.9rem;
}

.connection-help ul {
  margin: 0.5rem 0;
  padding-left: 1.5rem;
  font-size: 0.9rem;
}

.connection-help li {
  margin: 0.5rem 0;
}

.actions {
  display: flex;
  justify-content: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.primary-button {
  background-color: #28a745;
  color: white;
  border: none;
  padding: 0.75rem 2rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  min-width: 180px;
  font-weight: 600;
}

.primary-button:hover:not(:disabled) {
  background-color: #218838;
}

.danger-button {
  background-color: #dc3545;
  color: white;
  border: none;
  padding: 0.75rem 2rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  min-width: 180px;
  font-weight: 600;
}

.danger-button:hover:not(:disabled) {
  background-color: #c82333;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.error {
  color: #dc3545;
  text-align: center;
  margin: 1rem 0;
  padding: 1rem;
  background-color: #f8d7da;
  border: 1px solid #f5c6cb;
  border-radius: 4px;
}
</style>
