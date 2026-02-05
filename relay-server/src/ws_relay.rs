//! WebSocket Relay Handler
//!
//! Manages WebSocket connections between devices (Tauri apps) and viewers (browsers).
//! Relays video frames from devices to viewers and input events from viewers to devices.

use crate::device::{DeviceRegistry, StreamConfig, StreamState, Viewer};
use actix_ws::{Message, Session};
use dashmap::DashMap;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

/// Maximum message size (64MB for large frames)
const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

// ============================================================================
// WebSocket Relay State
// ============================================================================

/// WebSocket relay state
pub struct WsRelayState {
    /// Device connections by hostname
    device_connections: DashMap<String, DeviceConnection>,
    /// Viewer connections by viewer ID
    viewer_connections: DashMap<String, ViewerConnection>,
    /// Broadcast channels per device (for video frames)
    device_broadcasts: DashMap<String, broadcast::Sender<Vec<u8>>>,
}

impl WsRelayState {
    pub fn new() -> Self {
        Self {
            device_connections: DashMap::new(),
            viewer_connections: DashMap::new(),
            device_broadcasts: DashMap::new(),
        }
    }

    /// Get or create a broadcast channel for a device
    pub fn get_device_broadcast(&self, hostname: &str) -> broadcast::Sender<Vec<u8>> {
        self.device_broadcasts
            .entry(hostname.to_lowercase())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(16); // Buffer 16 frames
                tx
            })
            .clone()
    }

    /// Subscribe to a device's video stream
    pub fn subscribe_to_device(&self, hostname: &str) -> Option<broadcast::Receiver<Vec<u8>>> {
        self.device_broadcasts
            .get(&hostname.to_lowercase())
            .map(|tx| tx.subscribe())
    }

    /// Check if a device is connected
    pub fn is_device_connected(&self, hostname: &str) -> bool {
        self.device_connections
            .contains_key(&hostname.to_lowercase())
    }

    /// Get viewer count for a device
    pub fn get_viewer_count(&self, hostname: &str) -> usize {
        let hostname = hostname.to_lowercase();
        self.viewer_connections
            .iter()
            .filter(|v| v.device_hostname == hostname)
            .count()
    }
}

impl Default for WsRelayState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Connection State Types
// ============================================================================

/// Device connection state
struct DeviceConnection {
    hostname: String,
    input_tx: mpsc::Sender<Vec<u8>>,
}

/// Viewer connection state
struct ViewerConnection {
    viewer_id: String,
    device_hostname: String,
}

// ============================================================================
// Message Types
// ============================================================================

/// Control message from device
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DeviceMessage {
    #[serde(rename = "register")]
    Register {
        hostname: String,
        display_name: Option<String>,
        capabilities: Option<serde_json::Value>,
    },
    #[serde(rename = "stream_start")]
    StreamStart {
        width: u32,
        height: u32,
        framerate: u32,
        bitrate_kbps: u32,
        codec: String,
    },
    #[serde(rename = "stream_stop")]
    StreamStop,
    #[serde(rename = "heartbeat")]
    Heartbeat,
    #[serde(rename = "stream_config")]
    StreamConfig {
        width: u32,
        height: u32,
        framerate: u32,
        bitrate_kbps: u32,
    },
}

/// Control message from viewer
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ViewerMessage {
    #[serde(rename = "ping")]
    Ping { timestamp: u64 },
    #[serde(rename = "mousemove")]
    MouseMove { x: i32, y: i32 },
    #[serde(rename = "mousedown")]
    MouseDown { x: i32, y: i32, button: String },
    #[serde(rename = "mouseup")]
    MouseUp { x: i32, y: i32, button: String },
    #[serde(rename = "wheel")]
    Wheel {
        x: i32,
        y: i32,
        delta_x: f64,
        delta_y: f64,
    },
    #[serde(rename = "keydown")]
    KeyDown { key: String, key_code: Option<u32> },
    #[serde(rename = "keyup")]
    KeyUp { key: String, key_code: Option<u32> },
    #[serde(rename = "request_keyframe")]
    RequestKeyframe,
    #[serde(rename = "quality_setting")]
    QualitySetting { quality: u8 },
}

