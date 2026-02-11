import { ref, reactive, computed, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/tauri";

export function useServer() {
  const serverStatus = ref(false);
  const serverUrl = ref("");
  const serverPort = ref(9921);
  const loading = ref(false);
  const errorMessage = ref("");
  const monitors = ref([]);
  const loadingMonitors = ref(false);

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

  // Server settings
  const settings = reactive({
    deltaEncoding: true,
    adaptiveQuality: true,
    encryptionEnabled: false,
    useWebRTC: true,
    hardwareAcceleration: true,
    selectedMonitor: 0,
    audioBitrate: 128,
    videoBitrate: 4000,
    framerate: 30
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
      const status = await invoke("get_server_status");
      serverStatus.value = status;
      
      if (status) {
        try {
          const url = await invoke("get_server_url");
          serverUrl.value = url;
        } catch (urlError) {
          console.warn("Failed to get server URL:", urlError);
          // If we can get status but not URL, something might be wrong
          serverStatus.value = false;
          serverUrl.value = "";
        }
      } else {
        serverUrl.value = "";
      }
      
      await loadMonitors();
      await checkRelayStatus();
    } catch (error) {
      console.error("Failed to check server status:", error);
      errorMessage.value = `Failed to check server status: ${error}`;
      serverStatus.value = false;
      serverUrl.value = "";
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
      const codec = selectedCodec.value;
      
      const url = await invoke("start_server", { 
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
      
      serverUrl.value = url;
      serverStatus.value = true;
      
      // Double-check the server status after starting
      setTimeout(async () => {
        await checkServerStatus();
      }, 1000);
      
    } catch (error) {
      console.error("Failed to start server:", error);
      errorMessage.value = `Failed to start server: ${error}`;
      serverStatus.value = false;
      serverUrl.value = "";
    } finally {
      loading.value = false;
    }
  }

  async function stopServer() {
    loading.value = true;
    errorMessage.value = "";
    
    try {
      await invoke("stop_server");
      serverStatus.value = false;
      serverUrl.value = "";
      
      // Double-check the server status after stopping
      setTimeout(async () => {
        await checkServerStatus();
      }, 1000);
      
    } catch (error) {
      console.error("Failed to stop server:", error);
      errorMessage.value = `Failed to stop server: ${error}`;
    } finally {
      loading.value = false;
    }
  }

  function buildUrlWithParams() {
    if (!serverUrl.value) return "";
    
    let url = serverUrl.value;
    // Ensure the URL ends with /kvm for the KVM client
    if (!url.endsWith('/kvm')) {
      url = url.replace(/\/$/, '') + '/kvm';
    }
    
    const params = [];
    
    if (settings.useWebRTC) {
      params.push('audio=true');
    }
    
    if (settings.encryptionEnabled) {
      params.push('encryption=true');
    }
    
    params.push(`codec=${selectedCodec.value}`);
    
    if (settings.selectedMonitor > 0) {
      params.push(`monitor=${settings.selectedMonitor}`);
    }
    
    if (params.length > 0) {
      url += (url.includes('?') ? ';' : '?') + params.join(';');
    }
    
    return url;
  }

  function openUrl() {
    const url = buildUrlWithParams();
    if (url) {
      window.open(url, '_blank');
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
  checkServerStatus().then(() => {
    startStatusMonitoring();
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
