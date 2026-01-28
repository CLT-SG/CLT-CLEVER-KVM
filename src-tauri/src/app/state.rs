use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::lib::DEFAULT_SERVER_PORT;
use crate::vnc::{VncKvmServer, ScreencastRegistration};

/// Server configuration options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerOptions {
    pub delta_encoding: Option<bool>,
    pub adaptive_quality: Option<bool>,
    pub encryption: Option<bool>,
    pub webrtc: Option<bool>,
    pub vp8: Option<bool>,
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

/// Shared state for VNC server management
pub struct ServerState {
    #[allow(dead_code)]
    pub runtime: Runtime,
    pub port: u16,
    pub running: bool,
    #[allow(dead_code)]
    pub options: ServerOptions,
    pub vnc_servers: Vec<Arc<Mutex<VncKvmServer>>>,
    pub vnc_registration: Option<ScreencastRegistration>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
            port: DEFAULT_SERVER_PORT,
            running: false,
            options: ServerOptions::default(),
            vnc_servers: Vec::new(),
            vnc_registration: None,
        }
    }
}