/// Message to send to viewer
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ViewerOutMessage {
    #[serde(rename = "connected")]
    Connected {
        device_hostname: String,
        device_name: String,
        width: u32,
        height: u32,
    },
    #[serde(rename = "stream_init")]
    StreamInit {
        width: u32,
        height: u32,
        framerate: u32,
        codec: String,
    },
    #[serde(rename = "pong")]
    Pong { timestamp: u64 },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "device_offline")]
    DeviceOffline,
}

// ============================================================================
// Device Connection Handler
// ============================================================================

/// Handle device WebSocket connection
pub async fn handle_device_connection(
    mut session: Session,
    mut msg_stream: actix_ws::MessageStream,
    hostname: String,
    registry: Arc<DeviceRegistry>,
    ws_state: Arc<WsRelayState>,
) {
    let hostname = hostname.to_lowercase();
    info!("🔌 Device connected: {}", hostname);

    // Create input channel for receiving input events from viewers
    let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(100);

    // Register device connection
    ws_state.device_connections.insert(
        hostname.clone(),
        DeviceConnection {
            hostname: hostname.clone(),
            input_tx,
        },
    );

    // Get broadcast channel for this device
    let broadcast_tx = ws_state.get_device_broadcast(&hostname);

    // Update device state to streaming
    registry
        .update_stream_state(&hostname, StreamState::Streaming)
        .await;

    // Clone session for input forwarding task
    let mut session_clone = session.clone();
    let hostname_for_task = hostname.clone();

    // Task to forward input events to device
    let input_forward_task = tokio::spawn(async move {
        while let Some(data) = input_rx.recv().await {
            if session_clone.binary(data).await.is_err() {
                break;
            }
        }
        debug!("Input forward task ended for {}", hostname_for_task);
    });

    // Process messages from device
    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                // Video frame - broadcast to all viewers
                let _ = broadcast_tx.send(data.to_vec());
            }
            Ok(Message::Text(text)) => {
                // Control message
                if let Ok(msg) = serde_json::from_str::<DeviceMessage>(&text) {
                    match msg {
                        DeviceMessage::Heartbeat => {
                            registry.heartbeat(&hostname).await;
                        }
                        DeviceMessage::StreamStart {
                            width,
                            height,
                            framerate,
                            bitrate_kbps,
                            codec,
                        } => {
                            let config = StreamConfig {
                                width,
                                height,
                                framerate,
                                bitrate_kbps,
                                codec,
                                monitor_index: 0,
                                fps: framerate,
                            };
                            registry.update_stream_config(&hostname, config).await;
                            registry
                                .update_stream_state(&hostname, StreamState::Streaming)
                                .await;
                            info!(
                                "📺 Device {} started streaming: {}x{} @ {}fps",
                                hostname, width, height, framerate
                            );
                        }
                        DeviceMessage::StreamStop => {
                            registry
                                .update_stream_state(&hostname, StreamState::Idle)
                                .await;
                            info!("⏹️ Device {} stopped streaming", hostname);
                        }
                        DeviceMessage::StreamConfig {
                            width,
                            height,
                            framerate,
                            bitrate_kbps,
                        } => {
                            let config = StreamConfig {
                                width,
                                height,
                                framerate,
                                bitrate_kbps,
                                codec: "h264".to_string(),
                                monitor_index: 0,
                                fps: framerate,
                            };
                            registry.update_stream_config(&hostname, config).await;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = session.pong(&data).await;
                debug!("Ping from device {}", hostname);
            }
            Ok(Message::Close(_)) => {
                info!("🔌 Device {} sent close frame", hostname);
                break;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Nop) | Ok(Message::Continuation(_)) => {}
            Err(e) => {
                error!("Device {} WebSocket error: {:?}", hostname, e);
                break;
            }
        }
    }

    // Cleanup
    input_forward_task.abort();
    ws_state.device_connections.remove(&hostname);
    ws_state.device_broadcasts.remove(&hostname);
    registry
        .update_stream_state(&hostname, StreamState::Idle)
        .await;

    info!("🔌 Device disconnected: {}", hostname);
}

// ============================================================================
// Viewer Connection Handler
// ============================================================================

