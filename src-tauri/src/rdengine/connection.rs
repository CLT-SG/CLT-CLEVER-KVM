//! WebSocket Connection Handler
//!
//! Manages a single client WebSocket connection with:
//! - Separated video and control message paths (prevents HOL blocking)
//! - Input event dispatching
//! - Ping/pong latency measurement
//! - Adaptive QoS feedback loop
//!
//! Follows RustDesk's connection.rs pattern:
//! - tokio::select! for multiplexing
//! - Separate channels for video vs control
//! - Input handler on its own path

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket};
use crossbeam_channel::Receiver;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::core::{InputHandler, InputEvent as CoreInputEvent};
use crate::rdengine::audio_service::{AudioFrame, AudioService, AudioServiceConfig};
use crate::rdengine::protocol::{self, ControlMsg, InputMsg, ServerInfo, CODEC_VP8, CODEC_VP9};
use crate::rdengine::qos::QualityControl;
use crate::rdengine::video_service::{VideoFrame, VideoService, VideoServiceConfig};
use crate::rdengine::codec::VpxCodec;

/// Connection handler configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub monitor_id: usize,
    pub codec: VpxCodec,
    pub framerate: u32,
    pub bitrate_kbps: u32,
    pub enable_audio: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            monitor_id: 0,
            codec: VpxCodec::VP9,
            framerate: 30,
            bitrate_kbps: 4000,
            enable_audio: false,
        }
    }
}

/// The connection handler manages a single WebSocket client session
pub struct ConnectionHandler;

impl ConnectionHandler {
    /// Handle a WebSocket connection with RustDesk-level streaming
    pub async fn handle(
        socket: WebSocket,
        config: ConnectionConfig,
        stop_rx: Option<tokio::sync::broadcast::Receiver<()>>,
    ) {
        info!(
            "New rdengine connection: monitor={}, codec={:?}, fps={}, bitrate={}kbps, audio={}",
            config.monitor_id, config.codec, config.framerate, config.bitrate_kbps, config.enable_audio
        );

        if let Err(e) = Self::handle_inner(socket, config, stop_rx).await {
            error!("Connection error: {}", e);
        }

        info!("rdengine connection closed");
    }

