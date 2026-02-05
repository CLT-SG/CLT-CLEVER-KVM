//! Device Registry for managing connected KVM devices
//! 
//! This module handles registration, tracking, and management of devices
//! that publish their streams to the relay server.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Device capabilities reported during registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Supports H.264 encoding
    pub supports_h264: bool,
    /// Supports hardware acceleration
    pub supports_hw_accel: bool,
    /// Supports audio streaming
    pub supports_audio: bool,
    /// Available monitors count
    pub monitor_count: u32,
    /// Maximum supported resolution width
    pub max_width: u32,
    /// Maximum supported resolution height
    pub max_height: u32,
    /// Operating system (windows, linux, macos)
    pub os: String,
    /// App version
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
            os: String::from("unknown"),
            version: String::from("1.0.0"),
        }
    }
}

/// Current streaming state of a device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StreamState {
    /// Device is registered but not streaming
    Idle,
    /// Device is actively streaming
    Streaming,
    /// Device is paused
    Paused,
    /// Device encountered an error
    Error(String),
}

/// Stream configuration from a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Video width
    pub width: u32,
    /// Video height
    pub height: u32,
    /// Target framerate
    pub framerate: u32,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Codec being used
    pub codec: String,
    /// Active monitor index
    pub monitor_index: u32,
    /// FPS (alias for framerate, for compatibility)
    #[serde(default = "default_fps")]
    pub fps: u32,
}

fn default_fps() -> u32 {
    60
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            framerate: 60,
            bitrate_kbps: 6000,
            codec: String::from("h264"),
            monitor_index: 0,
            fps: 60,
        }
    }
}

/// A registered KVM device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Unique device ID
    pub id: String,
    /// Device hostname (e.g., "my-desktop")
    pub hostname: String,
    /// Display name for the device
    pub display_name: String,
    /// Device IP address (for direct connections if needed)
    #[serde(skip_serializing)]
    pub ip_address: String,
    /// WebSocket port on the device
    pub ws_port: u16,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Current stream configuration
    pub stream_config: StreamConfig,
    /// Current streaming state
    pub state: StreamState,
    /// Number of active viewers
    pub viewer_count: u32,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Last heartbeat timestamp
    pub last_seen: DateTime<Utc>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl Device {
    pub fn new(hostname: String, ip_address: String, ws_port: u16) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        
        Self {
            id,
            hostname: hostname.clone(),
            display_name: hostname,
            ip_address,
            ws_port,
            capabilities: DeviceCapabilities::default(),
            stream_config: StreamConfig::default(),
            state: StreamState::Idle,
            viewer_count: 0,
            registered_at: now,
            last_seen: now,
            metadata: HashMap::new(),
        }
    }
    
    /// Check if the device is considered online (heartbeat within timeout)
    pub fn is_online(&self, timeout_secs: i64) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.last_seen);
        elapsed.num_seconds() < timeout_secs
    }
    
    /// Update the last seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
    }
    
    /// Get the mDNS-style hostname (.local suffix for local network)
    pub fn local_hostname(&self) -> String {
        if self.hostname.ends_with(".local") {
            self.hostname.clone()
        } else {
            format!("{}.local", self.hostname)
        }
    }
}

/// Device summary for API responses (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub id: String,
    pub hostname: String,
    pub display_name: String,
    pub os: String,
    pub state: StreamState,
    pub viewer_count: u32,
    pub stream_config: StreamConfig,
    pub is_online: bool,
    pub last_seen: DateTime<Utc>,
}

impl From<&Device> for DeviceSummary {
    fn from(device: &Device) -> Self {
        Self {
            id: device.id.clone(),
            hostname: device.hostname.clone(),
            display_name: device.display_name.clone(),
            os: device.capabilities.os.clone(),
            state: device.state.clone(),
            viewer_count: device.viewer_count,
            stream_config: device.stream_config.clone(),
            is_online: device.is_online(30),
            last_seen: device.last_seen,
        }
    }
}

