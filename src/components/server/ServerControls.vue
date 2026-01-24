<template>
  <div class="vnc-controls">
    <h3>VNC Server</h3>
    
    <div class="vnc-settings">
      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="vncEnabled" :disabled="vncRunning" />
          <span>Enable VNC Server</span>
        </label>
      </div>
      
      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="audioEnabled" :disabled="!vncEnabled || vncRunning" />
          <span>Enable Audio Stream</span>
        </label>
      </div>
      
      <div class="port-settings">
        <div class="setting-row">
          <label>
            <span class="label-text">VNC Port:</span>
            <input 
              type="number" 
              v-model.number="vncPort" 
              :disabled="vncRunning"
              min="1024"
              max="65535"
              class="port-input"
            />
          </label>
        </div>
        
        <div class="setting-row" v-if="audioEnabled">
          <label>
            <span class="label-text">Audio Port:</span>
            <input 
              type="number" 
              v-model.number="audioPort" 
              :disabled="vncRunning"
              min="1024"
              max="65535"
              class="port-input"
            />
          </label>
        </div>

        <div class="setting-row">
          <label>
            <span class="label-text">Monitor:</span>
            <select v-model.number="selectedMonitor" :disabled="vncRunning" class="monitor-select">
              <option :value="0">Primary Monitor</option>
              <option v-for="(monitor, index) in monitors" :key="monitor.id" :value="index">
                {{ monitor.name }} ({{ monitor.width }}x{{ monitor.height }})
              </option>
            </select>
          </label>
        </div>
      </div>
    </div>
    
    <div class="control-buttons">
      <button 
        @click="toggleVnc" 
        :disabled="!vncEnabled && !vncRunning"
        :class="{ 'btn-primary': !vncRunning, 'btn-danger': vncRunning }"
        class="vnc-toggle-btn"
      >
        {{ vncRunning ? 'Stop VNC Server' : 'Start VNC Server' }}
      </button>
    </div>
    
    <div v-if="vncRunning" class="vnc-info">
      <h4>VNC Server Status</h4>
      <div class="info-grid">
        <div class="info-item">
          <span class="info-label">VNC URL:</span>
          <code class="info-value">{{ vncInfo?.vnc_url }}</code>
          <button @click="copyToClipboard(vncInfo?.vnc_url)" class="copy-btn" title="Copy to clipboard">
            Copy
          </button>
        </div>
        
        <div class="info-item" v-if="vncInfo?.audio_url">
          <span class="info-label">Audio URL:</span>
          <code class="info-value">{{ vncInfo.audio_url }}</code>
          <button @click="copyToClipboard(vncInfo.audio_url)" class="copy-btn" title="Copy to clipboard">
            Copy
          </button>
        </div>
        
        <div class="info-item">
          <span class="info-label">Connected Clients:</span>
          <span class="info-value clients-count">{{ vncStatus?.clients || 0 }}</span>
        </div>
        
        <div class="info-item" v-if="vncStatus?.registration_status?.registered">
          <span class="info-label">Registration Status:</span>
          <span class="info-value status-registered">Registered (ID: {{ vncStatus.registration_status.id }})</span>
        </div>
      </div>
      
      <div class="action-buttons">
        <button 
          @click="registerWithCleverService" 
          :disabled="registering || (vncStatus?.registration_status?.registered)"
          class="btn-secondary"
        >
          {{ vncStatus?.registration_status?.registered ? 'Registered' : 'Register with CLEVER Service' }}
        </button>
        
        <button @click="refreshStatus" class="btn-secondary">
          Refresh Status
        </button>
      </div>
    </div>

    <div v-if="error" class="error-message">
      {{ error }}
    </div>

    <div v-if="successMessage" class="success-message">
      {{ successMessage }}
    </div>
  </div>
</template>

<script>
import { invoke } from '@tauri-apps/api/tauri';

