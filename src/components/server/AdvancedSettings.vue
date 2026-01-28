<template>
  <div class="advanced-toggle">
    <button @click="showAdvancedSettings = !showAdvancedSettings" class="text-button">
      {{ showAdvancedSettings ? '▲ Hide Advanced Settings' : '▼ Show Advanced Settings' }}
    </button>
  </div>
  
  <div v-if="showAdvancedSettings" class="advanced-settings" :class="{ disabled: disabled }">
    <div class="setting-group tls-section">
      <h4>🔒 Security & Connectivity</h4>
      <label>
        <input type="checkbox" v-model="useTlsUrls" :disabled="disabled" @change="handleTlsToggle" />
        Enable TLS/SSL (WSS) URLs for secure WebSocket connections
      </label>
      
      <div class="info-box" :class="{ 'active': useTlsUrls }">
        <p v-if="!useTlsUrls" class="info-text">
          ⚠️ <strong>Currently using unsecured connections (ws://)</strong><br>
          NoVNC and modern browsers require secure contexts (HTTPS/WSS) to function properly.
        </p>
        <p v-else class="info-text success">
          ✅ <strong>Secure WebSocket URLs enabled (wss://)</strong><br>
          Compatible with NoVNC and modern browsers in secure contexts.
        </p>
        
        <div class="url-examples">
          <p><strong>URL Format:</strong></p>
          <ul>
            <li v-if="!useTlsUrls">VNC WebSocket: <code>ws://hostname:5900/websockify</code></li>
            <li v-else>VNC WebSocket: <code>wss://hostname:5900/websockify</code></li>
            <li v-if="!useTlsUrls">Audio: <code>ws://hostname:6900/audio</code></li>
            <li v-else>Audio: <code>wss://hostname:6900/audio</code></li>
          </ul>
        </div>
        
        <div class="tls-requirements">
          <p><strong>⚙️ Requirements for WSS:</strong></p>
          <ul>
            <li>Reverse proxy (Nginx, Caddy, or HAProxy) with TLS termination</li>
            <li>Valid SSL certificate (Let's Encrypt recommended)</li>
            <li>Proxy must forward WebSocket connections to CLT-CLEVER-KVM</li>
          </ul>
          <p class="doc-link">
            📖 See <strong>docs/TLS_SETUP.md</strong> for detailed configuration guide
          </p>
        </div>
      </div>
    </div>
    
    <div class="setting-group">
      <h4>VNC Audio Settings</h4>
      <label>
        <input type="checkbox" v-model="settings.enableAudio" :disabled="disabled" @change="$emit('settings-changed')" />
        Enable Audio Streaming (via WebSocket on port {{ settings.audioPort }})
      </label>
      
      <div v-if="settings.enableAudio" class="slider-group">
        <label for="audio-port">Audio Port:</label>
        <input type="number" id="audio-port" v-model.number="settings.audioPort"
               min="1024" max="65535" :disabled="disabled" @change="$emit('settings-changed')" />
        <span class="help-text-inline">Shared across all monitors</span>
      </div>
    </div>
    
    <div class="setting-group">
      <h4>MediaMTX Server Configuration</h4>
      <p class="deprecation-notice">
        ℹ️ <strong>Note:</strong> MediaMTX is optional and used only for VNC stream relay in video wall deployments. 
        Audio streaming now uses direct WebSocket connections (no MediaMTX required).
      </p>
      <label>
        <input type="checkbox" v-model="settings.mediamtxAutoScan" :disabled="disabled" @change="handleAutoScanChange" />
        Auto-scan network for MediaMTX server on port 9997
      </label>
      
      <div class="mediamtx-url-group">
        <label for="mediamtx-url">MediaMTX URL:</label>
        <div class="url-input-group">
          <input 
            type="text" 
            id="mediamtx-url" 
            v-model="settings.mediamtxUrl"
            placeholder="http://192.168.1.100:9997 (auto-detected or manual)"
            :disabled="disabled" 
            @change="$emit('settings-changed')" 
          />
          <button 
            class="scan-button" 
            @click="handleManualScan" 
            :disabled="disabled || scanningMediaMtx"
            title="Manually scan for MediaMTX servers"
          >
            {{ scanningMediaMtx ? 'Scanning...' : 'Scan' }}
          </button>
        </div>
        <span class="help-text" v-if="mediamtxServers.length > 0">
          Found {{ mediamtxServers.length }} MediaMTX server(s) on the network
        </span>
        <div v-if="mediamtxServers.length > 1" class="server-list">
          <label>Available servers:</label>
          <select 
            v-model="settings.mediamtxUrl" 
            @change="$emit('settings-changed')"
            :disabled="disabled"
          >
            <option v-for="server in mediamtxServers" :key="server.ip" :value="server.url">
              {{ server.url }} ({{ server.ip }})
            </option>
          </select>
        </div>
      </div>
    </div>
    
    <div class="help-text">
      <p><strong>Architecture:</strong> VNC protocol (RFB 3.8) for video + WebSocket for audio (Opus encoded).</p>
      <p><strong>Audio:</strong> All monitors share a single audio stream on port 6900 since system audio is identical across all displays.</p>
      <p><strong>VNC Clients:</strong> Use TigerVNC, RealVNC, VNC Viewer, or NoVNC (browser-based) for connections.</p>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/tauri';

const props = defineProps({
  settings: Object,
  disabled: {
    type: Boolean,
    default: false
  },
  scanningMediaMtx: {
    type: Boolean,
    default: false
  },
  mediamtxServers: {
    type: Array,
    default: () => []
  }
});

const emit = defineEmits(['settings-changed', 'scan-mediamtx']);

const showAdvancedSettings = ref(false);
const useTlsUrls = ref(false);

onMounted(async () => {
  try {
    useTlsUrls.value = await invoke('get_use_tls_urls');
  } catch (error) {
    console.error('Failed to get TLS setting:', error);
    useTlsUrls.value = false;
  }
});

async function handleTlsToggle() {
  try {
    await invoke('set_use_tls_urls', { useTls: useTlsUrls.value });
    console.log(`TLS URLs ${useTlsUrls.value ? 'enabled' : 'disabled'}`);
  } catch (error) {
    console.error('Failed to set TLS URLs:', error);
    // Revert on error
    useTlsUrls.value = !useTlsUrls.value;
  }
}

async function handleManualScan() {
  emit('scan-mediamtx');
}

function handleAutoScanChange() {
  emit('settings-changed');
  if (props.settings.mediamtxAutoScan) {
    emit('scan-mediamtx');
  }
}
</script>

<style scoped>
.advanced-toggle {
  margin: 1rem 0;
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

.advanced-settings {
  background-color: #f8f9fa;
  border-radius: 4px;
  padding: 1rem;
  margin-bottom: 1rem;
}

.setting-group {
  margin-bottom: 0.75rem;
}

.setting-group label {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.setting-group input[type="checkbox"] {
  margin-right: 0.5rem;
}

/* TLS Section Styling */
.tls-section {
  background-color: #fff;
  border: 2px solid #3498db;
  border-radius: 6px;
  padding: 1rem;
  margin-bottom: 1.5rem;
}

.info-box {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #fff3cd;
  border-left: 4px solid #ffc107;
  border-radius: 4px;
}

.info-box.active {
  background-color: #d4edda;
  border-left-color: #28a745;
}

.info-text {
  margin: 0 0 0.75rem 0;
  font-size: 0.9rem;
  color: #856404;
}

.info-text.success {
  color: #155724;
}

.url-examples {
  margin-top: 0.75rem;
  padding: 0.75rem;
  background-color: #f8f9fa;
  border-radius: 4px;
}

.url-examples p {
  margin: 0 0 0.5rem 0;
  font-weight: 500;
  color: #495057;
}

.url-examples ul {
  margin: 0.5rem 0 0 0;
  padding-left: 1.5rem;
  list-style-type: disc;
}

.url-examples li {
  margin: 0.25rem 0;
  font-size: 0.9rem;
  color: #495057;
}

.url-examples code {
  background-color: #e9ecef;
  padding: 0.2rem 0.4rem;
  border-radius: 3px;
  font-family: 'Courier New', monospace;
  font-size: 0.85rem;
  color: #c7254e;
}

.tls-requirements {
  margin-top: 0.75rem;
  padding: 0.75rem;
  background-color: #e7f3ff;
  border-radius: 4px;
}

.tls-requirements p {
  margin: 0 0 0.5rem 0;
  font-weight: 500;
  color: #495057;
}

.tls-requirements ul {
  margin: 0.5rem 0;
  padding-left: 1.5rem;
  list-style-type: disc;
}

.tls-requirements li {
  margin: 0.25rem 0;
  font-size: 0.85rem;
  color: #495057;
}

.doc-link {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  color: #0066cc;
  font-style: italic;
}

.deprecation-notice {
  margin-bottom: 0.75rem;
  padding: 0.75rem;
  background-color: #fff3cd;
  border-left: 3px solid #ffc107;
  border-radius: 4px;
  font-size: 0.9rem;
  color: #856404;
}

.mediamtx-url-group {
  margin-top: 0.75rem;
  margin-left: 1.5rem;
}

.mediamtx-url-group > label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
}

.url-input-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.url-input-group input[type="text"] {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 0.9rem;
}

.scan-button {
  padding: 0.5rem 1rem;
  background-color: #3498db;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.9rem;
  white-space: nowrap;
}

.scan-button:hover:not(:disabled) {
  background-color: #2980b9;
}

.scan-button:disabled {
  background-color: #95a5a6;
  cursor: not-allowed;
}

.server-list {
  margin-top: 0.5rem;
}

.server-list label {
  display: block;
  margin-bottom: 0.25rem;
  font-size: 0.85rem;
}

.server-list select {
  width: 100%;
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
}

.slider-group {
  margin-bottom: 12px;
}

.slider-group label {
  display: block;
  margin-bottom: 4px;
}

.slider-group input[type="range"] {
  width: 100%;
}

.slider-group input[type="number"] {
  width: 120px;
  padding: 0.4rem;
  border: 1px solid #ddd;
  border-radius: 4px;
}

.help-text-inline {
  margin-left: 0.5rem;
  font-size: 0.85rem;
  color: #666;
  font-style: italic;
}

.help-text {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #e7f3ff;
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.help-text p {
  margin: 0.5rem 0;
  font-size: 0.9rem;
  color: #495057;
}

.help-text strong {
  color: #0066cc;
}

h4 {
  color: #2c3e50;
  font-size: 1rem;
  margin-top: 1rem;
  margin-bottom: 0.5rem;
}

.advanced-settings.disabled {
  opacity: 0.6;
  pointer-events: none;
}

input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
</style>
