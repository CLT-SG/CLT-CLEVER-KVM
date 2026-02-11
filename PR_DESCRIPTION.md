# Replace H.264/Relay Architecture with RDEngine VP9 Streaming and HTTPS

## [5.0.9] WebRTC Media Track for Native Video Rendering with Keyframe Gating

### Problem

1. Video frames were delivered via WebRTC DataChannels and decoded manually using WebCodecs `VideoDecoder` + canvas `drawImage()`, adding unnecessary CPU overhead when the browser can decode VP9 natively via a `<video>` element.
2. When a WebRTC media track was added, the browser's VP9 decoder received P-frames before any keyframe, producing rainbow colors and ghost overlay artifacts because P-frames cannot be decoded without a reference keyframe.
3. The browser had no way to request a keyframe from the server when its decoder lost sync (e.g., after packet loss), because RTCP PLI (Picture Loss Indication) packets were not parsed on the server side.
4. The keyframe was requested immediately after `create_offer()` -- before ICE negotiation completed. The generated keyframe was sent to a media track that the browser was not yet connected to, causing a noticeable delay before video appeared on the web client.

### Solution

1. Added a VP9/VP8 video media track (`TrackLocalStaticSample`) to the WebRTC PeerConnection. The browser receives it via `pc.ontrack` and assigns the `MediaStream` to a `<video>` element's `srcObject` for native hardware-accelerated decoding and rendering. DataChannel + WebCodecs + canvas rendering is retained as a fallback.
2. Added a keyframe-first gate (`media_track_sent_keyframe`) that blocks P-frames on the media track until a keyframe has been sent. Pre-keyframe P-frames are routed to DataChannel / WebSocket fallback instead.
3. Added RTCP PLI parsing in the `rtp_sender.read()` task. When the browser sends a PLI packet, the server forces the VPX encoder to produce a keyframe immediately via `keyframe_signal`.
4. Moved the keyframe request from `create_offer()` time to `ConnectionStateChanged(Connected)` time. Added a `connection_ready_signal` (`AtomicBool`) shared between the event task and main loop that resets the keyframe gate when ICE connects, ensuring the first keyframe the browser receives is through the now-connected transport.

### Changes Made

#### Modified Files - Backend
- **src-tauri/src/rdengine/webrtc_transport.rs**: Added `TrackLocalStaticSample` media track creation in `create_offer(codec)`, `send_video_sample()` method, `is_video_track_ready()` method, RTCP PLI reader task, `VideoTrackReady` and `KeyframeRequested` event variants, media track cleanup in `close()`
- **src-tauri/src/rdengine/connection.rs**: Added `AtomicBool` import and `FLAG_KEYFRAME` import, `connection_ready_signal` for ICE-connected gate reset, `keyframe_signal` shared with spawned event task, `VideoTrackReady`/`KeyframeRequested`/`ConnectionStateChanged(Connected)` handlers, `media_track_sent_keyframe` gate with keyframe-first routing logic, media track primary path with DataChannel and WebSocket fallback chain
- **src-tauri/src/rdengine/video_service.rs**: Added `keyframe_signal()` method returning `Arc<AtomicBool>` clone for async task access

#### Modified Files - Frontend
- **src-tauri/web-client/webrtc-transport.js**: Added `onMediaStream` callback, `videoStream`/`hasMediaTrack` state, `pc.ontrack` handler for video media tracks with track lifecycle events, `hasVideoMediaTrack()`/`getVideoStream()` methods, media track cleanup in `close()`
- **src-tauri/web-client/kvm-client.js**: Added `usingVideoElement` state, `activateVideoElementRendering(stream)` with `requestVideoFrameCallback` FPS tracking, `deactivateVideoElementRendering()` fallback to canvas, `onMediaStream` callback in WebRTC transport setup, guards in `handleVpxFrame()`/`handleH264Frame()` to skip canvas when using video element, updated mouse/touch coordinate mapping for video element, updated host cursor rendering target, updated connection health monitoring for video playback state
- **src-tauri/web-client/kvm-template.html**: Updated video element comment and inline styling for native media track rendering, updated codec dropdown labels to "VP9 (Native Video)" / "VP8 (Native Video)"

