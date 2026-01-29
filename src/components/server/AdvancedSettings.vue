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
            <li v-if="!useTlsUrls">VNC WebSocket: <code>ws://hostname:6080/</code></li>
            <li v-else>VNC WebSocket: <code>wss://hostname:6080/</code></li>
            <li v-if="!useTlsUrls">Audio: <code>ws://hostname:6900/audio</code></li>
            <li v-else>Audio: <code>wss://hostname:6900/audio</code></li>
          </ul>
          <p class="url-note">Note: Websockify runs on port 6080+, not the VNC port 5900+</p>
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
      <h4>🖥️ VNC Server Configuration</h4>
      <div class="config-grid">
        <div class="config-item">
          <label for="vnc-base-port">Base Port:</label>
          <input type="number" id="vnc-base-port" v-model.number="vncConfig.base_port"
                 min="1024" max="65500" :disabled="disabled" @change="saveVncConfig" />
          <span class="help-text-inline">Starting port for VNC servers (Monitor N = Port + N)</span>
        </div>
        
        <div class="config-item">
          <label for="vnc-quality">Quality:</label>
          <select id="vnc-quality" v-model="vncConfig.quality" :disabled="disabled" @change="saveVncConfig">
            <option value="low">Low (Higher compression, lower quality)</option>
            <option value="medium">Medium (Balanced)</option>
            <option value="high">High (Best quality, higher bandwidth)</option>
          </select>
        </div>
        
        <div class="config-item">
          <label for="vnc-fps">Frame Rate Limit:</label>
          <div class="slider-container">
            <input type="range" id="vnc-fps" v-model.number="vncConfig.frame_rate_limit"
                   min="15" max="60" :disabled="disabled" @change="saveVncConfig" />
            <span class="value-display">{{ vncConfig.frame_rate_limit }} FPS</span>
          </div>
          <span class="help-text-inline">Lower values reduce bandwidth usage</span>
        </div>
        
        <div class="config-item checkbox-item">
          <label>
            <input type="checkbox" v-model="vncConfig.cursor_encoding" :disabled="disabled" @change="saveVncConfig" />
            Enable Cursor Encoding
          </label>
          <span class="help-text-inline">Send cursor shape to client for better performance</span>
        </div>
        
        <div class="config-item checkbox-item">
          <label>
            <input type="checkbox" v-model="vncConfig.desktop_resize" :disabled="disabled" @change="saveVncConfig" />
            Allow Desktop Resize
          </label>
          <span class="help-text-inline">Let clients request resolution changes</span>
        </div>
        
        <div class="config-item checkbox-item">
          <label>
            <input type="checkbox" v-model="vncConfig.view_only" :disabled="disabled" @change="saveVncConfig" />
            View-Only Mode
          </label>
          <span class="help-text-inline">Disable keyboard/mouse input (view only)</span>
        </div>
      </div>
    </div>
    
    <div class="setting-group">
      <h4>🔊 Audio Configuration</h4>
      <div class="config-grid">
        <div class="config-item">
          <label for="audio-sample-rate">Sample Rate:</label>
          <select id="audio-sample-rate" v-model.number="audioConfig.sample_rate" :disabled="disabled" @change="saveAudioConfig">
            <option :value="44100">44.1 kHz (CD Quality)</option>
            <option :value="48000">48 kHz (Professional)</option>
          </select>
          <span class="help-text-inline">Higher values = better quality but more bandwidth</span>
        </div>
        
        <div class="config-item">
          <label for="audio-quality">Encoding Quality:</label>
          <select id="audio-quality" v-model="audioConfig.quality" :disabled="disabled" @change="saveAudioConfig">
            <option value="voip">VoIP (Lowest bandwidth, good for voice)</option>
            <option value="audio">Audio (Balanced for music)</option>
            <option value="high">High Quality (Best quality, higher bandwidth)</option>
          </select>
        </div>
        
        <div class="config-item">
          <label for="audio-channels">Channels:</label>
          <select id="audio-channels" v-model="audioConfig.channels" :disabled="disabled" @change="saveAudioConfig">
            <option value="mono">Mono (Half bandwidth)</option>
            <option value="stereo">Stereo (Full quality)</option>
          </select>
        </div>
        
        <div class="config-item">
          <label for="audio-latency">Latency Mode:</label>
          <select id="audio-latency" v-model="audioConfig.latency" :disabled="disabled" @change="saveAudioConfig">
            <option value="ultra_low">Ultra Low (~5-10ms, may glitch)</option>
            <option value="low">Low (~10-20ms, balanced)</option>
            <option value="normal">Normal (~30-50ms, stable)</option>
          </select>
          <span class="help-text-inline">Lower latency may cause audio glitches on slow networks</span>
        </div>
      </div>
    </div>
    
    <div class="setting-group">
      <h4>🌐 Connection Settings</h4>
      <div class="config-grid">
        <div class="config-item">
          <label for="keep-alive">Keep-Alive Interval:</label>
          <div class="input-with-unit">
            <input type="number" id="keep-alive" v-model.number="connectionConfig.keep_alive_interval"
                   min="10" max="300" :disabled="disabled" @change="saveConnectionConfig" />
            <span class="unit">seconds</span>
          </div>
          <span class="help-text-inline">Ping interval to keep connections alive</span>
        </div>
        
        <div class="config-item">
          <label for="connection-timeout">Connection Timeout:</label>
          <div class="input-with-unit">
            <input type="number" id="connection-timeout" v-model.number="connectionConfig.connection_timeout"
                   min="60" max="3600" :disabled="disabled" @change="saveConnectionConfig" />
            <span class="unit">seconds</span>
          </div>
          <span class="help-text-inline">Idle timeout before disconnecting</span>
        </div>
        
        <div class="config-item">
          <label for="max-clients">Max Clients per Monitor:</label>
          <input type="number" id="max-clients" v-model.number="connectionConfig.max_clients_per_monitor"
                 min="1" max="50" :disabled="disabled" @change="saveConnectionConfig" />
          <span class="help-text-inline">Maximum concurrent connections per screen</span>
        </div>
        
        <div class="config-item checkbox-item">
          <label>
            <input type="checkbox" v-model="connectionConfig.auto_reconnect" :disabled="disabled" @change="saveConnectionConfig" />
            Enable Auto-Reconnect
          </label>
          <span class="help-text-inline">Automatically reconnect on connection loss</span>
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