    async fn handle_inner(
        socket: WebSocket,
        config: ConnectionConfig,
        stop_rx: Option<tokio::sync::broadcast::Receiver<()>>,
    ) -> Result<()> {
        // Split WebSocket into sender/receiver
        let (mut ws_tx, mut ws_rx) = socket.split();

        // Start video service on dedicated thread
        let video_config = VideoServiceConfig {
            monitor_id: config.monitor_id,
            codec: config.codec,
            framerate: config.framerate,
            bitrate_kbps: config.bitrate_kbps,
            ..VideoServiceConfig::default()
        };

        let mut video_service = VideoService::start(video_config)
            .map_err(|e| anyhow::anyhow!("Failed to start video service: {}", e))?;

        // Start audio service if enabled
        let audio_service = if config.enable_audio {
            match AudioService::start(AudioServiceConfig::default()) {
                Ok(svc) => {
                    info!("Audio service started");
                    Some(svc)
                }
                Err(e) => {
                    warn!("Failed to start audio: {} — continuing without audio", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize input handler
        let mut input_handler = InputHandler::new();

        // Get capture dimensions for server info
        let capture = crate::core::ScreenCapture::new(Some(config.monitor_id))
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let (width, height) = capture.dimensions();
        let (width, height) = (width as u32, height as u32);
        drop(capture);

        // Send server info as JSON text message
        let codec_name = match config.codec {
            VpxCodec::VP8 => "vp8",
            VpxCodec::VP9 => "vp9",
        };

        let server_info = ServerInfo {
            r#type: "server_info".to_string(),
            width,
            height,
            hostname: gethostname::gethostname().to_string_lossy().to_string(),
            codec: codec_name.to_string(),
            framerate: config.framerate,
            bitrate_kbps: config.bitrate_kbps,
            audio_enabled: audio_service.is_some(),
            protocol_version: 2, // v2 = RustDesk-inspired binary protocol
        };

        let info_json = serde_json::to_string(&server_info)?;
        ws_tx.send(Message::Text(info_json)).await?;

        // Send monitor list
        let monitors = crate::core::ScreenCapture::get_all_monitors()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let monitor_list: Vec<serde_json::Value> = monitors.iter().map(|m| {
            json!({
                "id": m.id,
                "name": m.name,
                "is_primary": m.is_primary,
                "width": m.width,
                "height": m.height,
            })
        }).collect();
        let monitors_msg = json!({
            "type": "monitors",
            "monitors": monitor_list,
        });
        ws_tx.send(Message::Text(serde_json::to_string(&monitors_msg)?)).await?;

        // Request initial keyframe
        video_service.request_keyframe();

        // Internal channel for outbound control messages
        let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<String>(32);

        // Clone frame receiver for the video bridge task
        let frame_rx = video_service.frame_rx().clone();
        let audio_rx = audio_service.as_ref().map(|s| s.audio_rx().clone());

        // QoS reference
        let qos = video_service.qos().clone();

        // Bridge crossbeam → tokio: a single persistent blocking task reads from
        // the crossbeam channel and forwards to a tokio mpsc channel.
        // This avoids the spawn_blocking-per-iteration race where orphaned tasks
        // steal frames from the channel.
        let (video_bridge_tx, mut video_bridge_rx) = mpsc::channel::<VideoFrame>(4);
        let bridge_frame_rx = frame_rx.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                match bridge_frame_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(frame) => {
                        if video_bridge_tx.blocking_send(frame).is_err() {
                            // Receiver dropped (connection closed)
                            break;
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // No frame available, check if we should stop
                        continue;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        log::error!("Video service channel disconnected — capture thread crashed or stopped");
                        break;
                    }
                }
            }
            log::debug!("Video bridge task exited");
        });

        // Audio bridge (same pattern)
        let mut audio_bridge_rx: Option<mpsc::Receiver<AudioFrame>> = None;
        if let Some(arx) = audio_rx {
            let (atx, arx_bridge) = mpsc::channel::<AudioFrame>(8);
            tokio::task::spawn_blocking(move || {
                loop {
                    match arx.recv_timeout(Duration::from_millis(50)) {
                        Ok(frame) => {
                            if atx.blocking_send(frame).is_err() {
                                break;
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
            audio_bridge_rx = Some(arx_bridge);
        }

        // Ping interval
        let mut ping_interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_ping_time = Instant::now();

        // QoS adjustment interval
        let mut qos_interval = tokio::time::interval(Duration::from_secs(3));

        // Optional stop signal
        let mut stop_rx = stop_rx;

        info!("Starting main connection loop (with bridge tasks)");

        loop {
            tokio::select! {
                // Priority 1: Stop signal
                _ = async {
                    if let Some(ref mut rx) = stop_rx {
                        rx.recv().await.ok();
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    info!("Stop signal received");
                    break;
                }

                // Priority 2: Video frames via bridge (crossbeam → tokio mpsc)
                frame = video_bridge_rx.recv() => {
                    match frame {
                        Some(video_frame) => {
                            if let Err(e) = ws_tx.send(Message::Binary(video_frame.message)).await {
                                warn!("Failed to send video frame: {}", e);
                                break;
                            }
                        }
                        None => {
                            error!("Video bridge channel closed — video service stopped");
                            break;
                        }
                    }
                }

                // Priority 3: Audio frames via bridge
                audio = async {
                    if let Some(ref mut rx) = audio_bridge_rx {
                        rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    match audio {
                        Some(audio_frame) => {
                            if let Err(e) = ws_tx.send(Message::Binary(audio_frame.message)).await {
                                warn!("Failed to send audio frame: {}", e);
                            }
                        }
                        None => {
                            warn!("Audio bridge channel closed");
                        }
                    }
                }

                // Priority 4: Control messages to send
                msg = ctrl_rx.recv() => {
                    if let Some(text) = msg {
                        if let Err(e) = ws_tx.send(Message::Text(text)).await {
                            warn!("Failed to send control message: {}", e);
                            break;
                        }
                    }
                }

                // Priority 5: Incoming messages from client
                msg = ws_rx.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            Self::handle_text_message(
                                &text,
                                &mut input_handler,
                                &video_service,
                                &qos,
                                &ctrl_tx,
                            ).await;
                        }
                        Some(Ok(Message::Binary(data))) => {
                            // Binary messages from client (ping)
                            if let Some(msg_type) = protocol::parse_message_type(&data) {
                                if msg_type == protocol::MSG_PING {
                                    if let Some(payload) = protocol::parse_payload(&data) {
                                        if payload.len() >= 8 {
                                            let ts = u64::from_le_bytes([
                                                payload[0], payload[1], payload[2], payload[3],
                                                payload[4], payload[5], payload[6], payload[7],
                                            ]);
                                            let pong = protocol::encode_pong(ts);
                                            let _ = ws_tx.send(Message::Binary(pong)).await;
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            info!("Client disconnected");
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = ws_tx.send(Message::Pong(data)).await;
                        }
                        Some(Ok(_)) => {} // Ignore other message types
                        Some(Err(e)) => {
                            warn!("WebSocket receive error: {}", e);
                            break;
                        }
                    }
                }

                // Priority 6: Periodic ping
                _ = ping_interval.tick() => {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    last_ping_time = Instant::now();

                    let ping_msg = json!({
                        "type": "ping",
                        "timestamp": now_ms,
                    });
                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ping_msg).unwrap())).await;
                }

                // Priority 7: QoS adjustment
                _ = qos_interval.tick() => {
                    let (fps, bitrate, changed) = qos.lock().maybe_adjust();
                    if changed {
                        let qos_msg = json!({
                            "type": "quality_update",
                            "fps": fps,
                            "bitrate_kbps": bitrate,
                            "quality": qos.lock().quality_level(),
                        });
                        let _ = ctrl_tx.send(serde_json::to_string(&qos_msg).unwrap()).await;
                    }
                }
            }
        }

        // Cleanup
        info!("Cleaning up connection resources");
        video_service.stop();
        if let Some(mut audio) = audio_service {
            audio.stop();
        }

        Ok(())
    }

    /// Handle a text message from the client (control or input)
    async fn handle_text_message(
        text: &str,
        input_handler: &mut InputHandler,
        video_service: &VideoService,
        qos: &Arc<parking_lot::Mutex<QualityControl>>,
        ctrl_tx: &mpsc::Sender<String>,
    ) {
        // Try parsing as control message first
        if let Ok(ctrl) = serde_json::from_str::<ControlMsg>(text) {
            match ctrl {
                ControlMsg::Ping { timestamp } => {
                    let ts = timestamp.unwrap_or(0);
                    let pong = json!({ "type": "pong", "timestamp": ts });
                    let _ = ctrl_tx.send(serde_json::to_string(&pong).unwrap()).await;
                }
                ControlMsg::Pong { timestamp } => {
                    if let Some(ts) = timestamp {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        if now > ts {
                            let rtt = (now - ts) as u32;
                            qos.lock().record_rtt(rtt);
                            debug!("RTT: {}ms", rtt);
                        }
                    }
                }
                ControlMsg::RequestKeyframe => {
                    info!("Client requested keyframe");
                    video_service.request_keyframe();
                }
                ControlMsg::QualityUpdate { quality } => {
                    info!("Client quality update: {}", quality);
                    // Map quality names to presets
                    match quality.as_str() {
                        "high" => {
                            qos.lock().set_fps(60);
                            qos.lock().set_bitrate(8000);
                        }
                        "low" => {
                            qos.lock().set_fps(15);
                            qos.lock().set_bitrate(1500);
                        }
                        _ => {
                            // "balanced" / default
                            qos.lock().set_fps(30);
                            qos.lock().set_bitrate(4000);
                        }
                    }
                }
                ControlMsg::BitrateUpdate { bitrate_kbps } => {
                    qos.lock().set_bitrate(bitrate_kbps);
                }
                ControlMsg::FpsUpdate { fps } => {
                    qos.lock().set_fps(fps);
                }
                ControlMsg::NetworkStats { latency, bandwidth, packet_loss } => {
                    if let Some(lat) = latency {
                        qos.lock().record_rtt(lat);
                    }
                }
                ControlMsg::SwitchCodec { codec } => {
                    info!("Client requested codec switch to: {}", codec);
                    // Codec switching requires reconnection with new config
                }
            }
            return;
        }

        // Try parsing as input event
        match serde_json::from_str::<InputMsg>(text) {
            Ok(input) => {
                let event = match input {
                    InputMsg::MouseMove { x, y, monitor_id } => {
                        let mid = monitor_id.and_then(|v| match v {
                            serde_json::Value::String(s) => Some(s),
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        });
                        CoreInputEvent::MouseMove {
                            x: x as i32,
                            y: y as i32,
                            monitor_id: mid,
                        }
                    }
                    InputMsg::MouseDown { x, y, button, monitor_id } => {
                        let btn_num = protocol::parse_button_value(&button);
                        let btn_str = match btn_num {
                            0 => "left",
                            1 => "middle",
                            2 => "right",
                            _ => "left",
                        }.to_string();
                        CoreInputEvent::MouseDown {
                            button: btn_str,
                            x: x as i32,
                            y: y as i32,
                            monitor_id: None,
                        }
                    }
                    InputMsg::MouseUp { x, y, button, monitor_id } => {
                        let btn_num = protocol::parse_button_value(&button);
                        let btn_str = match btn_num {
                            0 => "left",
                            1 => "middle",
                            2 => "right",
                            _ => "left",
                        }.to_string();
                        CoreInputEvent::MouseUp {
                            button: btn_str,
                            x: x as i32,
                            y: y as i32,
                            monitor_id: None,
                        }
                    }
                    InputMsg::Wheel { delta_x, delta_y, .. } => {
                        CoreInputEvent::MouseWheel {
                            delta_y: delta_y as i32,
                            delta_x: Some(delta_x as i32),
                            monitor_id: None,
                        }
                    }
                    InputMsg::KeyDown { key, code, key_code, ctrl_key, alt_key, shift_key, meta_key, modifiers } => {
                        // Build code string: prefer explicit `code`, fall back to key_code mapping
                        let code_str = code.or_else(|| {
                            // The client may send keyCode (integer) without code (string)
                            // We don't need code for InputHandler — it tries code first, then key
                            None
                        });
                        CoreInputEvent::KeyDown {
                            key,
                            code: code_str,
                            modifiers: Vec::new(),
                            repeat: None,
                        }
                    }
                    InputMsg::KeyUp { key, code, key_code, ctrl_key, alt_key, shift_key, meta_key, modifiers } => {
                        let code_str = code;
                        CoreInputEvent::KeyUp {
                            key,
                            code: code_str,
                            modifiers: Vec::new(),
                        }
                    }
                };

                if let Err(e) = input_handler.handle_event(event) {
                    debug!("Input event error: {}", e);
                }
            }
            Err(e) => {
                // Log parsing failures so we can diagnose protocol mismatches
                warn!("Failed to parse message as control or input: {} | raw: {}", e, &text[..text.len().min(200)]);
            }
        }
    }
}
