use crate::rdengine::{ConnectionHandler, ConnectionConfig};
use crate::rdengine::codec::VpxCodec;
use axum::extract::ws::WebSocket;
use tokio::sync::broadcast;
use log::{error, info};
use serde::{Deserialize, Serialize};

// Control messages for WebSocket communication
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    #[serde(rename = "ping")]
    Ping { timestamp: Option<u64> },
    
    #[serde(rename = "switch_codec")]
    SwitchCodec { codec: String },
    
    #[serde(rename = "request_keyframe")]
    RequestKeyframe,
    
    #[serde(rename = "quality_setting")]
    QualitySetting { quality: u8 },
    
    #[serde(rename = "bitrate_setting")]
    BitrateSetting { bitrate: u32 },
    
    #[serde(rename = "h264_config")]
    H264Config { 
        enable_hw_accel: bool,
        enable_opus: bool,
        target_bitrate: Option<u32>
    },
    
    #[serde(rename = "network_stats")]
    NetworkStats { 
        latency: Option<u32>,
        bandwidth: Option<f32>,
        packet_loss: Option<f32>
    },
}

// Helper function to make the future Send - uses H.264 streaming
pub async fn handle_socket_wrapper(socket: WebSocket, monitor: usize, codec: String, enable_audio: bool) {
    info!("🎬 New H.264 streaming WebSocket connection - Monitor: {}, Codec: {}, Audio: {}", 
          monitor, codec, enable_audio);
    
    handle_h264_socket(socket, monitor, enable_audio, None).await;
    
    info!("✅ H.264 streaming WebSocket connection closed - Monitor: {}", monitor);
}

// Helper function with stop signal - uses H.264 streaming
pub async fn handle_socket_wrapper_with_stop(
    socket: WebSocket, 
    monitor: usize, 
    codec: String, 
    enable_audio: bool, 
    stop_rx: broadcast::Receiver<()>
) {
    info!("🎬 New H.264 streaming WebSocket connection with stop signal - Monitor: {}, Codec: {}, Audio: {}", 
          monitor, codec, enable_audio);
    
    handle_h264_socket(socket, monitor, enable_audio, Some(stop_rx)).await;
    
    info!("✅ H.264 streaming WebSocket connection with stop signal closed - Monitor: {}", monitor);
}

pub async fn handle_socket_with_stop(
    socket: WebSocket, 
    monitor: usize, 
    codec: String, 
    enable_audio: bool, 
    stop_rx: broadcast::Receiver<()>
) {
    info!("🎬 New WebSocket connection with stop signal: monitor={}, codec={}, audio={}", 
          monitor, codec, enable_audio);
    
    // Use H.264 streaming for connections with stop signal
    handle_h264_socket(socket, monitor, enable_audio, Some(stop_rx)).await;
}

pub async fn handle_socket(socket: WebSocket, monitor: usize, codec: String, enable_audio: bool) {
    info!("🎬 New WebSocket connection: monitor={}, codec={}, audio={}", 
          monitor, codec, enable_audio);
    
    // Use H.264 streaming for direct connections
    handle_h264_socket(socket, monitor, enable_audio, None).await;
}

// H.264 streaming socket handler — now uses RustDesk-inspired rdengine as primary
async fn handle_h264_socket(
    socket: WebSocket, 
    monitor: usize, 
    enable_audio: bool,
    stop_rx: Option<broadcast::Receiver<()>>
) {
    info!("Initializing RustDesk-inspired VP9 streaming for monitor {}", monitor);
    
    // PRIMARY: Use the new rdengine (RustDesk-style VP9 encoding)
    let rd_config = ConnectionConfig {
        monitor_id: monitor,
        codec: VpxCodec::VP9,
        framerate: 30,
        bitrate_kbps: 2000,
        enable_audio,
    };

    // Try rdengine first — this is the low-latency path
    ConnectionHandler::handle(socket, rd_config, stop_rx).await;
}

pub async fn handle_socket_ultra(
    ws: axum::extract::WebSocketUpgrade,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let monitor = query.get("monitor")
        .and_then(|m| m.parse::<usize>().ok())
        .unwrap_or(0);

    info!("🔌 Ultra WebSocket connection request for monitor {}", monitor);

    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_ultra_connection(socket, monitor).await {
            error!("❌ Ultra WebSocket connection failed: {}", e);
        }
    })
}

async fn handle_ultra_connection(socket: WebSocket, monitor: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting RustDesk-inspired ultra streaming for monitor {}", monitor);
    
    // Use rdengine with LAN-optimized config
    let config = ConnectionConfig {
        monitor_id: monitor,
        codec: VpxCodec::VP9,
        framerate: 60,
        bitrate_kbps: 4000,
        enable_audio: false,
    };

    ConnectionHandler::handle(socket, config, None).await;
    
    Ok(())
}
