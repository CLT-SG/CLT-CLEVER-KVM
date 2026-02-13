<script setup>
import { onMounted, computed } from "vue";
import { useServer } from "./composables";
import { presets } from "./constants";
import { version } from "../package.json";

import {
  TabContainer,
  ServerStatus,
  ServerConfiguration,
  ConnectionOptions,
  LogViewer,
  UpdaterDialog
} from "./components";

import SettingsPanel from "./components/ui/SettingsPanel.vue";

const {
  serverStatus,
  serverUrl,
  serverPort,
  loading,
  errorMessage,
  settings,
  displays,
  loadingDisplays,
  checkServerStatus,
  startServer,
  stopServer,
  openUrl,
  copyUrl
} = useServer();

function applyPreset(presetName) {
  const preset = presets[presetName];
  if (preset) {
    Object.keys(preset).forEach(key => {
      if (key in settings) {
        settings[key] = preset[key];
      }
    });
  }
}

function updateServerPort(value) {
  serverPort.value = value;
}

function updateSelectedDisplay(value) {
  settings.selectedWebrtcDisplay = value;
}

// Define tabs with SVG icon names (Material Design style)
const tabs = computed(() => [
  { id: 'status', label: 'Status', icon: 'status' },
  { id: 'config', label: 'Configuration', icon: 'config' },
  { id: 'options', label: 'Options', icon: 'options' },
  { id: 'logs', label: 'Logs', icon: 'logs' },
  { id: 'settings', label: 'Settings', icon: 'settings' }
]);
</script>

<template>
  <main class="app-container">
    <!-- Header -->
    <header class="app-header">
      <div class="logo-section">
        <div class="logo-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="3" width="20" height="14" rx="2" />
            <line x1="8" y1="21" x2="16" y2="21" />
            <line x1="12" y1="17" x2="12" y2="21" />
          </svg>
        </div>
        <div class="logo-text">
          <h1>CLEVER KVM</h1>
          <span class="version">v{{ version }}</span>
        </div>
      </div>
      <div class="header-status">
        <span class="status-indicator" :class="{ active: serverStatus }"></span>
        <span class="status-label">{{ serverStatus ? 'Online' : 'Offline' }}</span>
      </div>
    </header>

    <!-- Main Content -->
    <TabContainer :tabs="tabs" default-tab="status">
      <template #status>
        <div class="panel-content">
          <ServerStatus 
            :server-status="serverStatus"
            :server-url="serverUrl"
            :loading="loading"
            :error-message="errorMessage"
            :start-server="startServer"
            :stop-server="stopServer"
            :open-url="openUrl"
            :copy-url="copyUrl"
          />
        </div>
      </template>

      <template #config>
        <div class="panel-content">
          <div class="panel-header">
            <h2>Server Configuration</h2>
            <span v-if="serverStatus" class="config-badge warning">
              Server running - stop to modify
            </span>
          </div>
          <ServerConfiguration 
            :server-port="serverPort"
            :settings="settings"
            :displays="displays"
            :disabled="serverStatus"
            @apply-preset="applyPreset"
            @update:server-port="updateServerPort"
            @update:selected-display="updateSelectedDisplay"
          />
        </div>
      </template>

      <template #options>
        <div class="panel-content">
          <ConnectionOptions />
        </div>
      </template>

      <template #logs>
        <div class="panel-content">
          <LogViewer />
        </div>
      </template>

      <template #settings>
        <div class="panel-content">
          <SettingsPanel />
        </div>
      </template>
    </TabContainer>

    <!-- Auto-updater dialog -->
    <UpdaterDialog />
  </main>
</template>

<style scoped>
.app-container {
  max-width: 1000px;
  margin: 0 auto;
  padding: var(--spacing-lg);
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Header Styles */
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-lg);
  padding-bottom: var(--spacing-md);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.logo-section {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
}

.logo-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, var(--color-primary) 0%, var(--color-primary-dark) 100%);
  border-radius: var(--radius-lg);
  color: var(--bg-primary);
  box-shadow: var(--shadow-glow);
}

.logo-icon svg {
  width: 28px;
  height: 28px;
}

.logo-text h1 {
  font-size: 1.75rem;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  letter-spacing: -0.02em;
}

.version {
  font-size: 0.75rem;
  color: var(--text-muted);
  font-family: var(--font-mono);
}

.header-status {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--bg-card);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
}

.status-indicator {
  width: 10px;
  height: 10px;
  border-radius: var(--radius-full);
  background: var(--color-error);
  transition: all var(--transition-normal);
}

.status-indicator.active {
  background: var(--color-success);
  box-shadow: 0 0 12px var(--color-success);
  animation: pulse 2s ease-in-out infinite;
}

.status-label {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--text-secondary);
}

/* Panel Content */
.panel-content {
  animation: fadeIn 0.3s ease;
  height: 100%;
  overflow-y: auto;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-lg);
}

.panel-header h2 {
  margin: 0;
  font-size: 1.35rem;
  color: var(--text-primary);
}

.config-badge {
  font-size: 0.75rem;
  padding: 0.35rem 0.75rem;
  border-radius: var(--radius-sm);
}

.config-badge.warning {
  background: rgba(255, 174, 0, 0.15);
  color: var(--color-warning);
  border: 1px solid rgba(255, 174, 0, 0.3);
}

/* Update Section */
.update-section {
  margin-top: var(--spacing-xl);
  padding-top: var(--spacing-xl);
  border-top: 1px solid var(--border-color);
}

.update-section h3 {
  margin: 0 0 var(--spacing-md) 0;
  font-size: 1.1rem;
  color: var(--text-primary);
}

/* Responsive */
@media (max-width: 768px) {
  .app-container {
    padding: var(--spacing-md);
  }
  
  .app-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
  }
  
  .logo-text h1 {
    font-size: 1.5rem;
  }
  
  .panel-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-sm);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.8;
    transform: scale(1.15);
  }
}
</style>
