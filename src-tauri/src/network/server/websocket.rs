//! WebSocket handlers for VP9 video streaming
//!
//! This module provides WebSocket connection handlers for the VP9 streaming
//! service using the RustDesk-inspired rdengine with WebRTC transport.

#![allow(dead_code)]

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
    
    #[serde(rename = "streaming_config")]
    StreamingConfig { 
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

// Helper function to make the future Send - uses VP9 streaming via rdengine
pub async fn handle_socket_wrapper(socket: WebSocket, monitor: usize, codec: String, enable_audio: bool) {
    info!("[INFO] New VP9 streaming WebSocket connection - Monitor: {}, Codec: {}, Audio: {}", 
          monitor, codec, enable_audio);
    
    handle_streaming_socket(socket, monitor, enable_audio, None).await;
    
    info!("[INFO] VP9 streaming WebSocket connection closed - Monitor: {}", monitor);
}

// Helper function with stop signal - uses VP9 streaming via rdengine
pub async fn handle_socket_wrapper_with_stop(
    socket: WebSocket, 
    monitor: usize, 
    codec: String, 
    enable_audio: bool, 
    stop_rx: broadcast::Receiver<()>
) {
    info!("[INFO] New VP9 streaming WebSocket connection with stop signal - Monitor: {}, Codec: {}, Audio: {}", 
          monitor, codec, enable_audio);
    
    handle_streaming_socket(socket, monitor, enable_audio, Some(stop_rx)).await;
    
    info!("[INFO] VP9 streaming WebSocket connection with stop signal closed - Monitor: {}", monitor);
}

pub async fn handle_socket_with_stop(
    socket: WebSocket, 
    monitor: usize, 
    codec: String, 
    enable_audio: bool, 
    stop_rx: broadcast::Receiver<()>
) {
    info!("[INFO] New WebSocket connection with stop signal: monitor={}, codec={}, audio={}", 
          monitor, codec, enable_audio);
    
    // Use VP9 streaming for connections with stop signal
    handle_streaming_socket(socket, monitor, enable_audio, Some(stop_rx)).await;
}

pub async fn handle_socket(socket: WebSocket, monitor: usize, codec: String, enable_audio: bool) {
    info!("[INFO] New WebSocket connection: monitor={}, codec={}, audio={}", 
          monitor, codec, enable_audio);
    
    // Use VP9 streaming for direct connections
    handle_streaming_socket(socket, monitor, enable_audio, None).await;
}

// VP9 streaming socket handler — uses RustDesk-inspired rdengine
// with WebRTC DataChannel transport for low-latency peer-to-peer streaming
async fn handle_streaming_socket(
    socket: WebSocket, 
    monitor: usize, 
    enable_audio: bool,
    stop_rx: Option<broadcast::Receiver<()>>
) {
    info!("Initializing RustDesk-inspired VP9 streaming with WebRTC transport for monitor {}", monitor);
    
    // PRIMARY: Use rdengine with WebRTC DataChannel transport
    let rd_config = ConnectionConfig {
        monitor_id: monitor,
        codec: VpxCodec::VP9,
        framerate: 24,
        bitrate_kbps: 1500,
        enable_audio,
        enable_webrtc: true, // Use WebRTC for video/audio transport
        webrtc_config: Some(crate::rdengine::WebRtcConfig::lan()), // LAN-optimized: no STUN delay
    };

    ConnectionHandler::handle(socket, rd_config, stop_rx).await;
}

pub async fn handle_socket_ultra(
    ws: axum::extract::WebSocketUpgrade,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let monitor = query.get("monitor")
        .and_then(|m| m.parse::<usize>().ok())
        .unwrap_or(0);

    info!("[INFO] Ultra WebSocket connection request for monitor {}", monitor);

    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_ultra_connection(socket, monitor).await {
            error!("[ERROR] Ultra WebSocket connection failed: {}", e);
        }
    })
}

async fn handle_ultra_connection(socket: WebSocket, monitor: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting RustDesk-inspired ultra streaming with WebRTC for monitor {}", monitor);
    
    // Use rdengine with LAN-optimized WebRTC config
    let config = ConnectionConfig {
        monitor_id: monitor,
        codec: VpxCodec::VP9,
        framerate: 30,
        bitrate_kbps: 2000,
        enable_audio: false,
        enable_webrtc: true,
        webrtc_config: Some(crate::rdengine::WebRtcConfig::lan()), // LAN-optimized: no STUN
    };

    ConnectionHandler::handle(socket, config, None).await;
    
    Ok(())
}
