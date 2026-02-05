<template>
  <div class="relay-status">
    <div class="relay-header">
      <h3>Relay Server</h3>
      <div class="status-badge" :class="statusClass">
        {{ statusText }}
      </div>
    </div>

    <div v-if="relayStatus.connected" class="relay-info">
      <div class="info-row">
        <span class="label">Server:</span>
        <span class="value">{{ relayStatus.relayHostname }}</span>
      </div>
      <div class="info-row">
        <span class="label">Dashboard:</span>
        <span class="value url">
          <a :href="relayStatus.relayUrl" target="_blank">{{ relayStatus.relayUrl }}</a>
        </span>
      </div>
      <p class="info-note success">
        Your device is visible on the relay server dashboard. Viewers can connect through the relay.
      </p>
    </div>

    <div v-else-if="isReconnecting" class="relay-reconnecting">
      <div class="info-row">
        <span class="label">Server:</span>
        <span class="value">{{ relayStatus.relayHostname || 'Unknown' }}</span>
      </div>
      <p class="info-note warning">
        <span class="reconnect-icon">🔄</span>
        Connection lost. Auto-reconnecting...
        <span v-if="relayStatus.reconnectAttempts > 0" class="attempt-count">
          (Attempt {{ relayStatus.reconnectAttempts }})
        </span>
      </p>
    </div>

    <div v-else class="relay-disconnected">
      <p class="info-note">
        Connect to a relay server to make your device accessible from a centralized dashboard.
      </p>
    </div>

    <!-- Auto-reconnect toggle -->
    <div v-if="relayStatus.connected || isReconnecting" class="auto-reconnect-toggle">
      <label class="toggle-label">
        <input 
          type="checkbox" 
          :checked="relayStatus.autoReconnect"
          @change="handleAutoReconnectToggle"
          :disabled="relayLoading"
        />
        <span class="toggle-text">Auto-reconnect when server is down</span>
      </label>
    </div>

    <div class="relay-actions">
      <template v-if="!relayStatus.connected && !isReconnecting">
        <button 
          class="secondary-button"
          @click="handleDiscover"
          :disabled="relayLoading"
        >
          {{ relayLoading ? 'Searching...' : 'Discover' }}
        </button>
        <button 
          class="primary-button"
          @click="handleAutoConnect"
          :disabled="relayLoading"
        >
          {{ relayLoading ? 'Connecting...' : 'Auto Connect' }}
        </button>
      </template>
      <template v-else-if="isReconnecting">
        <button 
          class="secondary-button"
          @click="handleDisconnect"
          :disabled="relayLoading"
        >
          Cancel Reconnection
        </button>
      </template>
      <template v-else>
        <button 
          class="secondary-button"
          @click="openDashboard"
        >
          Open Dashboard
        </button>
        <button 
          class="danger-button"
          @click="handleDisconnect"
          :disabled="relayLoading"
        >
          {{ relayLoading ? 'Disconnecting...' : 'Disconnect' }}
        </button>
      </template>
    </div>

    <!-- Discovered relays list -->
    <div v-if="discoveredRelays.length > 0 && !relayStatus.connected && !isReconnecting" class="discovered-relays">
      <h4>Available Relay Servers</h4>
      <ul class="relay-list">
        <li 
          v-for="relay in discoveredRelays" 
          :key="relay.url"
          class="relay-item"
        >
          <div class="relay-item-info">
            <span class="relay-name">{{ relay.hostname }}</span>
            <span class="relay-url">{{ relay.url }}</span>
          </div>
          <button 
            class="connect-button"
            @click="handleConnect(relay.url)"
            :disabled="relayLoading"
          >
            Connect
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  relayStatus: {
    type: Object,
    required: true
  },
  discoveredRelays: {
    type: Array,
    default: () => []
  },
  relayLoading: {
    type: Boolean,
    default: false
  },
  discoverRelays: {
    type: Function,
    required: true
  },
  connectToRelay: {
    type: Function,
    required: true
  },
  disconnectFromRelay: {
    type: Function,
    required: true
  },
  autoConnectRelay: {
    type: Function,
    required: true
  },
  setRelayAutoReconnect: {
    type: Function,
    required: false,
    default: () => {}
  }
});

const isReconnecting = computed(() => {
  return props.relayStatus.state === 'reconnecting';
});

const statusClass = computed(() => {
  if (props.relayStatus.connected) return 'connected';
  if (props.relayStatus.state === 'connecting') return 'connecting';
  if (props.relayStatus.state === 'reconnecting') return 'reconnecting';
  if (props.relayStatus.state === 'error') return 'error';
  return 'disconnected';
});

