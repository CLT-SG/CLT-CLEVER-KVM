use crate::streaming::{
    RealtimeConfig, 
    IntegratedStreamHandler, 
    IntegratedStreamConfig,
    RealtimeStreamHandler,
    UltraStreamHandler,
    LowLatencyPipeline,
    PipelineConfig,
};
use axum::extract::ws::WebSocket;
use tokio::{sync::broadcast};
use log::{error, info, warn};
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

// H.264 streaming socket handler
async fn handle_h264_socket(
    socket: WebSocket, 
    monitor: usize, 
    enable_audio: bool,
    stop_rx: Option<broadcast::Receiver<()>>
) {
    info!("🚀 Initializing low-latency H.264 streaming for monitor {}", monitor);
    
    // Use the new low-latency H.264 pipeline as primary
    let pipeline_config = PipelineConfig {
        monitor_id: monitor,
        framerate: 30,
        bitrate_kbps: 4000,
        keyframe_interval_sec: 2,
        enable_audio,
        enable_hw_accel: true,
        target_latency_ms: 20,
        ..PipelineConfig::default()
    };
    
    match LowLatencyPipeline::new(pipeline_config) {
        Ok(pipeline) => {
            info!("✅ Low-latency H.264 pipeline initialized successfully");
            pipeline.handle_connection(socket, stop_rx).await;
        }
        Err(e) => {
            warn!("⚠️ Low-latency pipeline failed: {} - falling back to RGBA streaming", e);
            
            // Fallback to RGBA streaming
            match UltraStreamHandler::new(monitor) {
                Ok(handler) => {
                    info!("✅ RGBA fallback streaming handler initialized");
                    handler.handle_connection(socket, stop_rx).await;
                }
                Err(e) => {
                    error!("❌ Failed to create fallback streaming handler: {}", e);
                    
                    // Final fallback to standard real-time streaming
                    info!("🔄 Final fallback to real-time streaming...");
                    let enhanced_config = RealtimeConfig {
                        monitor_id: monitor,
                        width: 1920,
                        height: 1080,  
                        bitrate: 8000,
                        framerate: 30,
                        keyframe_interval: 30,
                        target_latency_ms: 150,
                    };
                    
                    match RealtimeStreamHandler::new(enhanced_config) {
                        Ok(handler) => {
                            info!("✅ Real-time fallback handler initialized");
                            handler.handle_connection(socket, stop_rx).await;
                        }
                        Err(e) => {
                            error!("❌ All streaming handlers failed to initialize: {}", e);
                        }
                    }
                }
            }
        }
    }
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
    info!("⚡ Starting ultra-performance H.264 streaming for monitor {}", monitor);
    
    // Try ultra-performance H.264 streaming first
    match crate::streaming::UltraStreamHandler::new(monitor) {
        Ok(ultra_handler) => {
            info!("🚀 Using ULTRA-PERFORMANCE H.264 streaming mode");
            ultra_handler.handle_connection(socket, Some(tokio::sync::broadcast::channel(1).1)).await;
        },
        Err(e) => {
            warn!("⚠️  Ultra-performance H.264 mode failed: {} - falling back to enhanced mode", e);
            
            // Fallback to enhanced real-time streaming
            let enhanced_config = crate::streaming::RealtimeConfig {
                monitor_id: monitor,
                width: 1920,
                height: 1080,  
                bitrate: 10000, // Very high bitrate for excellent quality
                framerate: 60,  // Smooth framerate
                keyframe_interval: 60, // Frequent keyframes for stability
                target_latency_ms: 120, // Optimized latency
            };
            
            match crate::streaming::RealtimeStreamHandler::new(enhanced_config) {
                Ok(fallback_handler) => {
                    info!("🔄 Using ENHANCED H.264 real-time streaming mode");
                    fallback_handler.handle_connection(socket, Some(tokio::sync::broadcast::channel(1).1)).await;
                },
                Err(e) => {
                    error!("❌ Both ultra and enhanced H.264 streaming failed: {}", e);
                    return Err(e.into());
                }
            }
        }
    }
    
    Ok(())
}
