//! WebRTC Transport Layer for RDEngine
//!
//! Provides a WebRTC-based transport as an alternative to raw WebSocket binary
//! streaming. WebRTC offers significant advantages for real-time peer-to-peer
//! video streaming:
//!
//! - **UDP transport** — no TCP head-of-line blocking; lost frames are skipped
//!   rather than stalling the entire stream
//! - **DTLS encryption** — built-in secure transport without extra TLS overhead
//! - **ICE connectivity** — automatic NAT traversal for peer-to-peer connections
//! - **Unreliable DataChannel** — ideal for video frames where latest data
//!   matters more than guaranteed delivery
//! - **Native media tracks** — browser handles jitter buffer for audio
//!
//! ## Architecture
//!
//! The WebRTC transport integrates with the existing RDEngine pipeline:
//!
//! ```text
//! VideoService (VP9 encode) ──► WebRTC DataChannel (unreliable) ──► Browser
//! AudioService (Opus encode) ──► WebRTC DataChannel (reliable)  ──► Browser
//! CursorService              ──► WebRTC DataChannel (reliable)  ──► Browser
//! Browser input events       ──► WebSocket (JSON text messages)  ──► Server
//! SDP/ICE signaling          ──► WebSocket (JSON text messages)  ──► Both
//! ```
//!
//! WebSocket is retained for:
//! - SDP offer/answer signaling
//! - ICE candidate exchange
//! - Input events (keyboard/mouse) — these are small, infrequent, and must be reliable
//! - Control messages (keyframe request, QoS updates)

use anyhow::{Context, Result};
use bytes::Bytes;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Notify};
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// Configuration for the WebRTC transport
#[derive(Debug, Clone)]
pub struct WebRtcConfig {
    /// STUN/TURN servers for ICE connectivity
    pub ice_servers: Vec<String>,
    /// Maximum video DataChannel buffer size (bytes)
    pub video_buffer_size: usize,
    /// Whether to use unordered (unreliable) delivery for video
    pub video_unordered: bool,
    /// Max retransmits for video (0 = fire-and-forget, None = reliable)
    pub video_max_retransmits: Option<u16>,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            // No STUN by default — Clever KVM is a LAN application.
            // External STUN servers (e.g. Google) add 5-30s ICE gathering delay.
            // Host candidates are sufficient for same-network peers.
            ice_servers: vec![],
            video_buffer_size: 16 * 1024 * 1024, // 16MB buffer for video frames
            video_unordered: true,                // Unordered = lower latency
            video_max_retransmits: Some(0),       // Fire-and-forget for video
        }
    }
}

impl WebRtcConfig {
    /// LAN-optimized config — no STUN needed, maximum throughput
    #[allow(dead_code)]
    pub fn lan() -> Self {
        Self {
            ice_servers: vec![], // No STUN needed on LAN
            video_buffer_size: 32 * 1024 * 1024,
            video_unordered: true,
            video_max_retransmits: Some(0),
        }
    }
}

/// Events emitted by the WebRTC transport to the connection handler
#[derive(Debug)]
pub enum WebRtcEvent {
    /// DataChannel opened and ready
    VideoChannelReady,
    /// Audio DataChannel opened and ready
    AudioChannelReady,
    /// Cursor DataChannel opened and ready
    CursorChannelReady,
    /// Peer connection state changed
    ConnectionStateChanged(RTCPeerConnectionState),
    /// An ICE candidate was generated (send to remote via signaling)
    IceCandidate(String),
    /// The transport encountered an error
    Error(String),
}

/// The WebRTC transport manages a single peer connection with data channels
/// for video, audio, and cursor data.
pub struct WebRtcTransport {
    /// The underlying WebRTC peer connection
    peer_connection: Arc<RTCPeerConnection>,
    /// DataChannel for video frames (unreliable, unordered for lowest latency)
    video_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    /// DataChannel for audio frames (reliable for Opus)
    audio_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    /// DataChannel for cursor updates (reliable, small messages)
    cursor_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    /// Event sender for notifying the connection handler
    event_tx: mpsc::Sender<WebRtcEvent>,
    /// Whether the video channel is ready for sending
    video_ready: Arc<Notify>,
    /// Configuration
    config: WebRtcConfig,
}