/// Active viewer connection
#[derive(Debug, Clone)]
pub struct Viewer {
    /// Unique viewer ID
    pub id: String,
    /// Viewer's remote address
    pub addr: SocketAddr,
    /// Device being viewed
    pub device_id: String,
    /// Connection timestamp
    pub connected_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// User agent (if available)
    pub user_agent: Option<String>,
}

impl Viewer {
    pub fn new(addr: SocketAddr, device_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            addr,
            device_id,
            connected_at: now,
            last_activity: now,
            user_agent: None,
        }
    }
}

/// Device Registry - manages all connected devices and viewers
pub struct DeviceRegistry {
    /// Registered devices by hostname
    devices: DashMap<String, Arc<RwLock<Device>>>,
    /// Device lookup by ID
    devices_by_id: DashMap<String, String>,
    /// Active viewers by viewer ID
    viewers: DashMap<String, Arc<RwLock<Viewer>>>,
    /// Viewers grouped by device hostname
    viewers_by_device: DashMap<String, Vec<String>>,
    /// Heartbeat timeout in seconds
    heartbeat_timeout: i64,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: DashMap::new(),
            devices_by_id: DashMap::new(),
            viewers: DashMap::new(),
            viewers_by_device: DashMap::new(),
            heartbeat_timeout: 30,
        }
    }
    
    /// Register a new device or update existing one
    pub async fn register_device(&self, mut device: Device) -> Device {
        let hostname = device.hostname.clone().to_lowercase();
        
        // Check if device already exists
        if let Some(existing) = self.devices.get(&hostname) {
            let mut existing_device = existing.write().await;
            existing_device.ip_address = device.ip_address;
            existing_device.ws_port = device.ws_port;
            existing_device.capabilities = device.capabilities;
            existing_device.state = StreamState::Idle;
            existing_device.touch();
            return existing_device.clone();
        }
        
        // Normalize hostname
        device.hostname = hostname.clone();
        let device_id = device.id.clone();
        
        let device_arc = Arc::new(RwLock::new(device.clone()));
        self.devices.insert(hostname.clone(), device_arc);
        self.devices_by_id.insert(device_id, hostname);
        
        device
    }
    
    /// Unregister a device
    pub async fn unregister_device(&self, hostname: &str) {
        let hostname = hostname.to_lowercase();
        
        if let Some((_, device)) = self.devices.remove(&hostname) {
            let device = device.read().await;
            self.devices_by_id.remove(&device.id);
            
            // Remove all viewers for this device
            if let Some((_, viewer_ids)) = self.viewers_by_device.remove(&hostname) {
                for viewer_id in viewer_ids {
                    self.viewers.remove(&viewer_id);
                }
            }
        }
    }
    
    /// Get a device by hostname
    pub async fn get_device(&self, hostname: &str) -> Option<Device> {
        let hostname = hostname.to_lowercase();
        if let Some(device) = self.devices.get(&hostname) {
            Some(device.read().await.clone())
        } else {
            None
        }
    }
    
    /// Get a device by ID
    pub async fn get_device_by_id(&self, id: &str) -> Option<Device> {
        if let Some(hostname) = self.devices_by_id.get(id) {
            self.get_device(&hostname).await
        } else {
            None
        }
    }
    
    /// Update device heartbeat
    pub async fn heartbeat(&self, hostname: &str) -> bool {
        let hostname = hostname.to_lowercase();
        if let Some(device) = self.devices.get(&hostname) {
            device.write().await.touch();
            true
        } else {
            false
        }
    }
    
    /// Update device stream state
    pub async fn update_stream_state(&self, hostname: &str, state: StreamState) {
        let hostname = hostname.to_lowercase();
        if let Some(device) = self.devices.get(&hostname) {
            device.write().await.state = state;
        }
    }
    
    /// Update device stream configuration
    pub async fn update_stream_config(&self, hostname: &str, config: StreamConfig) {
        let hostname = hostname.to_lowercase();
        if let Some(device) = self.devices.get(&hostname) {
            device.write().await.stream_config = config;
        }
    }
    
    /// Get all registered devices
    pub async fn list_devices(&self) -> Vec<DeviceSummary> {
        let mut devices = Vec::new();
        
        for entry in self.devices.iter() {
            let device = entry.read().await;
            devices.push(DeviceSummary::from(&*device));
        }
        
        devices
    }
    
    /// Get only online devices
    pub async fn list_online_devices(&self) -> Vec<DeviceSummary> {
        let mut devices = Vec::new();
        
        for entry in self.devices.iter() {
            let device = entry.read().await;
            if device.is_online(self.heartbeat_timeout) {
                devices.push(DeviceSummary::from(&*device));
            }
        }
        
        devices
    }
    
    /// Register a viewer for a device
    pub async fn add_viewer(&self, viewer: Viewer) -> String {
        let viewer_id = viewer.id.clone();
        let device_hostname = viewer.device_id.to_lowercase();
        
        // Increment viewer count on device
        if let Some(device) = self.devices.get(&device_hostname) {
            device.write().await.viewer_count += 1;
        }
        
        // Add to viewers map
        self.viewers.insert(viewer_id.clone(), Arc::new(RwLock::new(viewer)));
        
        // Track viewer by device
        self.viewers_by_device
            .entry(device_hostname)
            .or_insert_with(Vec::new)
            .push(viewer_id.clone());
        
        viewer_id
    }
    
    /// Remove a viewer
    pub async fn remove_viewer(&self, viewer_id: &str) {
        if let Some((_, viewer)) = self.viewers.remove(viewer_id) {
            let viewer = viewer.read().await;
            let device_hostname = viewer.device_id.to_lowercase();
            
            // Decrement viewer count on device
            if let Some(device) = self.devices.get(&device_hostname) {
                let mut device = device.write().await;
                device.viewer_count = device.viewer_count.saturating_sub(1);
            }
            
            // Remove from device tracking
            if let Some(mut viewers) = self.viewers_by_device.get_mut(&device_hostname) {
                viewers.retain(|id| id != viewer_id);
            }
        }
    }
    
    /// Get viewers for a device
    pub async fn get_viewers_for_device(&self, hostname: &str) -> Vec<Viewer> {
        let hostname = hostname.to_lowercase();
        let mut result = Vec::new();
        
        if let Some(viewer_ids) = self.viewers_by_device.get(&hostname) {
            for viewer_id in viewer_ids.iter() {
                if let Some(viewer) = self.viewers.get(viewer_id) {
                    result.push(viewer.read().await.clone());
                }
            }
        }
        
        result
    }
    
    /// Clean up stale devices that haven't sent heartbeats
    pub async fn cleanup_stale_devices(&self) -> Vec<String> {
        let mut removed = Vec::new();
        let mut to_remove = Vec::new();
        
        for entry in self.devices.iter() {
            let device = entry.read().await;
            if !device.is_online(self.heartbeat_timeout) {
                to_remove.push(device.hostname.clone());
            }
        }
        
        for hostname in to_remove {
            self.unregister_device(&hostname).await;
            removed.push(hostname);
        }
        
        removed
    }
    
    /// Get registry statistics
    pub async fn get_stats(&self) -> RegistryStats {
        let total_devices = self.devices.len();
        let total_viewers = self.viewers.len();
        
        let mut online_devices = 0;
        let mut streaming_devices = 0;
        
        for entry in self.devices.iter() {
            let device = entry.read().await;
            if device.is_online(self.heartbeat_timeout) {
                online_devices += 1;
                if device.state == StreamState::Streaming {
                    streaming_devices += 1;
                }
            }
        }
        
        RegistryStats {
            total_devices,
            online_devices,
            streaming_devices,
            total_viewers,
        }
    }
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_devices: usize,
    pub online_devices: usize,
    pub streaming_devices: usize,
    pub total_viewers: usize,
}