/// Handle viewer WebSocket connection
pub async fn handle_viewer_connection(
    mut session: Session,
    mut msg_stream: actix_ws::MessageStream,
    hostname: String,
    registry: Arc<DeviceRegistry>,
    ws_state: Arc<WsRelayState>,
) {
    let hostname = hostname.to_lowercase();
    let viewer_id = uuid::Uuid::new_v4().to_string();

    info!("👁️ Viewer {} connecting to device: {}", viewer_id, hostname);

    // Check if device exists and is online
    let device = match registry.get_device(&hostname).await {
        Some(d) if d.is_online(30) => d,
        Some(_) => {
            error!("Device {} is offline", hostname);
            let _ = send_error(&mut session, "Device is offline").await;
            return;
        }
        None => {
            error!("Device {} not found", hostname);
            let _ = send_error(&mut session, "Device not found").await;
            return;
        }
    };

    // Subscribe to device's video stream
    let mut video_rx = match ws_state.subscribe_to_device(&hostname) {
        Some(rx) => rx,
        None => {
            error!("Device {} is not streaming", hostname);
            let _ = send_error(&mut session, "Device is not streaming").await;
            return;
        }
    };

    // Register viewer
    let viewer = Viewer::new(SocketAddr::from(([0, 0, 0, 0], 0)), hostname.clone());
    let viewer_id = registry.add_viewer(viewer).await;

    ws_state.viewer_connections.insert(
        viewer_id.clone(),
        ViewerConnection {
            viewer_id: viewer_id.clone(),
            device_hostname: hostname.clone(),
        },
    );

    // Send initial connected message
    let connected_msg = ViewerOutMessage::Connected {
        device_hostname: device.hostname.clone(),
        device_name: device.display_name.clone(),
        width: device.stream_config.width,
        height: device.stream_config.height,
    };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = session.text(json).await;
    }

    // Send stream init message
    let init_msg = ViewerOutMessage::StreamInit {
        width: device.stream_config.width,
        height: device.stream_config.height,
        framerate: device.stream_config.framerate,
        codec: device.stream_config.codec.clone(),
    };
    if let Ok(json) = serde_json::to_string(&init_msg) {
        let _ = session.text(json).await;
    }

    // Get input channel to device
    let input_tx = ws_state
        .device_connections
        .get(&hostname)
        .map(|c| c.input_tx.clone());

    // Clone session for video forwarding task
    let mut session_for_video = session.clone();
    let viewer_id_for_task = viewer_id.clone();

    // Task to forward video frames to viewer
    let video_forward_task = tokio::spawn(async move {
        loop {
            match video_rx.recv().await {
                Ok(data) => {
                    if session_for_video.binary(data).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Viewer {} lagged, skipped {} frames", viewer_id_for_task, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Device disconnected
                    let offline_msg = ViewerOutMessage::DeviceOffline;
                    if let Ok(json) = serde_json::to_string(&offline_msg) {
                        let _ = session_for_video.text(json).await;
                    }
                    break;
                }
            }
        }
    });

    // Process messages from viewer
    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Control/input message
                if let Ok(viewer_msg) = serde_json::from_str::<ViewerMessage>(&text) {
                    match viewer_msg {
                        ViewerMessage::Ping { timestamp } => {
                            let pong_msg = ViewerOutMessage::Pong { timestamp };
                            if let Ok(json) = serde_json::to_string(&pong_msg) {
                                let _ = session.text(json).await;
                            }
                        }
                        _ => {
                            // Forward input events to device
                            if let Some(ref tx) = input_tx {
                                if let Ok(json) = serde_json::to_string(&viewer_msg) {
                                    let _ = tx.send(json.into_bytes()).await;
                                }
                            }
                        }
                    }
                }
            }
            Ok(Message::Binary(data)) => {
                // Forward binary data to device (if needed)
                if let Some(ref tx) = input_tx {
                    let _ = tx.send(data.to_vec()).await;
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = session.pong(&data).await;
            }
            Ok(Message::Close(_)) => {
                info!("👁️ Viewer {} sent close frame", viewer_id);
                break;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Nop) | Ok(Message::Continuation(_)) => {}
            Err(e) => {
                error!("Viewer {} WebSocket error: {:?}", viewer_id, e);
                break;
            }
        }
    }

    // Cleanup
    video_forward_task.abort();
    ws_state.viewer_connections.remove(&viewer_id);
    registry.remove_viewer(&viewer_id).await;

    info!(
        "👁️ Viewer {} disconnected from device: {}",
        viewer_id, hostname
    );
}

/// Send error message and close connection
async fn send_error(session: &mut Session, message: &str) -> Result<(), actix_ws::Closed> {
    let error_msg = ViewerOutMessage::Error {
        message: message.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&error_msg) {
        session.text(json).await?;
    }
    session.clone().close(None).await
}
