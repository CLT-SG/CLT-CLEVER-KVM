# Replace H.264/Relay Architecture with RDEngine VP9 Streaming and HTTPS

## [5.0.7] WebRTC DataChannel Transport for Low-Latency Streaming

### Problem

1. Video, audio, and cursor data were sent over WebSocket (TCP), which suffers from head-of-line blocking -- a single lost packet stalls all subsequent frames until retransmitted.
2. TCP retransmissions added unpredictable latency spikes to the video stream, especially on congested or lossy networks.
3. No UDP-based peer-to-peer transport existed for real-time streaming on LAN.
4. The `rustls` crate panicked at runtime with "no process-level CryptoProvider" because both `ring` (via webrtc/dtls) and `aws-lc-rs` (via axum-server) features were enabled, and rustls could not auto-detect which provider to use.
5. External STUN servers (e.g. `stun.l.google.com:19302`) caused 5-30 second ICE gathering delays on LAN connections where they are unnecessary.
6. During WebRTC negotiation, video frames were silently dropped while the DataChannel was not yet ready, causing a delayed video startup of up to 30 seconds.

### Solution

1. Added WebRTC DataChannel transport with three channels: video (unreliable, unordered UDP for lowest latency), audio (reliable), and cursor (reliable). WebSocket is retained for SDP/ICE signaling, keyboard/mouse input, and control messages.
2. Created `webrtc_transport.rs` (Rust server-side) and `webrtc-transport.js` (browser client-side) to manage WebRTC PeerConnection lifecycle, DataChannel creation, and SDP/ICE signaling via WebSocket.
3. Updated `connection.rs` to route video/audio/cursor through WebRTC DataChannels with automatic fallback to WebSocket binary when the DataChannel is not ready or encounters errors.
4. Added explicit `rustls::crypto::ring::default_provider().install_default()` at application startup before any TLS usage to resolve the CryptoProvider conflict.
5. Configured LAN-optimized ICE with no external STUN servers -- host candidates are sufficient for same-network peers, eliminating gathering delays.
6. Bumped binary protocol version from 2 to 3, added `webrtc_enabled` field to `ServerInfo` so clients can detect WebRTC support.

### Changes Made

#### New Files
- **src-tauri/src/rdengine/webrtc_transport.rs**: WebRTC transport module -- `WebRtcTransport` struct with `WebRtcConfig`, `WebRtcEvent` enum, three DataChannels (video/audio/cursor), SDP offer/answer, ICE candidate handling, backpressure control via buffered amount check, `close()` cleanup
- **src-tauri/web-client/webrtc-transport.js**: Client-side `WebRtcTransport` class -- handles SDP offer from server, creates RTCPeerConnection, sets up DataChannel handlers for video/audio/cursor, forwards ICE candidates via WebSocket, connection state monitoring, transport statistics

#### Modified Files - Backend
- **src-tauri/Cargo.toml**: Added `webrtc = "0.17"` and `bytes = "1"` dependencies
- **src-tauri/src/main.rs**: Added `rustls::crypto::ring::default_provider().install_default()` after `env_logger::init()` to resolve CryptoProvider conflict between ring and aws-lc-rs
- **src-tauri/src/rdengine/mod.rs**: Added `pub mod webrtc_transport` and `pub use webrtc_transport::{WebRtcTransport, WebRtcConfig}`
- **src-tauri/src/rdengine/protocol.rs**: Added `pub webrtc_enabled: bool` to `ServerInfo`, protocol_version bumped to 3
- **src-tauri/src/rdengine/connection.rs**: Added `enable_webrtc` and `webrtc_config` to `ConnectionConfig`, WebRTC transport setup with SDP offer generation, ICE candidate event forwarding, video/audio/cursor routing through DataChannel with WebSocket fallback on `Ok(false)` and `Err(_)`, WebRTC signaling message handling (`webrtc_answer`, `webrtc_ice_candidate`), cleanup on disconnect
- **src-tauri/src/network/server/websocket.rs**: Both `handle_h264_socket()` and `handle_ultra_connection()` now use `enable_webrtc: true` with `WebRtcConfig::lan()` for LAN-optimized config

#### Modified Files - Frontend
- **src-tauri/web-client/kvm-client.js**: Added `webrtcTransport`, `webrtcEnabled`, `webrtcConnected` state, `handleWebRTCOffer()` creates WebRtcTransport routing DataChannel frames to existing binary handlers, `handleMessage()` handles `webrtc_offer` and `webrtc_ice_candidate`, `cleanupConnection()` closes WebRTC transport
- **src-tauri/web-client/kvm-template.html**: Added `<script src="/static/webrtc-transport.js"></script>` between vpx-decoder.js and kvm-client.js

#### Modified Files - Documentation
- **docs/RDENGINE_STREAMING_IMPLEMENTATION.md**: Updated architecture overview, added transport architecture table, WebRTC signaling flow diagram, WebRTC Transport section, updated module structure and dependency tree, added performance comparison (WebRTC vs WebSocket), updated comparison table with three-column format (H.264 / RDEngine v2 WebSocket / RDEngine v3 WebRTC)

### Testing

- Verified cargo check passes with 0 errors
- Video frames delivered via WebSocket immediately during WebRTC negotiation (no 30s delay)
- WebRTC DataChannel connects within 1-2 seconds on LAN (no STUN delay)
- Video transitions from WebSocket to WebRTC DataChannel seamlessly
- Client displays "WebRTC connected -- low-latency mode active" notification on successful connection
- Automatic fallback to WebSocket binary when WebRTC is unavailable