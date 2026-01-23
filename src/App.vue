<script setup>
import { onMounted, computed } from "vue";
import { useServer } from "./composables";

import {
  TabContainer,
  ServerStatus,
  ServerConfiguration,
  ConnectionOptions,
  LogViewer,
  UpdaterDialog,
  UpdateChecker
} from "./components";

const {
  serverStatus,
  serverUrl,
  serverPort,
  loading,
  errorMessage,
  settings,
  monitors,
  loadingMonitors,
  vncInfo,
  checkServerStatus,
  startServer,
  stopServer,
  openUrl,
  copyUrl
} = useServer();

// The status checking is now automatic, but we can still call it manually if needed
onMounted(async () => {
  // Initial check is now handled by the composable
  // await checkServerStatus();
});

function updateServerPort(value) {
  serverPort.value = value;
}

function updateSelectedMonitor(value) {
  settings.selectedMonitor = value;
}

// Define tabs based on server status
const tabs = computed(() => {
  const baseTabs = [
    { id: 'status', label: 'Server Status' },
    { id: 'config', label: 'Configuration' },
    { id: 'options', label: 'Connection Options' },
    { id: 'logs', label: 'Logs' }
  ];

  return baseTabs;
});
</script>

<template>
  <main class="container">
    <div class="header">
      <h1>Clever KVM</h1>
    </div>

    <TabContainer :tabs="tabs" default-tab="status">
      <template #status>
        <ServerStatus 
          :server-status="serverStatus"
          :server-url="serverUrl"
          :vnc-info="vncInfo"
          :loading="loading"
          :error-message="errorMessage"
          :start-server="startServer"
          :stop-server="stopServer"
          :copy-url="copyUrl"
        />
        
        <!-- Update checker section -->
        <div class="update-section">
          <h3>Application Updates</h3>
          <UpdateChecker />
        </div>
      </template>

      <template #config>
        <div class="config-content">
          <h2>Server Configuration</h2>
          <div v-if="serverStatus" class="config-warning">
            <p>⚠️ Server is currently running. Stop the server to modify these settings.</p>
          </div>
          <ServerConfiguration 
            :server-port="serverPort"
            :settings="settings"
            :monitors="monitors"
            :disabled="serverStatus"
            @update:server-port="updateServerPort"
            @update:selected-monitor="updateSelectedMonitor"
          />
        </div>
      </template>

      <template #options>
        <ConnectionOptions />
      </template>

      <template #logs>
        <LogViewer />
      </template>
    </TabContainer>

    <!-- Auto-updater dialog -->
    <UpdaterDialog />
  </main>
</template>

<style scoped>
.container {
  max-width: 1000px;
  margin: 0 auto;
  padding: 1rem;
  min-height: 100vh;
}

.header {
  text-align: center;
  margin-bottom: 2rem;
}

h1 {
  font-size: 2.5rem;
  margin-bottom: 0.5rem;
  color: #2c3e50;
}

.description {
  font-size: 1.2rem;
  color: #7f8c8d;
  margin-bottom: 0;
}

.config-content h2 {
  margin-top: 0;
  color: #2c3e50;
  font-size: 1.5rem;
  margin-bottom: 1.5rem;
}

.config-warning {
  background-color: #fff3cd;
  border: 1px solid #ffeaa7;
  border-radius: 4px;
  padding: 1rem;
  margin-bottom: 1.5rem;
  color: #856404;
}

.config-warning p {
  margin: 0;
  font-weight: 500;
}

.update-section {
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid #e9ecef;
}

.update-section h3 {
  margin: 0 0 1rem 0;
  color: #2c3e50;
  font-size: 1.25rem;
}

@media (max-width: 768px) {
  .container {
    padding: 0.5rem;
  }
  
  h1 {
    font-size: 2rem;
  }
  
  .description {
    font-size: 1rem;
  }
}
</style>