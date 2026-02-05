//! Relay Client for CLEVER KVM Tauri App
//!
//! Handles auto-discovery of relay servers via mDNS and registration
//! of this device for remote viewing through the relay server.
//! 
//! Features auto-reconnection when relay server becomes unavailable.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// mDNS service type for relay server discovery
const RELAY_SERVICE_TYPE: &str = "_clever-kvm._tcp.local.";

/// Default relay server port
const DEFAULT_RELAY_PORT: u16 = 8881;

/// Heartbeat interval in seconds
const HEARTBEAT_INTERVAL: u64 = 10;

/// Initial reconnection delay in seconds
const INITIAL_RECONNECT_DELAY: u64 = 2;

/// Maximum reconnection delay in seconds (cap for exponential backoff)
const MAX_RECONNECT_DELAY: u64 = 60;

/// Maximum consecutive reconnection failures before giving up
const MAX_RECONNECT_ATTEMPTS: u32 = 0; // 0 = unlimited

/// Device capabilities for registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub supports_h264: bool,
    pub supports_hw_accel: bool,
    pub supports_audio: bool,
    pub monitor_count: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub os: String,
    pub version: String,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            supports_h264: true,
            supports_hw_accel: false,
            supports_audio: false,
            monitor_count: 1,
            max_width: 1920,
            max_height: 1080,
            os: std::env::consts::OS.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Stream start message to relay server
#[derive(Debug, Serialize)]
struct StreamStartMessage {
    #[serde(rename = "type")]
    msg_type: String,
    width: u32,
    height: u32,
    framerate: u32,
    bitrate_kbps: u32,
    codec: String,
}

/// Registration request to relay server
#[derive(Debug, Serialize)]
pub struct RegisterRequest {
    pub hostname: String,
    pub display_name: Option<String>,
    pub ip_address: String,
    pub ws_port: u16,
    pub capabilities: DeviceCapabilities,
}

/// Registration response from relay server
#[derive(Debug, Deserialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub device_id: String,
    pub message: String,
}

/// Discovered relay server
#[derive(Debug, Clone)]
pub struct DiscoveredRelay {
    pub hostname: String,
    pub port: u16,
    pub addresses: Vec<String>,
    pub url: String,
}

/// Relay connection state
#[derive(Debug, Clone, PartialEq)]
pub enum RelayState {
    Disconnected,
    Discovering,
    Connecting,
    Connected,
    Streaming,
    Reconnecting,
    Error(String),
}

/// Auto-reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Whether auto-reconnect is enabled
    pub enabled: bool,
    /// Initial delay before first reconnection attempt (seconds)
    pub initial_delay: u64,
    /// Maximum delay between reconnection attempts (seconds)
    pub max_delay: u64,
    /// Maximum reconnection attempts (0 = unlimited)
    pub max_attempts: u32,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: INITIAL_RECONNECT_DELAY,
            max_delay: MAX_RECONNECT_DELAY,
            max_attempts: MAX_RECONNECT_ATTEMPTS,
        }
    }
}

/// Relay client for connecting to relay servers
pub struct RelayClient {
    /// Current relay server
    relay: Arc<RwLock<Option<DiscoveredRelay>>>,
    /// Connection state
    state: Arc<RwLock<RelayState>>,
    /// Device hostname
    hostname: String,
    /// Device display name
    display_name: String,
    /// Device capabilities
    capabilities: DeviceCapabilities,
    /// Local WebSocket port
    local_ws_port: u16,
    /// Frame sender for streaming
    frame_tx: Option<mpsc::Sender<Vec<u8>>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Auto-reconnection configuration
    reconnect_config: Arc<RwLock<ReconnectConfig>>,
    /// Flag to signal reconnection task to stop
    reconnect_shutdown: Arc<AtomicBool>,
    /// Current reconnection attempt count
    reconnect_attempts: Arc<AtomicU32>,
    /// Last known relay for reconnection
    last_relay: Arc<RwLock<Option<DiscoveredRelay>>>,
}