export default {
  name: 'ServerControls',
  data() {
    return {
      vncEnabled: false,
      audioEnabled: true,
      vncPort: 5900,
      audioPort: 5901,
      selectedMonitor: 0,
      monitors: [],
      vncRunning: false,
      vncInfo: null,
      vncStatus: null,
      registering: false,
      statusInterval: null,
      error: null,
      successMessage: null,
      cleverServiceUrl: import.meta.env.VITE_CLEVER_SERVICE_URL || 'http://clever-service:8000',
    };
  },
  async mounted() {
    await this.loadMonitors();
    await this.checkVncStatus();
  },
  beforeUnmount() {
    this.stopStatusPolling();
  },
  methods: {
    async loadMonitors() {
      try {
        this.monitors = await invoke('get_available_monitors');
      } catch (e) {
        console.error('Failed to load monitors:', e);
      }
    },
    
    async toggleVnc() {
      this.error = null;
      this.successMessage = null;

      try {
        if (this.vncRunning) {
          await invoke('stop_vnc_server');
          this.vncRunning = false;
          this.vncInfo = null;
          this.vncStatus = null;
          this.stopStatusPolling();
          this.successMessage = 'VNC server stopped successfully';
        } else {
          this.vncInfo = await invoke('start_vnc_server', {
            port: this.vncPort,
            monitor: this.selectedMonitor,
            enableAudio: this.audioEnabled,
            audioPort: this.audioEnabled ? this.audioPort : null,
          });
          this.vncRunning = true;
          this.successMessage = 'VNC server started successfully';
          this.startStatusPolling();
        }
      } catch (e) {
        this.error = `Failed to ${this.vncRunning ? 'stop' : 'start'} VNC server: ${e}`;
        console.error('VNC server error:', e);
      }
    },
    
    async registerWithCleverService() {
      this.error = null;
      this.successMessage = null;
      this.registering = true;

      try {
        const registration = await invoke('register_with_clever_service', {
          cleverUrl: this.cleverServiceUrl
        });
        
        this.successMessage = `Successfully registered with CLEVER Service (ID: ${registration.id})`;
        await this.checkVncStatus();
      } catch (e) {
        this.error = `Failed to register with CLEVER Service: ${e}`;
        console.error('Registration error:', e);
      } finally {
        this.registering = false;
      }
    },
    
    async checkVncStatus() {
      try {
        this.vncStatus = await invoke('get_vnc_status');
        this.vncRunning = this.vncStatus.running;
        
        if (this.vncRunning && !this.statusInterval) {
          this.startStatusPolling();
        } else if (!this.vncRunning && this.statusInterval) {
          this.stopStatusPolling();
        }
      } catch (e) {
        console.error('Failed to get VNC status:', e);
      }
    },
    
    async refreshStatus() {
      await this.checkVncStatus();
      this.successMessage = 'Status refreshed';
      setTimeout(() => {
        this.successMessage = null;
      }, 2000);
    },
    
    startStatusPolling() {
      if (this.statusInterval) return;
      
      this.statusInterval = setInterval(async () => {
        if (this.vncRunning) {
          await this.checkVncStatus();
        }
      }, 2000);
    },
    
    stopStatusPolling() {
      if (this.statusInterval) {
        clearInterval(this.statusInterval);
        this.statusInterval = null;
      }
    },
    
    copyToClipboard(text) {
      if (!text) return;
      
      navigator.clipboard.writeText(text).then(() => {
        this.successMessage = 'Copied to clipboard!';
        setTimeout(() => {
          this.successMessage = null;
        }, 2000);
      }).catch((e) => {
        this.error = 'Failed to copy to clipboard';
        console.error('Copy error:', e);
      });
    }
  }
};
</script>

<style scoped>
.vnc-controls {
  padding: 20px;
  background: var(--bg-secondary, #f5f5f5);
  border-radius: 8px;
  margin-bottom: 20px;
}

.vnc-controls h3 {
  margin: 0 0 20px 0;
  color: var(--text-primary, #333);
  font-size: 1.5em;
}

.vnc-controls h4 {
  margin: 15px 0 10px 0;
  color: var(--text-secondary, #666);
  font-size: 1.2em;
}

.vnc-settings {
  background: white;
  padding: 15px;
  border-radius: 6px;
  margin-bottom: 15px;
}

.setting-row {
  margin-bottom: 12px;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.checkbox-label {
  display: flex;
  align-items: center;
  cursor: pointer;
  user-select: none;
}

.checkbox-label input[type="checkbox"] {
  margin-right: 8px;
  width: 18px;
  height: 18px;
  cursor: pointer;
}

.checkbox-label span {
  font-weight: 500;
  color: var(--text-primary, #333);
}

.label-text {
  display: inline-block;
  min-width: 100px;
  font-weight: 500;
  color: var(--text-secondary, #666);
}

.port-input,
.monitor-select {
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
  width: 200px;
  box-sizing: border-box;
}

.port-input:focus,
.monitor-select:focus {
  outline: none;
  border-color: var(--primary-color, #007bff);
  box-shadow: 0 0 0 2px rgba(0, 123, 255, 0.1);
}

.control-buttons,
.action-buttons {
  display: flex;
  gap: 10px;
  margin-top: 15px;
}

.vnc-toggle-btn,
.btn-primary,
.btn-secondary,
.btn-danger {
  padding: 10px 20px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary {
  background: var(--success-color, #28a745);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: var(--success-hover, #218838);
}

.btn-danger {
  background: var(--danger-color, #dc3545);
  color: white;
}

.btn-danger:hover:not(:disabled) {
  background: var(--danger-hover, #c82333);
}

.btn-secondary {
  background: var(--secondary-color, #6c757d);
  color: white;
}

.btn-secondary:hover:not(:disabled) {
  background: var(--secondary-hover, #5a6268);
}

button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.vnc-info {
  background: white;
  padding: 15px;
  border-radius: 6px;
  margin-top: 15px;
}

.info-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 15px;
}

.info-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  background: var(--bg-secondary, #f8f9fa);
  border-radius: 4px;
}

.info-label {
  font-weight: 600;
  color: var(--text-secondary, #666);
  min-width: 120px;
}

.info-value {
  flex: 1;
  color: var(--text-primary, #333);
}

code.info-value {
  background: #e9ecef;
  padding: 4px 8px;
  border-radius: 3px;
  font-family: 'Courier New', monospace;
  font-size: 13px;
}

.clients-count {
  font-weight: 700;
  font-size: 1.2em;
  color: var(--primary-color, #007bff);
}

.status-registered {
  color: var(--success-color, #28a745);
  font-weight: 600;
}

.status-registered::before {
  content: "✓ ";
  font-weight: bold;
  margin-right: 0.25rem;
}

.copy-btn {
  padding: 0.4rem 0.75rem;
  background: var(--primary-color, #007bff);
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
  font-weight: 500;
  transition: background-color 0.2s;
}

.copy-btn:hover {
  background-color: #0056b3;
}

.error-message {
  background: #f8d7da;
  color: #721c24;
  padding: 12px;
  border-radius: 6px;
  margin-top: 15px;
  border: 1px solid #f5c6cb;
}

.success-message {
  background: #d4edda;
  color: #155724;
  padding: 12px;
  border-radius: 6px;
  margin-top: 15px;
  border: 1px solid #c3e6cb;
}
</style>