// Configuration objects
const vncConfig = ref({
  base_port: 5900,
  quality: 'high',
  frame_rate_limit: 60,
  cursor_encoding: true,
  desktop_resize: true,
  view_only: false
});

const audioConfig = ref({
  sample_rate: 48000,
  quality: 'high',
  channels: 'stereo',
  latency: 'low'
});

const connectionConfig = ref({
  keep_alive_interval: 30,
  connection_timeout: 300,
  auto_reconnect: true,
  max_clients_per_monitor: 5
});

onMounted(async () => {
  try {
    useTlsUrls.value = await invoke('get_use_tls_urls');
  } catch (error) {
    console.error('Failed to get TLS setting:', error);
    useTlsUrls.value = false;
  }
  
  // Load VNC configuration
  try {
    vncConfig.value = await invoke('get_vnc_config');
  } catch (error) {
    console.error('Failed to load VNC config:', error);
  }
  
  // Load audio configuration
  try {
    audioConfig.value = await invoke('get_audio_config');
  } catch (error) {
    console.error('Failed to load audio config:', error);
  }
  
  // Load connection configuration
  try {
    connectionConfig.value = await invoke('get_connection_config');
  } catch (error) {
    console.error('Failed to load connection config:', error);
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

async function saveVncConfig() {
  try {
    await invoke('set_vnc_config', { config: vncConfig.value });
    console.log('VNC configuration saved:', vncConfig.value);
  } catch (error) {
    console.error('Failed to save VNC config:', error);
  }
}

async function saveAudioConfig() {
  try {
    await invoke('set_audio_config', { config: audioConfig.value });
    console.log('Audio configuration saved:', audioConfig.value);
  } catch (error) {
    console.error('Failed to save audio config:', error);
  }
}

async function saveConnectionConfig() {
  try {
    await invoke('set_connection_config', { config: connectionConfig.value });
    console.log('Connection configuration saved:', connectionConfig.value);
  } catch (error) {
    console.error('Failed to save connection config:', error);
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

/* Configuration Grid Layout */
.config-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1rem;
  margin-top: 0.75rem;
}

@media (min-width: 768px) {
  .config-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.config-item {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.config-item label {
  font-weight: 500;
  font-size: 0.9rem;
  color: #495057;
}

.config-item input[type="number"],
.config-item select {
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 0.9rem;
}

.config-item input[type="range"] {
  width: 100%;
}

.slider-container {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.slider-container input[type="range"] {
  flex: 1;
}

.value-display {
  min-width: 60px;
  text-align: right;
  font-weight: 500;
  color: #495057;
  font-size: 0.9rem;
}

.input-with-unit {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.input-with-unit input {
  flex: 1;
}

.unit {
  font-size: 0.85rem;
  color: #666;
  white-space: nowrap;
}

.checkbox-item {
  flex-direction: row;
  align-items: flex-start;
}

.checkbox-item label {
  display: flex;
  align-items: flex-start;
  cursor: pointer;
  font-weight: normal;
}

.checkbox-item input[type="checkbox"] {
  margin-right: 0.5rem;
  margin-top: 0.15rem;
}
</style>