impl RelayClient {
    /// Create a new relay client
    pub fn new(hostname: String, display_name: String, local_ws_port: u16) -> Self {
        Self {
            relay: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(RelayState::Disconnected)),
            hostname,
            display_name,
            capabilities: DeviceCapabilities::default(),
            local_ws_port,
            frame_tx: None,
            shutdown_tx: None,
            reconnect_config: Arc::new(RwLock::new(ReconnectConfig::default())),
            reconnect_shutdown: Arc::new(AtomicBool::new(false)),
            reconnect_attempts: Arc::new(AtomicU32::new(0)),
            last_relay: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Set device capabilities
    pub fn set_capabilities(&mut self, caps: DeviceCapabilities) {
        self.capabilities = caps;
    }
    
    /// Configure auto-reconnection settings
    pub async fn set_reconnect_config(&self, config: ReconnectConfig) {
        *self.reconnect_config.write().await = config;
    }
    
    /// Enable or disable auto-reconnection
    pub async fn set_auto_reconnect(&self, enabled: bool) {
        self.reconnect_config.write().await.enabled = enabled;
        if !enabled {
            // Stop any ongoing reconnection attempts
            self.reconnect_shutdown.store(true, Ordering::SeqCst);
        }
    }
    
    /// Check if auto-reconnect is enabled
    pub async fn is_auto_reconnect_enabled(&self) -> bool {
        self.reconnect_config.read().await.enabled
    }
    
    /// Get current connection state
    pub async fn get_state(&self) -> RelayState {
        self.state.read().await.clone()
    }
    
    /// Get connected relay info
    pub async fn get_relay(&self) -> Option<DiscoveredRelay> {
        self.relay.read().await.clone()
    }
    
    /// Get current reconnection attempt count
    pub fn get_reconnect_attempts(&self) -> u32 {
        self.reconnect_attempts.load(Ordering::SeqCst)
    }
    
    /// Discover relay servers on the network
    pub async fn discover_relays(&self, timeout_ms: u64) -> Vec<DiscoveredRelay> {
        *self.state.write().await = RelayState::Discovering;
        
        let mut relays = Vec::new();
        
        // Try mDNS discovery
        #[cfg(feature = "mdns")]
        {
            match mdns_sd::ServiceDaemon::new() {
                Ok(daemon) => {
                    if let Ok(receiver) = daemon.browse(RELAY_SERVICE_TYPE) {
                        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
                        
                        while std::time::Instant::now() < deadline {
                            match receiver.recv_timeout(Duration::from_millis(100)) {
                                Ok(event) => {
                                    if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                                        let relay = DiscoveredRelay {
                                            hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                            port: info.get_port(),
                                            addresses: info.get_addresses().iter().map(|a| a.to_string()).collect(),
                                            url: format!("http://{}:{}", 
                                                info.get_addresses().iter().next()
                                                    .map(|a| a.to_string())
                                                    .unwrap_or_else(|| info.get_hostname().to_string()),
                                                info.get_port()
                                            ),
                                        };
                                        
                                        if !relays.iter().any(|r: &DiscoveredRelay| r.hostname == relay.hostname) {
                                            info!("📡 Discovered relay server: {}", relay.hostname);
                                            relays.push(relay);
                                        }
                                    }
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("mDNS discovery not available: {}", e);
                }
            }
        }
        
        // If no relays found via mDNS, try common local addresses
        if relays.is_empty() {
            info!("📡 No mDNS relays found, trying common addresses...");
            
            let common_addresses = vec![
                "localhost",
                "127.0.0.1",
            ];
            
            for addr in common_addresses {
                let url = format!("http://{}:{}/api/health", addr, DEFAULT_RELAY_PORT);
                if let Ok(response) = reqwest_lite_check(&url).await {
                    if response {
                        relays.push(DiscoveredRelay {
                            hostname: addr.to_string(),
                            port: DEFAULT_RELAY_PORT,
                            addresses: vec![addr.to_string()],
                            url: format!("http://{}:{}", addr, DEFAULT_RELAY_PORT),
                        });
                        info!("📡 Found relay server at: {}:{}", addr, DEFAULT_RELAY_PORT);
                    }
                }
            }
        }
        
        *self.state.write().await = RelayState::Disconnected;
        relays
    }
    
    /// Connect to a relay server
    pub async fn connect(&mut self, relay: DiscoveredRelay) -> Result<(), String> {
        *self.state.write().await = RelayState::Connecting;
        
        // Reset reconnection state
        self.reconnect_shutdown.store(false, Ordering::SeqCst);
        self.reconnect_attempts.store(0, Ordering::SeqCst);
        
        // Store the relay for potential reconnection
        *self.last_relay.write().await = Some(relay.clone());
        
        // Register with the relay server
        let register_url = format!("{}/api/register", relay.url);
        let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        
        let request = RegisterRequest {
            hostname: self.hostname.clone(),
            display_name: Some(self.display_name.clone()),
            ip_address: local_ip,
            ws_port: self.local_ws_port,
            capabilities: self.capabilities.clone(),
        };
        
        // Send registration request
        let client = reqwest::Client::new();
        match client.post(&register_url)
            .json(&request)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    if let Ok(reg_response) = response.json::<RegisterResponse>().await {
                        if reg_response.success {
                            info!("✅ Registered with relay server: {} (ID: {})", 
                                  relay.hostname, reg_response.device_id);
                            
                            *self.relay.write().await = Some(relay.clone());
                            *self.state.write().await = RelayState::Connected;
                            
                            // Start heartbeat task with auto-reconnection
                            self.start_heartbeat_with_reconnect(relay.clone());
                            
                            return Ok(());
                        } else {
                            return Err(reg_response.message);
                        }
                    }
                }
                Err(format!("Registration failed: HTTP {}", status))
            }
            Err(e) => {
                *self.state.write().await = RelayState::Error(e.to_string());
                Err(format!("Failed to connect to relay: {}", e))
            }
        }
    }
    
    /// Connect via WebSocket for streaming
    pub async fn connect_ws(&mut self) -> Result<mpsc::Sender<Vec<u8>>, String> {
        self.connect_ws_with_config(1920, 1080, 60, 6000).await
    }
    
    /// Connect via WebSocket for streaming with specific configuration
    pub async fn connect_ws_with_config(
        &mut self,
        width: u32,
        height: u32,
        framerate: u32,
        bitrate_kbps: u32,
    ) -> Result<mpsc::Sender<Vec<u8>>, String> {
        let relay = self.relay.read().await.clone()
            .ok_or("Not connected to relay server")?;
        
        let ws_url = format!("ws://{}:{}/ws/device/{}", 
            relay.addresses.first().unwrap_or(&relay.hostname),
            relay.port,
            self.hostname
        );
        
        info!("🔌 Connecting WebSocket to relay: {}", ws_url);
        
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                let (mut ws_tx, mut ws_rx) = ws_stream.split();
                let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(100);
                let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
                
                // Send stream_start message to notify relay server of stream configuration
                let stream_start = StreamStartMessage {
                    msg_type: "stream_start".to_string(),
                    width,
                    height,
                    framerate,
                    bitrate_kbps,
                    codec: "h264".to_string(),
                };
                
                if let Ok(json) = serde_json::to_string(&stream_start) {
                    if let Err(e) = ws_tx.send(Message::Text(json)).await {
                        warn!("Failed to send stream_start message: {}", e);
                    } else {
                        info!("📺 Sent stream_start: {}x{} @ {}fps, {} kbps", 
                              width, height, framerate, bitrate_kbps);
                    }
                }
                
                let state = self.state.clone();
                let hostname = self.hostname.clone();
                
                // Task to send frames
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            Some(frame) = frame_rx.recv() => {
                                if ws_tx.send(Message::Binary(frame)).await.is_err() {
                                    break;
                                }
                            }
                            _ = shutdown_rx.recv() => {
                                break;
                            }
                        }
                    }
                    debug!("Frame sender task ended for {}", hostname);
                });
                
                // Task to receive input events
                let state_clone = state.clone();
                tokio::spawn(async move {
                    while let Some(msg) = ws_rx.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                // Handle input events from viewers
                                debug!("Received input event: {}", text);
                                // TODO: Forward to input handler
                            }
                            Ok(Message::Binary(data)) => {
                                // Handle binary data (if needed)
                                debug!("Received binary data: {} bytes", data.len());
                            }
                            Ok(Message::Close(_)) => {
                                info!("WebSocket closed by relay server");
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    *state_clone.write().await = RelayState::Disconnected;
                });
                
                *self.state.write().await = RelayState::Streaming;
                self.frame_tx = Some(frame_tx.clone());
                self.shutdown_tx = Some(shutdown_tx);
                
                Ok(frame_tx)
            }
            Err(e) => {
                *self.state.write().await = RelayState::Error(e.to_string());
                Err(format!("WebSocket connection failed: {}", e))
            }
        }
    }
    
    /// Send a video frame to the relay
    pub async fn send_frame(&self, frame: Vec<u8>) -> Result<(), String> {
        if let Some(ref tx) = self.frame_tx {
            tx.send(frame).await.map_err(|e| e.to_string())
        } else {
            Err("Not connected for streaming".to_string())
        }
    }
    
    /// Start heartbeat task (legacy, without auto-reconnect)
    fn start_heartbeat(&self, relay: DiscoveredRelay) {
        let hostname = self.hostname.clone();
        let state = self.state.clone();
        
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let heartbeat_url = format!("{}/api/heartbeat/{}", relay.url, hostname);
            
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL));
            
            loop {
                interval.tick().await;
                
                // Check if still connected
                if *state.read().await == RelayState::Disconnected {
                    break;
                }
                
                // Send heartbeat
                match client.post(&heartbeat_url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if !response.status().is_success() {
                            warn!("Heartbeat failed: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        warn!("Heartbeat error: {}", e);
                    }
                }
            }
            
            debug!("Heartbeat task ended for {}", hostname);
        });
    }
    
    /// Start heartbeat task with auto-reconnection support
    fn start_heartbeat_with_reconnect(&self, relay: DiscoveredRelay) {
        let hostname = self.hostname.clone();
        let display_name = self.display_name.clone();
        let capabilities = self.capabilities.clone();
        let local_ws_port = self.local_ws_port;
        let state = self.state.clone();
        let relay_arc = self.relay.clone();
        let reconnect_config = self.reconnect_config.clone();
        let reconnect_shutdown = self.reconnect_shutdown.clone();
        let reconnect_attempts = self.reconnect_attempts.clone();
        let last_relay = self.last_relay.clone();
        
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut consecutive_failures: u32 = 0;
            let max_consecutive_failures: u32 = 3;
            
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL));
            
            loop {
                interval.tick().await;
                
                // Check if shutdown was requested
                if reconnect_shutdown.load(Ordering::SeqCst) {
                    debug!("Heartbeat task shutdown requested for {}", hostname);
                    break;
                }
                
                // Check current state
                let current_state = state.read().await.clone();
                if current_state == RelayState::Disconnected {
                    break;
                }
                
                // Skip heartbeat during reconnection
                if current_state == RelayState::Reconnecting {
                    continue;
                }
                
                // Get current relay URL
                let relay_info = relay_arc.read().await.clone();
                let heartbeat_url = match &relay_info {
                    Some(r) => format!("{}/api/heartbeat/{}", r.url, hostname),
                    None => {
                        debug!("No relay configured, skipping heartbeat");
                        continue;
                    }
                };
                
                // Send heartbeat
                match client.post(&heartbeat_url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            // Reset failure counter on success
                            consecutive_failures = 0;
                            reconnect_attempts.store(0, Ordering::SeqCst);
                        } else {
                            warn!("Heartbeat failed: HTTP {}", response.status());
                            consecutive_failures += 1;
                        }
                    }
                    Err(e) => {
                        warn!("Heartbeat error: {}", e);
                        consecutive_failures += 1;
                    }
                }
                
                // Check if we need to trigger reconnection
                if consecutive_failures >= max_consecutive_failures {
                    info!("🔄 Connection lost to relay server after {} failed heartbeats", consecutive_failures);
                    
                    let config = reconnect_config.read().await.clone();
                    if config.enabled {
                        // Trigger reconnection
                        let hostname_clone = hostname.clone();
                        let display_name_clone = display_name.clone();
                        let capabilities_clone = capabilities.clone();
                        let state_clone = state.clone();
                        let relay_arc_clone = relay_arc.clone();
                        let reconnect_shutdown_clone = reconnect_shutdown.clone();
                        let reconnect_attempts_clone = reconnect_attempts.clone();
                        let last_relay_clone = last_relay.clone();
                        let config_clone = config.clone();
                        
                        tokio::spawn(async move {
                            attempt_reconnection(
                                hostname_clone,
                                display_name_clone,
                                capabilities_clone,
                                local_ws_port,
                                state_clone,
                                relay_arc_clone,
                                reconnect_shutdown_clone,
                                reconnect_attempts_clone,
                                last_relay_clone,
                                config_clone,
                            ).await;
                        });
                        
                        // Reset failure counter after triggering reconnection
                        consecutive_failures = 0;
                    } else {
                        // Auto-reconnect disabled, just mark as disconnected
                        *state.write().await = RelayState::Disconnected;
                        *relay_arc.write().await = None;
                        info!("📡 Relay connection lost (auto-reconnect disabled)");
                        break;
                    }
                }
            }
            
            debug!("Heartbeat task ended for {}", hostname);
        });
    }
    
    /// Disconnect from relay server
    pub async fn disconnect(&mut self) {
        // Stop reconnection attempts
        self.reconnect_shutdown.store(true, Ordering::SeqCst);
        
        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        
        // Unregister from relay
        if let Some(relay) = self.relay.read().await.clone() {
            let unregister_url = format!("{}/api/unregister/{}", relay.url, self.hostname);
            let client = reqwest::Client::new();
            let _ = client.post(&unregister_url)
                .timeout(Duration::from_secs(2))
                .send()
                .await;
        }
        
        *self.state.write().await = RelayState::Disconnected;
        *self.relay.write().await = None;
        *self.last_relay.write().await = None;
        self.frame_tx = None;
        
        info!("📡 Disconnected from relay server");
    }
}

