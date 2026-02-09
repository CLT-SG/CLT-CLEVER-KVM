//! WebSocket + WebRTC Connection Handler
//!
//! Manages a single client connection with two transport modes:
//!
//! **WebSocket-only mode (fallback):**
//! - Video/audio/cursor sent as binary WebSocket messages
//! - Input/control sent as JSON text WebSocket messages
//!
//! **WebRTC mode (preferred for real-time streaming):**
//! - Video frames sent via unreliable/unordered DataChannel (UDP, no HOL blocking)
//! - Audio frames sent via reliable DataChannel
//! - Cursor updates sent via reliable DataChannel
//! - Input/control still via WebSocket (reliable, low-frequency)
//! - SDP/ICE signaling via WebSocket JSON messages
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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::core::{InputHandler, InputEvent as CoreInputEvent};
use crate::rdengine::audio_service::{AudioFrame, AudioService, AudioServiceConfig};
use crate::rdengine::cursor_service::{CursorService, CursorServiceConfig, CursorUpdate};
use crate::rdengine::protocol::{self, ControlMsg, InputMsg, ServerInfo, CODEC_VP8, CODEC_VP9, FLAG_KEYFRAME};
use crate::rdengine::qos::QualityControl;
use crate::rdengine::video_service::{VideoFrame, VideoService, VideoServiceConfig};
use crate::rdengine::codec::VpxCodec;
use crate::rdengine::webrtc_transport::{WebRtcTransport, WebRtcConfig, WebRtcEvent};
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

