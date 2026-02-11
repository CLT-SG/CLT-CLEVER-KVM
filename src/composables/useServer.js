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

  // Relay server state
  const relayStatus = reactive({
    connected: false,
    relayUrl: null,
    relayHostname: null,
    state: "disconnected",
    autoReconnect: true,
    reconnectAttempts: 0
  });
  const discoveredRelays = ref([]);
  const relayLoading = ref(false);
  
  // Track if relay status command is available
  let relayCommandAvailable = true;

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
    deltaEncoding: true,
    adaptiveQuality: true,
    encryptionEnabled: false,
    useWebRTC: true,
    hardwareAcceleration: true,
    selectedMonitor: 0,
    audioPort: 6900,
    autoStart: true,
    mediamtxUrl: '',
    mediamtxAutoScan: true
  });

  const selectedCodec = computed(() => {
    return 'h264'; // H.264 is the only supported codec
  });

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

  async function checkRelayStatus() {
    // Skip if we already know the command is not available
    if (!relayCommandAvailable) {
      return;
    }
    
    try {
      const status = await invoke("get_relay_status");
      relayStatus.connected = status.connected;
      relayStatus.relayUrl = status.relay_url;
      relayStatus.relayHostname = status.relay_hostname;
      relayStatus.state = status.state;
      relayStatus.autoReconnect = status.auto_reconnect ?? true;
      relayStatus.reconnectAttempts = status.reconnect_attempts ?? 0;
    } catch (error) {
      // If command is not found, disable future checks to avoid spamming console
      if (error && String(error).includes("not found")) {
        relayCommandAvailable = false;
        relayStatus.state = "unavailable";
      } else {
        console.warn("Failed to check relay status:", error);
        relayStatus.state = "error";
      }
      relayStatus.connected = false;
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
      await checkRelayStatus();
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
        options: {
          deltaEncoding: settings.deltaEncoding,
          adaptiveQuality: settings.adaptiveQuality,
          encryption: settings.encryptionEnabled,
          webrtc: settings.useWebRTC,
          hardware_accel: settings.hardwareAcceleration, // H.264 hardware acceleration
          hardwareAcceleration: settings.hardwareAcceleration,
          monitor: settings.selectedMonitor,
          audioBitrate: settings.audioBitrate * 1000,
          videoBitrate: settings.videoBitrate * 1000,
          framerate: settings.framerate
        }
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

  // Relay server functions
  async function discoverRelays() {
    relayLoading.value = true;
    try {
      const relays = await invoke("discover_relay_servers", { timeoutMs: 3000 });
      discoveredRelays.value = relays;
      return relays;
    } catch (error) {
      console.error("Failed to discover relay servers:", error);
      return [];
    } finally {
      relayLoading.value = false;
    }
  }

  async function connectToRelay(relayUrl) {
    relayLoading.value = true;
    try {
      const status = await invoke("connect_to_relay", { relayUrl });
      relayStatus.connected = status.connected;
      relayStatus.relayUrl = status.relay_url;
      relayStatus.relayHostname = status.relay_hostname;
      relayStatus.state = status.state;
      relayStatus.autoReconnect = status.auto_reconnect ?? true;
      relayStatus.reconnectAttempts = status.reconnect_attempts ?? 0;
      return status;
    } catch (error) {
      console.error("Failed to connect to relay:", error);
      errorMessage.value = `Failed to connect to relay: ${error}`;
      throw error;
    } finally {
      relayLoading.value = false;
    }
  }

  async function disconnectFromRelay() {
    relayLoading.value = true;
    try {
      const status = await invoke("disconnect_from_relay");
      relayStatus.connected = false;
      relayStatus.relayUrl = null;
      relayStatus.relayHostname = null;
      relayStatus.state = "disconnected";
      return status;
    } catch (error) {
      console.error("Failed to disconnect from relay:", error);
      throw error;
    } finally {
      relayLoading.value = false;
    }
  }

  async function autoConnectRelay() {
    relayLoading.value = true;
    try {
      const status = await invoke("auto_connect_relay");
      relayStatus.connected = status.connected;
      relayStatus.relayUrl = status.relay_url;
      relayStatus.relayHostname = status.relay_hostname;
      relayStatus.state = status.state;
      relayStatus.autoReconnect = status.auto_reconnect ?? true;
      relayStatus.reconnectAttempts = status.reconnect_attempts ?? 0;
      return status;
    } catch (error) {
      console.error("Failed to auto-connect to relay:", error);
      return { connected: false };
    } finally {
      relayLoading.value = false;
    }
  }

  async function setRelayAutoReconnect(enabled) {
    try {
      const status = await invoke("set_relay_auto_reconnect", { enabled });
      relayStatus.autoReconnect = status.auto_reconnect ?? enabled;
      return status;
    } catch (error) {
      console.error("Failed to set auto-reconnect:", error);
      throw error;
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
    // Aliases for backwards compatibility
    displays: monitors,
    loadingDisplays: loadingMonitors,
    selectedCodec,
    checkServerStatus,
    startServer,
    stopServer,
    openUrl,
    copyUrl,
    loadMonitors,
    startStatusMonitoring,
    stopStatusMonitoring,
    // Relay exports
    relayStatus,
    discoveredRelays,
    relayLoading,
    discoverRelays,
    connectToRelay,
    disconnectFromRelay,
    autoConnectRelay,
    checkRelayStatus,
    setRelayAutoReconnect
  };
}
