use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{sync::{broadcast, Mutex}, time};
use log::{debug, error, info, warn};
use serde_json::json;
use anyhow::Result;

use crate::streaming::{RealtimeStreamEncoder, RealtimeConfig};
use crate::core::{InputHandler, InputEvent};
use crate::network::models::NetworkStats;

pub struct RealtimeStreamHandler {
    encoder: Arc<Mutex<RealtimeStreamEncoder>>,
    input_handler: Arc<parking_lot::Mutex<InputHandler>>,
    last_keyframe_time: Instant,
    frame_count: u64,
    network_stats: NetworkStats,
}

impl RealtimeStreamHandler {
    pub fn new(config: RealtimeConfig) -> Result<Self> {
        let encoder = Arc::new(Mutex::new(RealtimeStreamEncoder::new(config)?));
        let input_handler = Arc::new(parking_lot::Mutex::new(InputHandler::new()));
        
        Ok(Self {
            encoder,
            input_handler,
            last_keyframe_time: Instant::now(),
            frame_count: 0,
            network_stats: NetworkStats::default(),
        })
    }

    pub async fn handle_connection(
        mut self,
        socket: WebSocket,
        stop_rx: Option<broadcast::Receiver<()>>,
    ) {
        info!("Starting real-time streaming session");
        
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(10);
        let (control_tx, mut control_rx) = tokio::sync::mpsc::channel::<String>(10);
        
        // Pre-fetch monitor data to avoid Send issues
        let monitor_data = crate::core::capture::ScreenCapture::get_all_monitors().ok();
        
        // Send initial server info and monitor list
        {
            let encoder = self.encoder.lock().await;
            let (width, height) = encoder.get_dimensions();
            
            // Send server info
            let server_info = json!({
                "type": "server_info",
                "width": width,
                "height": height,
                "hostname": "kvm-server",
                "codec": "vp8",
                "audio": false,
                "tile_width": 64,
                "tile_height": 64,
                "tile_size": 64
            });
            
            if let Err(e) = control_tx.send(server_info.to_string()).await {
                error!("Failed to send server info: {}", e);
                return;
            }
            
            // Send monitor list
            if let Some(monitors) = monitor_data {
                let monitor_list = json!({
                    "type": "monitor_list",
                    "monitors": monitors.iter().map(|m| json!({
                        "id": m.id,
                        "name": m.name,
                        "width": m.width,
                        "height": m.height,
                        "is_primary": m.is_primary
                    })).collect::<Vec<_>>()
                });
                
                if let Err(e) = control_tx.send(monitor_list.to_string()).await {
                    error!("Failed to send monitor list: {}", e);
                }
            }
        }
        
        // Streaming task
        let encoder_clone = Arc::clone(&self.encoder);
        let streaming_task = {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_millis(16)); // ~60 FPS for smoother streaming
                let mut last_stats_time = Instant::now();
                let mut frame_count = 0u64;
                let mut last_keyframe_time = Instant::now();
                
                loop {
                    interval.tick().await;
                    
                    // Check if we should force a keyframe
                    let force_keyframe = last_keyframe_time.elapsed() > Duration::from_secs(2);
                    
                    let encoded_data = {
                        let mut encoder = encoder_clone.lock().await;
                        
                        match encoder.capture_and_encode() {
                            Ok(data) => data,
                            Err(e) => {
                                error!("Capture/encode error: {}", e);
                                // Reduce error backoff time for better real-time performance
                                tokio::time::sleep(Duration::from_millis(16)).await; // ~1 frame at 60fps
                                continue;
                            }
                        }
                    };
                    
                    if force_keyframe {
                        last_keyframe_time = Instant::now();
                    }
                    
                    frame_count += 1;
                    
                    // Send binary data directly to client (what the client expects)
                    if let Err(_) = tx.send(encoded_data).await {
                        break; // Channel closed
                    }
                    
                    // Log performance stats periodically
                    if last_stats_time.elapsed() > Duration::from_secs(5) {
                        let (capture_ms, encode_ms, total_frames) = {
                            let encoder = encoder_clone.lock().await;
                            encoder.get_performance_stats()
                        };
                        info!("Performance: capture={:.1}ms, encode={:.1}ms, frames={}, avg_fps={:.1}", 
                              capture_ms, encode_ms, total_frames, 
                              frame_count as f64 / last_stats_time.elapsed().as_secs_f64());
                        last_stats_time = Instant::now();
                    }
                }
                
                info!("Streaming task ended");
            })
        };
        
        // Send task - handles both binary video data and text control messages
        let send_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Binary video data
                    Some(data) = rx.recv() => {
                        if let Err(e) = sender.send(Message::Binary(data)).await {
                            error!("Failed to send video data: {}", e);
                            break;
                        }
                    }
                    // Text control messages  
                    Some(text) = control_rx.recv() => {
                        if let Err(e) = sender.send(Message::Text(text)).await {
                            error!("Failed to send control message: {}", e);
                            break;
                        }
                    }
                    else => break,
                }
            }
        });
        
        // Receive task for handling client messages
        let encoder_clone2 = Arc::clone(&self.encoder);
        let control_tx_clone = control_tx.clone();
        let input_handler_clone = Arc::clone(&self.input_handler);
        let receive_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(json_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            match json_msg.get("type").and_then(|t| t.as_str()) {
                                // Handle input events (mouse and keyboard)
                                Some("mousemove") | Some("mousedown") | Some("mouseup") | 
                                Some("wheel") | Some("keydown") | Some("keyup") => {
                                    // Parse and handle input event
                                    if let Some(input_event) = parse_client_input_event(&json_msg) {
                                        let mut handler = input_handler_clone.lock();
                                        if let Err(e) = handler.handle_event(input_event) {
                                            debug!("Input event error: {}", e);
                                        }
                                    }
                                }
                                Some("ping") => {
                                    let pong = json!({
                                        "type": "pong",
                                        "timestamp": json_msg.get("timestamp")
                                    });
                                    if let Err(_) = control_tx_clone.send(pong.to_string()).await {
                                        break;
                                    }
                                }
                                Some("request_keyframe") => {
                                    let mut encoder = encoder_clone2.lock().await;
                                    encoder.force_keyframe();
                                }
                                Some("network_stats") => {
                                    if let Some(stats) = json_msg.get("stats") {
                                        if let Ok(network_stats) = serde_json::from_value::<NetworkStats>(stats.clone()) {
                                            let mut encoder = encoder_clone2.lock().await;
                                            if let Err(e) = encoder.adapt_to_network_conditions(&network_stats) {
                                                warn!("Failed to adapt to network conditions: {}", e);
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    debug!("Unknown message type: {}", text);
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed by client");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });
        
        // Wait for completion or stop signal
        tokio::select! {
            _ = streaming_task => info!("Streaming task completed"),
            _ = send_task => info!("Send task completed"),  
            _ = receive_task => info!("Receive task completed"),
            _ = async {
                if let Some(mut stop_rx) = stop_rx {
                    stop_rx.recv().await.ok();
                }
            } => info!("Stop signal received"),
        }
        
        info!("Real-time streaming session ended");
    }
}

/// Parse client input events from JSON and convert to InputEvent
/// Handles the conversion from web client format to server InputEvent format
fn parse_client_input_event(json: &serde_json::Value) -> Option<InputEvent> {
    let event_type = json.get("type")?.as_str()?;
    
    match event_type {
        "mousemove" => {
            let x = json.get("x")?.as_i64()? as i32;
            let y = json.get("y")?.as_i64()? as i32;
            let monitor_id = json.get("monitor_id").and_then(|v| v.as_str()).map(String::from);
            Some(InputEvent::MouseMove { x, y, monitor_id })
        }
        "mousedown" => {
            let x = json.get("x")?.as_i64()? as i32;
            let y = json.get("y")?.as_i64()? as i32;
            let button = json.get("button")?.as_str()?.to_string();
            let monitor_id = json.get("monitor_id").and_then(|v| v.as_str()).map(String::from);
            Some(InputEvent::MouseDown { button, x, y, monitor_id })
        }
        "mouseup" => {
            let x = json.get("x")?.as_i64()? as i32;
            let y = json.get("y")?.as_i64()? as i32;
            let button = json.get("button")?.as_str()?.to_string();
            let monitor_id = json.get("monitor_id").and_then(|v| v.as_str()).map(String::from);
            Some(InputEvent::MouseUp { button, x, y, monitor_id })
        }
        "wheel" => {
            let delta_y = json.get("delta_y")?.as_i64()? as i32;
            let delta_x = json.get("delta_x").and_then(|v| v.as_i64()).map(|v| v as i32);
            let monitor_id = json.get("monitor_id").and_then(|v| v.as_str()).map(String::from);
            Some(InputEvent::MouseWheel { delta_y, delta_x, monitor_id })
        }
        "keydown" => {
            let key = json.get("key")?.as_str()?.to_string();
            let code = json.get("code").and_then(|v| v.as_str()).map(String::from);
            
            // Convert individual modifier flags to modifiers array
            let mut modifiers = Vec::new();
            if json.get("ctrlKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Control".to_string());
            }
            if json.get("altKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Alt".to_string());
            }
            if json.get("shiftKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Shift".to_string());
            }
            if json.get("metaKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Meta".to_string());
            }
            
            let repeat = json.get("repeat").and_then(|v| v.as_bool());
            
            Some(InputEvent::KeyDown { key, code, modifiers, repeat })
        }
        "keyup" => {
            let key = json.get("key")?.as_str()?.to_string();
            let code = json.get("code").and_then(|v| v.as_str()).map(String::from);
            
            // Convert individual modifier flags to modifiers array
            let mut modifiers = Vec::new();
            if json.get("ctrlKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Control".to_string());
            }
            if json.get("altKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Alt".to_string());
            }
            if json.get("shiftKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Shift".to_string());
            }
            if json.get("metaKey").and_then(|v| v.as_bool()).unwrap_or(false) {
                modifiers.push("Meta".to_string());
            }
            
            Some(InputEvent::KeyUp { key, code, modifiers })
        }
        _ => None
    }
}
