use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::network::WebSocketServer;
use crate::network::relay_client::{RelayClient, RelayState, DiscoveredRelay};
use crate::lib::DEFAULT_SERVER_PORT;

/// Server configuration options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerOptions {
    pub delta_encoding: Option<bool>,
    pub adaptive_quality: Option<bool>,
    pub encryption: Option<bool>,
    pub webrtc: Option<bool>,
    pub hardware_accel: Option<bool>,
    pub monitor: Option<usize>,
}

/// Monitor information for the frontend
#[derive(Debug, Serialize)]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub is_primary: bool,
    pub width: usize,
    pub height: usize,
    pub position_x: i32,
    pub position_y: i32,
}

/// Relay connection status for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayStatus {
    pub connected: bool,
    pub relay_url: Option<String>,
    pub relay_hostname: Option<String>,
    pub state: String,
}

impl Default for RelayStatus {
    fn default() -> Self {
        Self {
            connected: false,
            relay_url: None,
            relay_hostname: None,
            state: "disconnected".to_string(),
        }
    }
}

/// Shared state between Tauri and WebSocket server
pub struct ServerState {
    pub runtime: Runtime,
    pub server_handle: Option<WebSocketServer>,
    pub port: u16,
    pub running: bool,
    pub options: ServerOptions,
    pub relay_client: Option<Arc<RwLock<RelayClient>>>,
    pub relay_status: RelayStatus,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
            server_handle: None,
            port: DEFAULT_SERVER_PORT,
            running: false,
            options: ServerOptions::default(),
            relay_client: None,
            relay_status: RelayStatus::default(),
        }
    }
}