/// Connection handler configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub monitor_id: usize,
    pub codec: VpxCodec,
    pub framerate: u32,
    pub bitrate_kbps: u32,
    pub enable_audio: bool,
    /// Enable WebRTC DataChannel transport for video/audio/cursor.
    /// When true, WebSocket is used only for signaling and input.
    /// When false, falls back to WebSocket binary transport.
    pub enable_webrtc: bool,
    /// WebRTC-specific configuration (ICE servers, buffer sizes, etc.)
    pub webrtc_config: Option<WebRtcConfig>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            monitor_id: 0,
            codec: VpxCodec::VP9,
            framerate: 24,
            bitrate_kbps: 1500,
            enable_audio: false,
            enable_webrtc: true, // WebRTC preferred by default
            webrtc_config: None, // Use WebRtcConfig::default()
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
            "New rdengine connection: monitor={}, codec={:?}, fps={}, bitrate={}kbps, audio={}, webrtc={}",
            config.monitor_id, config.codec, config.framerate, config.bitrate_kbps,
            config.enable_audio, config.enable_webrtc
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
            protocol_version: 3, // v3 = WebRTC DataChannel transport
            webrtc_enabled: config.enable_webrtc,
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
        // Created early so WebRTC event forwarder can use it
        let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<String>(32);

        // ── WebRTC Transport Setup ──────────────────────────────────────
        // If WebRTC is enabled, create the transport and send the SDP offer
        // to the client via WebSocket signaling. The client will respond with
        // Signal from WebRTC event task → main loop: ICE connected, reset keyframe gate.
        // Created unconditionally; only used when WebRTC is active.
        let connection_ready_signal = Arc::new(AtomicBool::new(false));

        // an SDP answer and ICE candidates, also via WebSocket.
        let webrtc_transport: Option<Arc<WebRtcTransport>> = if config.enable_webrtc {
            let rtc_config = config.webrtc_config.clone()
                .unwrap_or_else(WebRtcConfig::default);

            match WebRtcTransport::new(rtc_config).await {
                Ok((transport, mut rtc_event_rx)) => {
                    let transport = Arc::new(transport);

                    // Create SDP offer with video media track and send to client
                    match transport.create_offer(codec_name).await {
                        Ok(sdp_offer) => {
                            let offer_msg = json!({
                                "type": "webrtc_offer",
                                "sdp": sdp_offer,
                            });
                            ws_tx.send(Message::Text(serde_json::to_string(&offer_msg)?)).await?;
                            info!("WebRTC SDP offer sent to client");

                            // Don't request a keyframe here — ICE hasn't connected yet.
                            // The keyframe will be requested when ConnectionStateChanged(Connected)
                            // fires, ensuring the browser is actually ready to receive it.

                            // Spawn a task to forward WebRTC events to the WS control channel
                            let ctrl_tx_rtc = ctrl_tx.clone();
                            let keyframe_signal = video_service.keyframe_signal();
                            let connection_ready_signal_clone = connection_ready_signal.clone();
                            tokio::spawn(async move {
                                while let Some(event) = rtc_event_rx.recv().await {
                                    match event {
                                        WebRtcEvent::IceCandidate(candidate_json) => {
                                            let msg = json!({
                                                "type": "webrtc_ice_candidate",
                                                "candidate": candidate_json,
                                            });
                                            let _ = ctrl_tx_rtc.send(
                                                serde_json::to_string(&msg).unwrap()
                                            ).await;
                                        }
                                        WebRtcEvent::VideoTrackReady => {
                                            info!("WebRTC video media track is ready (native <video> rendering)");
                                            keyframe_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                                        }
                                        WebRtcEvent::KeyframeRequested => {
                                            info!("RTCP PLI: browser requested keyframe — forcing encoder");
                                            keyframe_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                                        }
                                        WebRtcEvent::VideoChannelReady => {
                                            info!("WebRTC video DataChannel is ready");
                                        }
                                        WebRtcEvent::AudioChannelReady => {
                                            info!("WebRTC audio DataChannel is ready");
                                        }
                                        WebRtcEvent::CursorChannelReady => {
                                            info!("WebRTC cursor DataChannel is ready");
                                        }
                                        WebRtcEvent::ConnectionStateChanged(state) => {
                                            info!("WebRTC connection state: {:?}", state);
                                            if state == RTCPeerConnectionState::Connected {
                                                // ICE is connected — the browser can now receive
                                                // RTP packets. Request a keyframe so the first
                                                // frame the browser actually receives is decodable.
                                                info!("ICE connected — requesting keyframe for immediate video");
                                                keyframe_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                                                // Tell the main loop to reset the keyframe gate
                                                // so P-frames are held until this fresh keyframe
                                                // is delivered through the connected transport.
                                                connection_ready_signal_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                                            } else if state == RTCPeerConnectionState::Failed
                                                || state == RTCPeerConnectionState::Disconnected
                                            {
                                                warn!("WebRTC peer connection lost — client should fall back to WebSocket");
                                            }
                                        }
                                        WebRtcEvent::Error(err) => {
                                            error!("WebRTC transport error: {}", err);
                                        }
                                    }
                                }
                            });

                            Some(transport)
                        }
                        Err(e) => {
                            warn!("Failed to create WebRTC offer: {} — falling back to WebSocket", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to initialize WebRTC transport: {} — falling back to WebSocket", e);
                    None
                }
            }
        } else {
            None
        };

        // Clone transport reference for the main loop
        let webrtc = webrtc_transport.clone();

        // Clone frame receiver for the video bridge task
        let frame_rx = video_service.frame_rx().clone();
        let audio_rx = audio_service.as_ref().map(|s| s.audio_rx().clone());

        // QoS reference
        let qos = video_service.qos().clone();

        // Bridge crossbeam → tokio: a single persistent blocking task reads from
        // the crossbeam channel and forwards to a tokio mpsc channel.
        // This avoids the spawn_blocking-per-iteration race where orphaned tasks
        // steal frames from the channel.
        let (video_bridge_tx, mut video_bridge_rx) = mpsc::channel::<VideoFrame>(2);
        let bridge_frame_rx = frame_rx.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                match bridge_frame_rx.recv_timeout(Duration::from_millis(50)) {
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

        // Start cursor tracking service
        let cursor_config = CursorServiceConfig {
            poll_interval_ms: 33, // ~30 Hz cursor updates (enough for smooth tracking)
            screen_width: width,
            screen_height: height,
            monitor_id: config.monitor_id,
        };
        let cursor_service = match CursorService::start(cursor_config) {
            Ok(svc) => {
                info!("Cursor service started for host cursor synchronization");
                Some(svc)
            }
            Err(e) => {
                warn!("Failed to start cursor service: {} — continuing without cursor sync", e);
                None
            }
        };

        // Cursor bridge (same pattern as video/audio)
        let mut cursor_bridge_rx: Option<mpsc::Receiver<CursorUpdate>> = None;
        if let Some(ref svc) = cursor_service {
            let crx = svc.cursor_rx().clone();
            let (ctx, crx_bridge) = mpsc::channel::<CursorUpdate>(8);
            tokio::task::spawn_blocking(move || {
                loop {
                    match crx.recv_timeout(Duration::from_millis(50)) {
                        Ok(update) => {
                            if ctx.blocking_send(update).is_err() {
                                break;
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    }
                }
                log::debug!("Cursor bridge task exited");
            });
            cursor_bridge_rx = Some(crx_bridge);
        }

        // Ping interval
        let mut ping_interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_ping_time = Instant::now();

        // QoS adjustment interval
        let mut qos_interval = tokio::time::interval(Duration::from_secs(3));

        // Media track keyframe gate: don't send P-frames via the media track
        // until we've successfully sent at least one keyframe. This prevents
        // the browser's VP9 decoder from receiving undecodable P-frames that
        // produce rainbow/ghost artifacts.
        let mut media_track_sent_keyframe = false;

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
                // Route through WebRTC media track (primary), DataChannel (fallback),
                // or WebSocket (legacy fallback).
                //
                // When media track is active, the browser renders video natively
                // via `<video>.srcObject` — no WebCodecs/canvas needed.
                frame = video_bridge_rx.recv() => {
                    match frame {
                        Some(video_frame) => {
                            if let Some(ref rtc) = webrtc {
                                // ── Primary path: WebRTC Video Media Track ──────────────
                                // Extract raw VP9/VP8 data from binary protocol message
                                // Protocol: [1B type][4B len][1B codec][1B flags][4B w][4B h][8B ts][data...]
                                // Raw encoded data starts at offset 23
                                let use_media_track = rtc.is_video_track_ready().await
                                    && video_frame.message.len() > 23;

                                if use_media_track {
                                    // When ICE transitions to Connected, the event task
                                    // sets connection_ready_signal to tell us the transport
                                    // is actually open. Reset the keyframe gate so we wait
                                    // for a fresh keyframe that the browser will receive.
                                    if connection_ready_signal.compare_exchange(
                                        true, false,
                                        std::sync::atomic::Ordering::Relaxed,
                                        std::sync::atomic::Ordering::Relaxed,
                                    ).is_ok() {
                                        info!("ICE connected — resetting media track keyframe gate");
                                        media_track_sent_keyframe = false;
                                    }

                                    // Check if this frame is a keyframe (flags byte at offset 6)
                                    let is_keyframe = video_frame.message.len() > 6
                                        && (video_frame.message[6] & FLAG_KEYFRAME) != 0;

                                    // Gate: don't send P-frames via media track until
                                    // we've sent a keyframe. The browser's VP9 decoder
                                    // cannot decode P-frames without a reference keyframe.
                                    if !media_track_sent_keyframe && !is_keyframe {
                                        // Skip media track for this P-frame;
                                        // fall through to DataChannel / WebSocket
                                        match rtc.send_video_frame(&video_frame.message).await {
                                            Ok(true) => {}
                                            Ok(false) | Err(_) => {
                                                if let Err(e) = ws_tx.send(Message::Binary(video_frame.message)).await {
                                                    warn!("Failed to send video frame via WebSocket fallback: {}", e);
                                                    break;
                                                }
                                            }
                                        }
                                    } else {
                                        if is_keyframe {
                                            if !media_track_sent_keyframe {
                                                info!("First keyframe sent via media track — browser can now decode");
                                            }
                                            media_track_sent_keyframe = true;
                                        }

                                        let raw_data = &video_frame.message[23..];
                                        let frame_duration = Duration::from_millis(
                                            1000 / config.framerate.max(1) as u64
                                        );
                                        match rtc.send_video_sample(raw_data, frame_duration).await {
                                            Ok(true) => {} // Sent via media track → <video> element
                                            Ok(false) | Err(_) => {
                                                // Media track send failed — fall through to DataChannel
                                                match rtc.send_video_frame(&video_frame.message).await {
                                                    Ok(true) => {} // Sent via DataChannel
                                                    Ok(false) | Err(_) => {
                                                        // DataChannel also failed — WebSocket fallback
                                                        if let Err(e) = ws_tx.send(Message::Binary(video_frame.message)).await {
                                                            warn!("Failed to send video frame via WebSocket fallback: {}", e);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // ── Fallback: DataChannel (raw binary protocol) ──────
                                    match rtc.send_video_frame(&video_frame.message).await {
                                        Ok(true) => {} // Sent via WebRTC DataChannel
                                        Ok(false) | Err(_) => {
                                            if let Err(e) = ws_tx.send(Message::Binary(video_frame.message)).await {
                                                warn!("Failed to send video frame via WebSocket fallback: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // WebSocket-only path (legacy/fallback)
                                if let Err(e) = ws_tx.send(Message::Binary(video_frame.message)).await {
                                    warn!("Failed to send video frame: {}", e);
                                    break;
                                }
                            }
                        }
                        None => {
                            error!("Video bridge channel closed — video service stopped");
                            break;
                        }
                    }
                }

                // Priority 3: Audio frames via bridge
                // Route through WebRTC DataChannel if available, fallback to WebSocket
                audio = async {
                    if let Some(ref mut rx) = audio_bridge_rx {
                        rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    match audio {
                        Some(audio_frame) => {
                            if let Some(ref rtc) = webrtc {
                                match rtc.send_audio_frame(&audio_frame.message).await {
                                    Ok(true) => {} // Sent via WebRTC
                                    Ok(false) | Err(_) => {
                                        // Fallback to WebSocket
                                        let _ = ws_tx.send(Message::Binary(audio_frame.message)).await;
                                    }
                                }
                            } else {
                                if let Err(e) = ws_tx.send(Message::Binary(audio_frame.message)).await {
                                    warn!("Failed to send audio frame: {}", e);
                                }
                            }
                        }
                        None => {
                            warn!("Audio bridge channel closed");
                        }
                    }
                }

                // Priority 3.5: Cursor updates via bridge
                // Route through WebRTC DataChannel if available, fallback to WebSocket
                cursor = async {
                    if let Some(ref mut rx) = cursor_bridge_rx {
                        rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    if let Some(cursor_update) = cursor {
                        if let Some(ref rtc) = webrtc {
                            match rtc.send_cursor_update(&cursor_update.message).await {
                                Ok(true) => {} // Sent via WebRTC
                                Ok(false) | Err(_) => {
                                    let _ = ws_tx.send(Message::Binary(cursor_update.message)).await;
                                }
                            }
                        } else {
                            if let Err(e) = ws_tx.send(Message::Binary(cursor_update.message)).await {
                                warn!("Failed to send cursor update: {}", e);
                            }
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
                                &webrtc,
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
        if let Some(mut cursor) = cursor_service {
            cursor.stop();
        }
        // Close WebRTC transport if active
        if let Some(rtc) = webrtc_transport {
            if let Err(e) = rtc.close().await {
                warn!("Error closing WebRTC transport: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a text message from the client (control, input, or WebRTC signaling)
    async fn handle_text_message(
        text: &str,
        input_handler: &mut InputHandler,
        video_service: &VideoService,
        qos: &Arc<parking_lot::Mutex<QualityControl>>,
        ctrl_tx: &mpsc::Sender<String>,
        webrtc: &Option<Arc<WebRtcTransport>>,
    ) {
        // ── WebRTC signaling messages ────────────────────────────────────
        // These are handled before control/input parsing because they have
        // a distinct JSON structure (type + sdp/candidate fields).
        if let Ok(signaling) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(msg_type) = signaling.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "webrtc_answer" => {
                        if let Some(ref rtc) = webrtc {
                            if let Some(sdp) = signaling.get("sdp").and_then(|v| v.as_str()) {
                                match rtc.handle_answer(sdp).await {
                                    Ok(_) => info!("WebRTC SDP answer applied successfully"),
                                    Err(e) => error!("Failed to apply WebRTC SDP answer: {}", e),
                                }
                            }
                        }
                        return;
                    }
                    "webrtc_ice_candidate" => {
                        if let Some(ref rtc) = webrtc {
                            if let Some(candidate) = signaling.get("candidate") {
                                let candidate_str = candidate.to_string();
                                match rtc.add_ice_candidate(&candidate_str).await {
                                    Ok(_) => debug!("WebRTC ICE candidate added"),
                                    Err(e) => warn!("Failed to add WebRTC ICE candidate: {}", e),
                                }
                            }
                        }
                        return;
                    }
                    _ => {
                        // Not a WebRTC message — fall through to control/input parsing
                    }
                }
            }
        }

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
                            qos.lock().set_fps(30);
                            qos.lock().set_bitrate(4000);
                        }
                        "low" => {
                            qos.lock().set_fps(15);
                            qos.lock().set_bitrate(1000);
                        }
                        _ => {
                            // "balanced" / default
                            qos.lock().set_fps(24);
                            qos.lock().set_bitrate(1500);
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