impl WebRtcTransport {
    /// Create a new WebRTC transport.
    ///
    /// Returns the transport and an event receiver for connection lifecycle events.
    pub async fn new(config: WebRtcConfig) -> Result<(Self, mpsc::Receiver<WebRtcEvent>)> {
        info!("Creating WebRTC transport with ICE servers: {:?}", config.ice_servers);

        // Build the WebRTC API
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()
            .context("Failed to register default codecs")?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .context("Failed to register interceptors")?;

        let mut setting_engine = SettingEngine::default();
        // Set shorter ICE timeouts for fast connection on LAN
        setting_engine.set_ice_timeouts(
            Some(std::time::Duration::from_secs(5)),  // disconnected timeout
            Some(std::time::Duration::from_secs(10)), // failed timeout
            Some(std::time::Duration::from_secs(2)),  // keepalive interval
        );

        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        // ICE server configuration
        let ice_servers: Vec<RTCIceServer> = if config.ice_servers.is_empty() {
            vec![] // LAN mode — no external STUN
        } else {
            config.ice_servers.iter().map(|url| RTCIceServer {
                urls: vec![url.clone()],
                ..Default::default()
            }).collect()
        };

        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let peer_connection = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .context("Failed to create RTCPeerConnection")?
        );

        let (event_tx, event_rx) = mpsc::channel::<WebRtcEvent>(64);
        let video_ready = Arc::new(Notify::new());

        let video_channel = Arc::new(Mutex::new(None));
        let audio_channel = Arc::new(Mutex::new(None));
        let cursor_channel = Arc::new(Mutex::new(None));

        // Set up peer connection state change handler
        {
            let event_tx = event_tx.clone();
            peer_connection.on_peer_connection_state_change(Box::new(move |state| {
                let event_tx = event_tx.clone();
                Box::pin(async move {
                    info!("WebRTC peer connection state: {:?}", state);
                    let _ = event_tx.send(WebRtcEvent::ConnectionStateChanged(state)).await;
                    if state == RTCPeerConnectionState::Failed {
                        let _ = event_tx.send(WebRtcEvent::Error(
                            "Peer connection failed".to_string()
                        )).await;
                    }
                })
            }));
        }