#### Modified Files - Documentation
- **docs/RDENGINE_STREAMING_IMPLEMENTATION.md**: Updated overview from DataChannel to Media Track transport, updated transport table with media track primary and DataChannel fallback rows, updated architecture diagrams with media track and dual rendering paths, added Native Video Element Rendering section, updated VP9 Decoding section as fallback, updated dependency tables and comparison table

### Testing

- Video renders via native `<video>` element using WebRTC media track (no canvas)
- No rainbow or overlay artifacts -- keyframe-first gate blocks P-frames until keyframe delivered
- Browser PLI requests trigger immediate keyframe generation on the server
- Video appears immediately when web client connects (keyframe requested after ICE connects)
- Automatic fallback to DataChannel + canvas rendering when media track unavailable
- Connection health monitor correctly detects active video playback via `<video>` element state

## [5.0.8] Replace H.264 Codec Defaults with VP9 Across UI and Server

### Problem

1. The web client template, codec dropdown, KVM_CONFIG, and server handlers all defaulted to H.264, contradicting the RDEngine VP9/WebRTC architecture documented in RDENGINE_STREAMING_IMPLEMENTATION.md.
2. The codec dropdown in the OSD was hardcoded to a single disabled "H.264 (Hardware)" option, preventing users from selecting VP8 or VP9.
3. The `KVM_CONFIG.codec` was hardcoded to `"h264"` instead of using the server-provided `{{codec}}` template variable, so the server's codec preference was ignored.
4. Server-side handlers (`kvm_client_handler`, `ws_handler`, `ws_handler_with_stop`) all defaulted to `"h264"` when no codec parameter was provided.
5. The codec dropdown had no change event listener, making codec switching impossible even if options were present.
6. VPX decoder was initialized after H.264 decoder despite being the primary codec, and stale H.264 references throughout kvm-client.js caused confusion.

### Solution

1. Changed all default codec values from `"h264"` to `"vp9"` across server handlers, client config, and fallback defaults.
2. Replaced the disabled single-option H.264 dropdown with an enabled VP9/VP8 selector.
3. Changed `KVM_CONFIG.codec` from hardcoded `"h264"` to dynamic `"{{codec}}"` so the server template variable is used.
4. Added a codec dropdown change event listener that updates `currentCodec`/`serverCodec` and triggers a WebSocket reconnect to apply the new codec.
5. Reordered decoder initialization to prioritize VPX over H.264 (legacy fallback).
6. Updated all stale H.264 comments and defaults throughout the codebase while retaining H.264 decoder as a functional legacy fallback.

### Changes Made

#### Modified Files - Backend
- **src-tauri/src/network/server/handlers.rs**: Changed default codec from `"h264"` to `"vp9"` in `kvm_client_handler`, `ws_handler`, and `ws_handler_with_stop`; updated comments

#### Modified Files - Frontend
- **src-tauri/web-client/kvm-template.html**: Changed video element comment to VP9/WebRTC, replaced disabled H.264 dropdown with VP9/VP8 options, changed `KVM_CONFIG.codec` from `"h264"` to `"{{codec}}"`, reordered script tags to load vpx-decoder.js before h264-decoder.js, marked h264-decoder.js as legacy fallback
- **src-tauri/web-client/kvm-client.js**: Changed `currentCodec` default from `"h264"` to `config.codec || "vp9"`, reordered `initializeVpxDecoder()` before `initializeH264Decoder()`, added codec dropdown change event listener with reconnect, updated `handleServerInfo` default codec to `"vp9"`, updated `handleStreamInfo` to sync dropdown to `currentCodec`, updated `handleWebRTCFrame` default codec to `"vp9"`, updated fallback config default to `"vp9"`, replaced stale H.264 comments throughout
- **src-tauri/web-client/kvm-template-parts.js**: Changed codec dropdown initialization from hardcoded `"h264"` to `config.codec || "vp9"`, updated comment

### Testing

- Codec dropdown displays VP9 (WebRTC) and VP8 (WebRTC) options
- Selecting VP8 triggers WebSocket reconnect with codec=vp8 in URL
- Server defaults to vp9 when no codec parameter is provided
- KVM_CONFIG.codec reflects server template variable
- H.264 legacy fallback path remains functional for rdengine codec ID 0x03

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