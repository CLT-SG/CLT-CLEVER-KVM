import { ref, reactive, computed, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/tauri";

export function useServer() {
  const serverStatus = ref(false);
  const serverUrl = ref("");
  const serverPort = ref(5900); // VNC default port
  const loading = ref(false);
  const errorMessage = ref("");
  const monitors = ref([]);
  const loadingMonitors = ref(false);
  const vncInfo = ref(null);

  // Status check interval
  let statusCheckInterval = null;

  // Clean up interval on unmount
  onUnmounted(() => {
    if (statusCheckInterval) {
      clearInterval(statusCheckInterval);
    }
  });

  // VNC Server settings
  const settings = reactive({
    enableAudio: true,
    selectedMonitor: 0,
    audioPort: 6900,
    autoStart: true,
    mediamtxUrl: '',
    mediamtxAutoScan: true
  });

  // Load settings from localStorage
  function loadSettings() {
    try {
      const savedSettings = localStorage.getItem('vnc-settings');
      if (savedSettings) {
        const parsed = JSON.parse(savedSettings);
        Object.assign(settings, parsed);
      }
    } catch (error) {
      console.error("Failed to load settings from localStorage:", error);
    }
  }

  // Save settings to localStorage
  function saveSettings() {
    try {
      localStorage.setItem('vnc-settings', JSON.stringify(settings));
    } catch (error) {
      console.error("Failed to save settings to localStorage:", error);
    }
  }

  // Load settings on initialization
  loadSettings();

  // MediaMTX scanning state
  const scanningMediaMtx = ref(false);
  const mediamtxServers = ref([]);

  async function scanMediaMtxServers() {
    scanningMediaMtx.value = true;
    try {
      const servers = await invoke("scan_mediamtx_servers");
      mediamtxServers.value = servers;
      
      // Auto-select first found server if auto-scan is enabled and no URL is set
      if (settings.mediamtxAutoScan && servers.length > 0 && !settings.mediamtxUrl) {
        settings.mediamtxUrl = servers[0].url;
        saveSettings();
        console.log("Auto-selected MediaMTX server:", settings.mediamtxUrl);
      }
      
      return servers;
    } catch (error) {
      console.error("Failed to scan for MediaMTX servers:", error);
      mediamtxServers.value = [];
      return [];
    } finally {
      scanningMediaMtx.value = false;
    }
  }

  async function loadMonitors() {
    loadingMonitors.value = true;
    try {
      monitors.value = await invoke("get_available_monitors");
      
      const primaryIndex = monitors.value.findIndex(m => m.is_primary);
      if (primaryIndex >= 0) {
        settings.selectedMonitor = primaryIndex;
      }
    } catch (error) {
      console.error("Failed to load monitors:", error);
      monitors.value = [];
    } finally {
      loadingMonitors.value = false;
    }
  }

  async function checkServerStatus() {
    try {
      const status = await invoke("get_vnc_status");
      serverStatus.value = status.running;
      
      if (status.running) {
        // Update status info while preserving monitor details from start_vnc_server
        if (vncInfo.value) {
          // Merge status into existing vncInfo to preserve monitor details
          vncInfo.value = {
            ...vncInfo.value,
            clients_connected: status.clients,
            audio_enabled: status.audio_enabled,
            registration_status: status.registration_status
          };
        } else {
          // No existing vncInfo (e.g., after page reload), use status data
          vncInfo.value = status;
        }
        // Set a basic VNC URL (actual VNC URL is in vncInfo)
        serverUrl.value = "VNC Server Running";
      } else {
        serverUrl.value = "";
        vncInfo.value = null;
      }
      
      await loadMonitors();
    } catch (error) {
      console.error("Failed to check VNC status:", error);
      errorMessage.value = `Failed to check VNC status: ${error}`;
      serverStatus.value = false;
      serverUrl.value = "";
      vncInfo.value = null;
    }
  }

  // Start periodic status checking
  function startStatusMonitoring() {
    if (statusCheckInterval) {
      clearInterval(statusCheckInterval);
    }
    
    // Check status every 5 seconds
    statusCheckInterval = setInterval(async () => {
      await checkServerStatus();
    }, 5000);
  }

  // Stop periodic status checking
  function stopStatusMonitoring() {
    if (statusCheckInterval) {
      clearInterval(statusCheckInterval);
      statusCheckInterval = null;
    }
  }

  async function startServer() {
    loading.value = true;
    errorMessage.value = "";
    
    try {
      const info = await invoke("start_vnc_server", { 
        port: serverPort.value,
        monitor: settings.selectedMonitor,
        enableAudio: settings.enableAudio,
        audioPort: settings.enableAudio ? settings.audioPort : null
      });
      
      vncInfo.value = info;
      serverUrl.value = info.vnc_url;
      serverStatus.value = true;
      
      // Double-check the server status after starting
      setTimeout(async () => {
        await checkServerStatus();
      }, 1000);
      
    } catch (error) {
      console.error("Failed to start VNC server:", error);
      errorMessage.value = `Failed to start VNC server: ${error}`;
      serverStatus.value = false;
      serverUrl.value = "";
      vncInfo.value = null;
    } finally {
      loading.value = false;
    }
  }

  async function stopServer() {
    loading.value = true;
    errorMessage.value = "";
    
    try {
      await invoke("stop_vnc_server");
      serverStatus.value = false;
      serverUrl.value = "";
      vncInfo.value = null;
      
      // Double-check the server status after stopping
      setTimeout(async () => {
        await checkServerStatus();
      }, 1000);
      
    } catch (error) {
      console.error("Failed to stop VNC server:", error);
      errorMessage.value = `Failed to stop VNC server: ${error}`;
    } finally {
      loading.value = false;
    }
  }

  function buildUrlWithParams() {
    if (!vncInfo.value || !vncInfo.value.vnc_url) return "";
    
    return vncInfo.value.vnc_url;
  }

  function openUrl() {
    const url = buildUrlWithParams();
    if (url) {
      // Copy VNC URL to clipboard and show notification
      navigator.clipboard.writeText(url).then(() => {
        console.log(`VNC URL copied to clipboard: ${url}`);
      }).catch(err => {
        console.error('Failed to copy VNC URL:', err);
      });
    }
  }

  function copyUrl() {
    const url = buildUrlWithParams();
    if (url) {
      navigator.clipboard.writeText(url);
    }
  }

  // Initialize monitoring when composable is created
  checkServerStatus().then(async () => {
    startStatusMonitoring();
    
    // Auto-scan for MediaMTX servers if enabled
    if (settings.mediamtxAutoScan) {
      console.log("Auto-scanning for MediaMTX servers...");
      try {
        await scanMediaMtxServers();
      } catch (error) {
        console.error("Failed to scan for MediaMTX servers:", error);
      }
    }
    
    // Auto-start VNC server if enabled and not already running
    if (settings.autoStart && !serverStatus.value) {
      console.log("Auto-starting VNC server...");
      try {
        await startServer();
      } catch (error) {
        console.error("Failed to auto-start VNC server:", error);
      }
    }
  });

  return {
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
    copyUrl,
    loadMonitors,
    startStatusMonitoring,
    stopStatusMonitoring,
    saveSettings,
    scanningMediaMtx,
    mediamtxServers,
    scanMediaMtxServers
  };
}