/// Attempt to reconnect to the relay server with exponential backoff
async fn attempt_reconnection(
    hostname: String,
    display_name: String,
    capabilities: DeviceCapabilities,
    local_ws_port: u16,
    state: Arc<RwLock<RelayState>>,
    relay_arc: Arc<RwLock<Option<DiscoveredRelay>>>,
    reconnect_shutdown: Arc<AtomicBool>,
    reconnect_attempts: Arc<AtomicU32>,
    last_relay: Arc<RwLock<Option<DiscoveredRelay>>>,
    config: ReconnectConfig,
) {
    // Set state to reconnecting
    *state.write().await = RelayState::Reconnecting;
    
    let client = reqwest::Client::new();
    let mut current_delay = config.initial_delay;
    
    loop {
        // Check if shutdown was requested
        if reconnect_shutdown.load(Ordering::SeqCst) {
            info!("🔄 Reconnection cancelled");
            *state.write().await = RelayState::Disconnected;
            return;
        }
        
        let attempt = reconnect_attempts.fetch_add(1, Ordering::SeqCst) + 1;
        
        // Check max attempts (0 = unlimited)
        if config.max_attempts > 0 && attempt > config.max_attempts {
            error!("❌ Max reconnection attempts ({}) reached, giving up", config.max_attempts);
            *state.write().await = RelayState::Error("Max reconnection attempts reached".to_string());
            return;
        }
        
        info!("🔄 Reconnection attempt {} (delay: {}s)", attempt, current_delay);
        
        // Wait before attempting
        tokio::time::sleep(Duration::from_secs(current_delay)).await;
        
        // Check again after sleep
        if reconnect_shutdown.load(Ordering::SeqCst) {
            info!("🔄 Reconnection cancelled during wait");
            *state.write().await = RelayState::Disconnected;
            return;
        }
        
        // Get the last known relay
        let relay = match last_relay.read().await.clone() {
            Some(r) => r,
            None => {
                warn!("No relay information available for reconnection");
                *state.write().await = RelayState::Error("No relay information".to_string());
                return;
            }
        };
        
        // First, check if the relay server is up
        let health_url = format!("{}/api/health", relay.url);
        let health_check = client.get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        
        if health_check.is_err() || !health_check.unwrap().status().is_success() {
            debug!("Relay server not available yet, will retry...");
            // Exponential backoff with cap
            current_delay = std::cmp::min(current_delay * 2, config.max_delay);
            continue;
        }
        
        // Relay is up, attempt to register
        let register_url = format!("{}/api/register", relay.url);
        let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        
        let request = RegisterRequest {
            hostname: hostname.clone(),
            display_name: Some(display_name.clone()),
            ip_address: local_ip,
            ws_port: local_ws_port,
            capabilities: capabilities.clone(),
        };
        
        match client.post(&register_url)
            .json(&request)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(reg_response) = response.json::<RegisterResponse>().await {
                        if reg_response.success {
                            info!("✅ Successfully reconnected to relay server: {} (ID: {})", 
                                  relay.hostname, reg_response.device_id);
                            
                            // Update state
                            *relay_arc.write().await = Some(relay.clone());
                            *state.write().await = RelayState::Connected;
                            reconnect_attempts.store(0, Ordering::SeqCst);
                            
                            return;
                        }
                    }
                }
                warn!("Registration failed during reconnection, will retry...");
            }
            Err(e) => {
                warn!("Reconnection attempt failed: {}", e);
            }
        }
        
        // Exponential backoff with cap
        current_delay = std::cmp::min(current_delay * 2, config.max_delay);
    }
}

/// Simple health check without full reqwest
async fn reqwest_lite_check(url: &str) -> Result<bool, ()> {
    // Use tokio's TcpStream for a simple connection check
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let url = url.replace("http://", "").replace("/api/health", "");
    let parts: Vec<&str> = url.split(':').collect();
    
    if parts.len() != 2 {
        return Err(());
    }
    
    let addr = format!("{}:{}", parts[0], parts[1]);
    
    match TcpStream::connect(&addr).await {
        Ok(mut stream) => {
            let request = format!(
                "GET /api/health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                parts[0]
            );
            
            if stream.write_all(request.as_bytes()).await.is_err() {
                return Err(());
            }
            
            let mut response = [0u8; 1024];
            if let Ok(n) = stream.read(&mut response).await {
                let response_str = String::from_utf8_lossy(&response[..n]);
                return Ok(response_str.contains("200 OK") || response_str.contains("\"status\":\"ok\""));
            }
            
            Err(())
        }
        Err(_) => Err(())
    }
}

/// Get local IP address
fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

/// Get system hostname
pub fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_lowercase())
        .unwrap_or_else(|_| "unknown".to_string())
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}