const statusText = computed(() => {
  if (props.relayStatus.connected) return 'Connected';
  if (props.relayStatus.state === 'connecting') return 'Connecting...';
  if (props.relayStatus.state === 'reconnecting') return 'Reconnecting...';
  if (props.relayStatus.state === 'error') return 'Error';
  return 'Disconnected';
});

async function handleDiscover() {
  await props.discoverRelays();
}

async function handleConnect(url) {
  await props.connectToRelay(url);
}

async function handleDisconnect() {
  await props.disconnectFromRelay();
}

async function handleAutoConnect() {
  await props.autoConnectRelay();
}

async function handleAutoReconnectToggle(event) {
  const enabled = event.target.checked;
  await props.setRelayAutoReconnect(enabled);
}

function openDashboard() {
  if (props.relayStatus.relayUrl) {
    window.open(props.relayStatus.relayUrl, '_blank');
  }
}
</script>

<style scoped>
.relay-status {
  background: #f8f9fa;
  border-radius: 8px;
  padding: 1rem;
  margin-top: 1rem;
}

.relay-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
}

.relay-header h3 {
  margin: 0;
  font-size: 1rem;
  color: #2c3e50;
}

.status-badge {
  padding: 0.25rem 0.75rem;
  border-radius: 20px;
  font-size: 0.85rem;
  font-weight: 500;
}

.status-badge.connected {
  background: #d4edda;
  color: #155724;
}

.status-badge.connecting {
  background: #fff3cd;
  color: #856404;
}

.status-badge.reconnecting {
  background: #fff3cd;
  color: #856404;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.status-badge.disconnected {
  background: #e9ecef;
  color: #6c757d;
}

.status-badge.error {
  background: #f8d7da;
  color: #721c24;
}

.relay-info {
  margin-bottom: 1rem;
}

.info-row {
  display: flex;
  margin-bottom: 0.5rem;
}

.info-row .label {
  color: #6c757d;
  width: 60px;
  flex-shrink: 0;
}

.info-row .value {
  color: #2c3e50;
  font-weight: 500;
}

.info-row .value.url {
  font-family: monospace;
  font-size: 0.9rem;
  word-break: break-all;
}

.info-row .value.url a {
  color: #3498db;
  text-decoration: none;
}

.info-row .value.url a:hover {
  text-decoration: underline;
}

.info-note {
  font-size: 0.9rem;
  color: #6c757d;
  margin: 0.5rem 0;
  padding: 0.5rem;
  background: #fff;
  border-radius: 4px;
}

.info-note.success {
  background: #d4edda;
  color: #155724;
}

.info-note.warning {
  background: #fff3cd;
  color: #856404;
}

.relay-reconnecting {
  margin-bottom: 1rem;
}

.reconnect-icon {
  display: inline-block;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.attempt-count {
  font-size: 0.85rem;
  opacity: 0.8;
}

.auto-reconnect-toggle {
  margin: 0.75rem 0;
  padding: 0.5rem;
  background: #fff;
  border-radius: 4px;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  font-size: 0.9rem;
  color: #2c3e50;
}

.toggle-label input[type="checkbox"] {
  width: 1rem;
  height: 1rem;
  cursor: pointer;
}

.toggle-text {
  user-select: none;
}

.relay-disconnected {
  margin-bottom: 1rem;
}

.relay-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.primary-button,
.secondary-button,
.danger-button,
.connect-button {
  padding: 0.5rem 1rem;
  border-radius: 4px;
  border: none;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.primary-button {
  background: #3498db;
  color: white;
}

.primary-button:hover:not(:disabled) {
  background: #2980b9;
}

.secondary-button {
  background: #ecf0f1;
  color: #2c3e50;
}

.secondary-button:hover:not(:disabled) {
  background: #dfe6e9;
}

.danger-button {
  background: #e74c3c;
  color: white;
}

.danger-button:hover:not(:disabled) {
  background: #c0392b;
}

.connect-button {
  background: #27ae60;
  color: white;
  padding: 0.25rem 0.75rem;
}

.connect-button:hover:not(:disabled) {
  background: #219a52;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.discovered-relays {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #dee2e6;
}

.discovered-relays h4 {
  margin: 0 0 0.75rem 0;
  font-size: 0.95rem;
  color: #2c3e50;
}

.relay-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.relay-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem;
  background: #fff;
  border-radius: 4px;
  margin-bottom: 0.5rem;
}

.relay-item-info {
  display: flex;
  flex-direction: column;
}

.relay-name {
  font-weight: 500;
  color: #2c3e50;
}

.relay-url {
  font-size: 0.8rem;
  color: #6c757d;
  font-family: monospace;
}
</style>