        // Set up ICE candidate handler
        {
            let event_tx = event_tx.clone();
            peer_connection.on_ice_candidate(Box::new(move |candidate| {
                let event_tx = event_tx.clone();
                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        match candidate.to_json() {
                            Ok(json) => {
                                let json_str = serde_json::to_string(&json)
                                    .unwrap_or_default();
                                debug!("ICE candidate generated: {}", json_str);
                                let _ = event_tx.send(WebRtcEvent::IceCandidate(json_str)).await;
                            }
                            Err(e) => {
                                warn!("Failed to serialize ICE candidate: {}", e);
                            }
                        }
                    }
                })
            }));
        }

        let transport = Self {
            peer_connection,
            video_channel,
            audio_channel,
            cursor_channel,
            event_tx,
            video_ready,
            config,
        };

        Ok((transport, event_rx))
    }

    /// Create data channels and generate an SDP offer.
    ///
    /// The server acts as the offerer — it creates the data channels and
    /// generates the SDP offer to send to the client via WebSocket signaling.
    pub async fn create_offer(&self) -> Result<String> {
        // Create video data channel (unreliable, unordered for lowest latency)
        let video_dc_init = RTCDataChannelInit {
            ordered: Some(!self.config.video_unordered),
            max_retransmits: self.config.video_max_retransmits,
            ..Default::default()
        };
        let video_dc = self.peer_connection
            .create_data_channel("video", Some(video_dc_init))
            .await
            .context("Failed to create video DataChannel")?;
        info!("Created video DataChannel (unordered={}, max_retransmits={:?})",
            self.config.video_unordered, self.config.video_max_retransmits);

        // Create audio data channel (reliable for Opus)
        let audio_dc = self.peer_connection
            .create_data_channel("audio", None)
            .await
            .context("Failed to create audio DataChannel")?;

        // Create cursor data channel (reliable, small messages)
        let cursor_dc = self.peer_connection
            .create_data_channel("cursor", None)
            .await
            .context("Failed to create cursor DataChannel")?;

        // Set up open/close handlers for each channel
        self.setup_video_channel_handlers(video_dc).await;
        self.setup_audio_channel_handlers(audio_dc).await;
        self.setup_cursor_channel_handlers(cursor_dc).await;

        // Create offer
        let offer = self.peer_connection
            .create_offer(None)
            .await
            .context("Failed to create SDP offer")?;

        // Set local description
        self.peer_connection
            .set_local_description(offer.clone())
            .await
            .context("Failed to set local description")?;

        let sdp = offer.sdp;
        info!("SDP offer created ({} bytes)", sdp.len());
        Ok(sdp)
    }

    /// Handle an SDP answer from the remote peer (received via WebSocket signaling)
    pub async fn handle_answer(&self, sdp: &str) -> Result<()> {
        let answer = RTCSessionDescription::answer(sdp.to_string())
            .context("Failed to parse SDP answer")?;

        self.peer_connection
            .set_remote_description(answer)
            .await
            .context("Failed to set remote description")?;

        info!("SDP answer applied");
        Ok(())
    }

    /// Add a remote ICE candidate (received via WebSocket signaling)
    pub async fn add_ice_candidate(&self, candidate_json: &str) -> Result<()> {
        let candidate: RTCIceCandidateInit = serde_json::from_str(candidate_json)
            .context("Failed to parse ICE candidate JSON")?;

        self.peer_connection
            .add_ice_candidate(candidate)
            .await
            .context("Failed to add ICE candidate")?;

        debug!("Remote ICE candidate added");
        Ok(())
    }

    /// Send a video frame over the WebRTC DataChannel.
    ///
    /// The frame should be a complete binary protocol message (same format as
    /// the WebSocket binary message). This allows the client to use the same
    /// parsing logic regardless of transport.
    ///
    /// Returns Ok(true) if sent, Ok(false) if channel not ready (frame dropped).
    pub async fn send_video_frame(&self, data: &[u8]) -> Result<bool> {
        let channel = self.video_channel.lock().await;
        if let Some(dc) = channel.as_ref() {
            // Check buffered amount to prevent backpressure buildup
            let buffered = dc.buffered_amount().await;
            if buffered > self.config.video_buffer_size {
                debug!("Video DataChannel buffer full ({}B), dropping frame", buffered);
                return Ok(false);
            }

            dc.send(&Bytes::copy_from_slice(data))
                .await
                .context("Failed to send video frame via DataChannel")?;
            Ok(true)
        } else {
            Ok(false) // Channel not ready yet
        }
    }

    /// Send an audio frame over the WebRTC DataChannel (reliable).
    pub async fn send_audio_frame(&self, data: &[u8]) -> Result<bool> {
        let channel = self.audio_channel.lock().await;
        if let Some(dc) = channel.as_ref() {
            dc.send(&Bytes::copy_from_slice(data))
                .await
                .context("Failed to send audio frame via DataChannel")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Send a cursor update over the WebRTC DataChannel (reliable).
    pub async fn send_cursor_update(&self, data: &[u8]) -> Result<bool> {
        let channel = self.cursor_channel.lock().await;
        if let Some(dc) = channel.as_ref() {
            dc.send(&Bytes::copy_from_slice(data))
                .await
                .context("Failed to send cursor update via DataChannel")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if the video DataChannel is open and ready
    #[allow(dead_code)]
    pub async fn is_video_ready(&self) -> bool {
        self.video_channel.lock().await.is_some()
    }

    /// Wait until the video DataChannel is ready
    #[allow(dead_code)]
    pub async fn wait_video_ready(&self) {
        self.video_ready.notified().await;
    }

    /// Get the current peer connection state
    #[allow(dead_code)]
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer_connection.connection_state()
    }

    /// Close the WebRTC transport and free all resources
    pub async fn close(&self) -> Result<()> {
        info!("Closing WebRTC transport");

        // Close data channels
        if let Some(dc) = self.video_channel.lock().await.take() {
            let _ = dc.close().await;
        }
        if let Some(dc) = self.audio_channel.lock().await.take() {
            let _ = dc.close().await;
        }
        if let Some(dc) = self.cursor_channel.lock().await.take() {
            let _ = dc.close().await;
        }

        // Close peer connection
        self.peer_connection.close().await
            .context("Failed to close peer connection")?;

        info!("WebRTC transport closed");
        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────────

    async fn setup_video_channel_handlers(&self, dc: Arc<RTCDataChannel>) {
        let video_channel = self.video_channel.clone();
        let event_tx = self.event_tx.clone();
        let video_ready = self.video_ready.clone();
        let dc_clone = dc.clone();

        dc.on_open(Box::new(move || {
            let video_channel = video_channel.clone();
            let event_tx = event_tx.clone();
            let dc_ref = dc_clone.clone();
            let video_ready = video_ready.clone();
            Box::pin(async move {
                info!("Video DataChannel opened (label: {}, id: {:?})",
                    dc_ref.label(), dc_ref.id());
                *video_channel.lock().await = Some(dc_ref);
                video_ready.notify_waiters();
                let _ = event_tx.send(WebRtcEvent::VideoChannelReady).await;
            })
        }));

        // Note: The server sends video; it doesn't receive on this channel.
        // If the client needs to send data back (e.g., acknowledgements),
        // it would go through the WebSocket control channel instead.
    }

    async fn setup_audio_channel_handlers(&self, dc: Arc<RTCDataChannel>) {
        let audio_channel = self.audio_channel.clone();
        let event_tx = self.event_tx.clone();
        let dc_clone = dc.clone();

        dc.on_open(Box::new(move || {
            let audio_channel = audio_channel.clone();
            let event_tx = event_tx.clone();
            let dc_ref = dc_clone.clone();
            Box::pin(async move {
                info!("Audio DataChannel opened (label: {}, id: {:?})",
                    dc_ref.label(), dc_ref.id());
                *audio_channel.lock().await = Some(dc_ref);
                let _ = event_tx.send(WebRtcEvent::AudioChannelReady).await;
            })
        }));
    }

    async fn setup_cursor_channel_handlers(&self, dc: Arc<RTCDataChannel>) {
        let cursor_channel = self.cursor_channel.clone();
        let event_tx = self.event_tx.clone();
        let dc_clone = dc.clone();

        dc.on_open(Box::new(move || {
            let cursor_channel = cursor_channel.clone();
            let event_tx = event_tx.clone();
            let dc_ref = dc_clone.clone();
            Box::pin(async move {
                info!("Cursor DataChannel opened (label: {}, id: {:?})",
                    dc_ref.label(), dc_ref.id());
                *cursor_channel.lock().await = Some(dc_ref);
                let _ = event_tx.send(WebRtcEvent::CursorChannelReady).await;
            })
        }));
    }
}

impl Drop for WebRtcTransport {
    fn drop(&mut self) {
        debug!("WebRtcTransport dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webrtc_config_defaults() {
        let config = WebRtcConfig::default();
        assert!(config.video_unordered);
        assert_eq!(config.video_max_retransmits, Some(0));
        assert!(!config.ice_servers.is_empty());
    }

    #[tokio::test]
    async fn test_webrtc_config_lan() {
        let config = WebRtcConfig::lan();
        assert!(config.ice_servers.is_empty());
        assert!(config.video_unordered);
    }
}
