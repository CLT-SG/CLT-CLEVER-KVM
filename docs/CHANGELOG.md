# Clever KVM Release Changelog

## Version History

## [5.0.11] - 2026-02-11

### Standardize Log Format and Remove Legacy H.264 References

### Bug Fixes
- **Emoji Log Prefixes**: Rust backend and shell scripts used system emojis in log messages that can cause encoding issues and render inconsistently across platforms
- **Unused FFmpeg Linking**: build.rs linked FFmpeg libraries (avformat, avcodec, avutil, swscale, swresample) that were legacy from H.264 pipeline, no longer used with VP9/libvpx
- **Misleading H.264 Naming**: websocket.rs contained H264Config enum variant and handle_h264_socket function despite only using VP9 codec via rdengine
- **Dead Codec Constant**: protocol.rs contained unused CODEC_H264 constant
- **Wrong Codec References**: Build scripts output messages referenced H.264 instead of VP9

### Improvements
- **Standardized Rust Logs**: All emoji prefixes in Rust log macros replaced with text prefixes: [INFO], [OK], [WARNING], [ERROR], [DEBUG], [TIP]
- **Standardized Script Logs**: All emoji prefixes in shell scripts replaced with text prefixes: [INFO], [COMPLETE], [WARNING], [ERROR]
- **Removed FFmpeg Legacy**: Removed unused FFmpeg library linking from build.rs
- **VP9-Only Naming**: Renamed H264Config to StreamingConfig, handle_h264_socket to handle_streaming_socket
- **Clean Protocol**: Removed CODEC_H264 constant from protocol.rs
- **Correct Build Output**: Updated build.sh and build.bat to reference VP9 hardware-accelerated encoding
- **Reduced Warnings**: Added #![allow(dead_code)] to public API modules, reducing cargo check warnings from 96 to 42

### Technical Changes
- **src-tauri/build.rs**: Removed FFmpeg library linking
- **src-tauri/src/rdengine/protocol.rs**: Removed CODEC_H264, added #![allow(dead_code)]
- **src-tauri/src/rdengine/*.rs**: Added #![allow(dead_code)] to codec.rs, video_service.rs, audio_service.rs, cursor_service.rs, connection.rs, qos.rs, webrtc_transport.rs
- **src-tauri/src/network/server/websocket.rs**: Renamed H264Config/handle_h264_socket, replaced emojis, added #![allow(dead_code)]
- **src-tauri/src/network/server/server.rs**: Replaced emojis with [INFO]/[OK] prefixes
- **src-tauri/src/network/server/handlers.rs**: Added #![allow(dead_code)]
- **src-tauri/src/network/server/models.rs**: Added #![allow(dead_code)]
- **src-tauri/src/system/system_optimizer.rs**: Replaced emojis, added #[allow(dead_code)]
- **src-tauri/src/core/capture.rs**: Replaced emojis, added #![allow(dead_code)]
- **src-tauri/src/core/native_capture.rs**: Replaced emojis, added #![allow(dead_code)]
- **src-tauri/src/core/scrap_capture.rs**: Added #![allow(dead_code)]
- **src-tauri/src/app/commands.rs**: Replaced emojis with [INFO]/[OK]/[WARNING] prefixes
- **src-tauri/src/lib/constants.rs**: Added #![allow(dead_code)]
- **src-tauri/src/lib/error_types.rs**: Added #![allow(dead_code)]
- **scripts/build.sh**: Updated H.264 to VP9 references
- **scripts/build.bat**: Updated H.264 to VP9 references
- **scripts/prepare-release.sh**: Replaced emojis with [INFO]/[COMPLETE]
- **scripts/setup-github-secrets.sh**: Replaced emojis with [INFO]/[COMPLETE]/[WARNING]/[ERROR]
- **scripts/test-updater.sh**: Replaced emojis with [INFO]/[COMPLETE]/[ERROR]

### Remove H.264 Legacy Codec from Web Client

### Bug Fixes
- **Dead H.264 Code**: Web client contained ~300 lines of H.264 decoder initialization, frame handling, and YUV fallback rendering code that was never executed since RDEngine uses VP9/VP8 exclusively
- **Wrong Codec Configuration**: normalizeCodec() returned 'h264' and getCodecConfigurations() returned AVC codec strings despite VP9 being the only supported codec
- **Misleading Comments**: Code comments and log messages referenced H.264 decoder and frame handling paths that no longer exist in the architecture

### Improvements
- **VP9-Only Codebase**: Removed all H.264 decoder properties, initialization methods, frame handlers, and YUV fallback renderers from kvm-client.js
- **Correct Codec Strings**: normalizeCodec() now returns 'vp9' or 'vp8', getCodecConfigurations() returns VP9/VP8 WebCodecs codec strings
- **Cleaner Video Pipeline**: handleBinaryVideoFrame() no longer checks for "H264" magic header, handleRdEngineVideoFrame() no longer has H.264 fallback branch

### Technical Changes
- **src-tauri/web-client/kvm-client.js**: Removed h264Decoder/h264SPS/h264PPS properties, removed initializeH264Decoder()/handleH264Frame()/handleH264VideoFrame()/renderH264Fallback()/renderH264FallbackLegacy()/renderH264FallbackGrayscale()/bilinearSample() methods, removed H.264 reset from resetDecoderState(), removed H.264 magic header detection, updated processVideoQueue()/normalizeCodec()/getCodecConfigurations() for VP9, updated comments and logs

### Replace System Emojis with SVG Icons in Web Client

### Bug Fixes
- **Inconsistent Emoji Rendering**: Web client UI buttons and settings sections used system emojis that rendered differently across operating systems and browsers
- **Non-Standard Console Prefixes**: Console log messages used emoji prefixes that can cause encoding issues and are non-standard for debugging output

### Improvements
- **SVG Icon System**: Replaced all UI emojis with inline SVG icons using Feather icon style (stroke-based, 16px) for consistent cross-platform rendering
- **Standardized Log Prefixes**: Console log emojis replaced with text prefixes [ERROR], [WARNING], [INFO], [DEBUG] for consistent debugging output

### Technical Changes
- **src-tauri/web-client/kvm-client.js**: Replaced emoji prefixes in console.log/warn/error with [ERROR]/[WARNING]/[INFO]/[DEBUG], removed emoji from canvas text display
- **src-tauri/web-client/kvm-template.html**: Replaced button emojis (fullscreen, settings, disconnect) with inline SVG icons, replaced settings section title emojis (Display, Audio, Performance) with inline SVG icons
- **src-tauri/web-client/kvm-client.css**: Added .icon CSS class for SVG sizing (16px), .osd-button .icon and .section-title .icon styles for stroke and fill properties

### Cross-Platform File-Based Logging for Logs Tab

### Bug Fixes
- **Logs Tab Not Working**: Logs tab displayed "Error log not found or accessible" on all platforms because get_logs() used hardcoded Linux paths (/tmp/clever-kvm-debug.log) that were never created
- **No File Logging**: env_logger::init() only outputs to console (stderr), log files were never written to disk
- **Clear Button Non-Functional**: Clear button only cleared UI state, did not clear actual log files

### Improvements
- **Cross-Platform Log Directory**: Logs stored in platform-appropriate directories using dirs crate (Windows: %LOCALAPPDATA%, macOS: ~/Library/Application Support, Linux: ~/.local/share)
- **File-Based Logging**: Added fern logger with dual output to clever-kvm.log (all levels) and clever-kvm-error.log (WARN and above)
- **In-Memory Buffers**: Recent logs kept in memory for instant UI access without file I/O
- **Timestamped Entries**: All log entries include timestamp, level, and target module
- **Log File Paths Display**: UI shows actual log file locations for user reference
- **Backend Clear Function**: Clear button now removes logs from both memory buffers and disk files

### Technical Changes
- **src-tauri/Cargo.toml**: Added fern = "0.7" and chrono = "0.4" dependencies
- **src-tauri/src/lib/logger.rs** (new): Cross-platform logging module with get_log_directory(), init_logging(), read_debug_log(), read_error_log(), clear_logs(), get_log_paths()
- **src-tauri/src/lib/mod.rs**: Export logger module and public functions
- **src-tauri/src/main.rs**: Replaced env_logger::init() with init_logging(), registered clear_app_logs and get_log_file_paths commands
- **src-tauri/src/app/commands.rs**: Updated get_logs() to use read_debug_log()/read_error_log(), added clear_app_logs() and get_log_file_paths() commands
- **src/components/server/LogViewer.vue**: Added logPaths state, clearLogs() calls clear_app_logs backend, added log file paths info section with .log-paths styling

## [5.0.10] - 2026-02-11

### Matrix Dark Theme UI Redesign with Interactive Server Controls

### Bug Fixes
- **Outdated Light Theme Styling**: Application UI used default light theme that was inconsistent across components
- **Redundant Server Control Buttons**: Separate Start and Stop buttons took unnecessary vertical space in Status tab
- **Emoji Icons Instead of SVG**: Components used emoji icons that rendered inconsistently across platforms
- **Suboptimal Content Density**: Font sizes and application height caused unnecessary scrolling in some views
- **Misplaced UpdateChecker**: UpdateChecker was in Status tab instead of Settings tab where it belongs
- **Unused Appearance Section**: Appearance Settings section existed with only one theme option

### Improvements
- **Matrix Dark Theme**: Global dark theme with CSS variables for colors (primary green #00ff41), backgrounds (#0a0a0a), spacing, shadows, and effects
- **Clickable Status Ring**: Status indicator converted to interactive button that toggles server start/stop on click with visual hover feedback
- **SVG Icon System**: All emoji icons replaced with inline SVG icons using Feather/Material Design style (stroke-based, 18-20px)
- **Increased Window Height**: Application window height increased 10% from 600px to 660px for better content visibility
- **Reduced Font Size**: Base font size reduced 10% from 16px to 14px for improved content density
- **Relocated UpdateChecker**: UpdateChecker moved to Settings tab above Application Settings section
- **Removed Appearance Section**: Appearance Settings removed since only Matrix Dark theme exists

### Technical Changes
- **src/assets/styles/theme.css** (new): Global Matrix dark theme CSS with variables for colors, backgrounds, text, borders, shadows, transitions, radius, spacing, and fonts
- **src/components/ui/SettingsPanel.vue** (new): Settings panel with UpdateChecker, Startup Settings (auto-start, start minimized, auto-start server), System Tray Settings (show icon, minimize to tray, notifications)
- **src/main.js**: Added theme.css import
- **src/App.vue**: Updated tabs with icon identifiers, moved UpdateChecker to Settings tab via SettingsPanel
- **src/components/ui/TabContainer.vue**: Added inline SVG icons for tabs (status, config, options, logs, settings), Matrix theme styling with glow effects
- **src/components/ui/index.js**: Added SettingsPanel export
- **src/components/server/ServerStatus.vue**: Status ring button with toggleServer function, hover states show action icons, loading spinner
- **src/components/server/ConnectionOptions.vue**: SVG icons in features array, grid layout for options/features, copy button for example URL
- **src/components/server/ServerConfiguration.vue**: Card-based layout with config-card styling, label hints
- **src/components/server/PresetSelector.vue**: Card-based presets with SVG icons, spec display, active state indicator
- **src/components/server/LogViewer.vue**: Matrix theme with section icons, line counts, refresh/clear buttons, empty states
- **src/components/update/UpdateChecker.vue**: Card layout with icons, last checked timestamp, status messages
- **src/components/update/UpdaterDialog.vue**: Modal transitions, status icons, gradient progress bar
- **src/composables/useServer.js**: Relay command tracking, displays/loadingDisplays aliases
- **src-tauri/tauri.conf.json**: Window height changed from 600 to 660

## [5.0.9] - 2026-02-09

### WebRTC Media Track for Native Video Rendering with Keyframe Gating

### Bug Fixes
- **Rainbow/Overlay Video Artifacts**: Browser VP9 decoder received P-frames before any keyframe via the media track, producing rainbow colors and ghost overlay artifacts because P-frames cannot be decoded without a reference keyframe
- **No RTCP PLI Handling**: Browser had no way to request a keyframe from the server when its decoder lost sync after packet loss, because RTCP PLI packets were not parsed
- **Delayed Video Appearance**: Keyframe was requested immediately after `create_offer()` before ICE negotiation completed, so the keyframe was sent to a media track the browser was not connected to yet -- video only appeared after the encoder's periodic keyframe interval

### Improvements
- **Native Video Rendering**: Video delivered via WebRTC media track (`TrackLocalStaticSample`) for native `<video>` element rendering with hardware-accelerated VP9/VP8 decoding -- no WebCodecs or canvas needed
- **Keyframe-First Gating**: P-frames blocked on the media track until a keyframe has been sent, preventing decoder artifacts; pre-keyframe P-frames routed to DataChannel/WebSocket fallback
- **RTCP PLI Keyframe Requests**: Server parses RTCP PLI packets from the browser and immediately forces the encoder to produce a keyframe
- **ICE-Connected Keyframe Timing**: Keyframe requested when `ConnectionStateChanged(Connected)` fires instead of at `create_offer()` time, with `connection_ready_signal` to reset the keyframe gate so the browser receives a fresh keyframe through the connected transport
- **Dual Rendering Paths**: Primary path uses `<video>.srcObject` from WebRTC media track; fallback path uses WebCodecs `VideoDecoder` + canvas via DataChannel
- **Video Element FPS Tracking**: `requestVideoFrameCallback` tracks FPS and updates `lastFrameTime` for connection health monitoring
- **Connection Health for Video Element**: Health monitor checks `<video>` playback state when using native rendering, preventing false stale-stream detection

### Technical Changes
- **src-tauri/src/rdengine/webrtc_transport.rs**: Added `TrackLocalStaticSample` media track in `create_offer(codec)`, `send_video_sample()`, `is_video_track_ready()`, RTCP PLI reader task, `VideoTrackReady` and `KeyframeRequested` events, media track cleanup in `close()`
- **src-tauri/src/rdengine/connection.rs**: Added `connection_ready_signal` (`AtomicBool`) for ICE gate reset, `keyframe_signal` shared with event task, `media_track_sent_keyframe` gate with keyframe-first routing, media track primary path with DataChannel/WebSocket fallback chain, keyframe request on `ConnectionStateChanged(Connected)`
- **src-tauri/src/rdengine/video_service.rs**: Added `keyframe_signal()` method returning `Arc<AtomicBool>` for async task keyframe triggering
- **src-tauri/web-client/webrtc-transport.js**: Added `onMediaStream` callback, `pc.ontrack` handler, `videoStream`/`hasMediaTrack` state, `hasVideoMediaTrack()`/`getVideoStream()` methods, media track cleanup
- **src-tauri/web-client/kvm-client.js**: Added `activateVideoElementRendering(stream)` with `requestVideoFrameCallback` FPS tracking, `deactivateVideoElementRendering()` canvas fallback, `usingVideoElement` state, video element coordinate mapping for mouse/touch/cursor, health monitor video playback state check
- **src-tauri/web-client/kvm-template.html**: Updated `<video>` element styling for native media track rendering, codec dropdown labels to "Native Video"
- **docs/RDENGINE_STREAMING_IMPLEMENTATION.md**: Updated transport table, architecture diagrams, browser client docs, dependency tables, and comparison table for media track architecture

## [5.0.8] - 2026-02-09

### Replace H.264 Codec Defaults with VP9 Across UI and Server

### Bug Fixes
- **Hardcoded H.264 Default Codec**: Web client template, KVM_CONFIG, and all server handlers defaulted to H.264 despite the RDEngine architecture using VP9/WebRTC for streaming
- **Disabled Codec Dropdown**: The codec dropdown in the OSD was hardcoded to a single disabled "H.264 (Hardware)" option, preventing codec selection
- **Ignored Server Codec Template Variable**: `KVM_CONFIG.codec` was hardcoded to `"h264"` instead of using the server-provided `{{codec}}` template variable
- **No Codec Switching**: The codec dropdown had no change event listener, making runtime codec switching impossible
- **Wrong Decoder Priority**: H.264 decoder was initialized before VPX decoder despite VP9 being the primary codec

### Improvements
- **VP9 Default Codec**: All default codec values changed from `"h264"` to `"vp9"` across server handlers, client config, and fallback defaults
- **VP9/VP8 Codec Dropdown**: Replaced disabled H.264 dropdown with enabled VP9 (WebRTC) and VP8 (WebRTC) options in the OSD
- **Dynamic Codec Config**: `KVM_CONFIG.codec` now uses the server template variable `"{{codec}}"` instead of a hardcoded value
- **Runtime Codec Switching**: Added codec dropdown change event listener that updates codec state and triggers WebSocket reconnect to apply the new codec
- **VPX Decoder Priority**: VPX decoder initialized before H.264 decoder; H.264 retained as legacy fallback

### Technical Changes
- **src-tauri/src/network/server/handlers.rs**: Default codec changed from `"h264"` to `"vp9"` in `kvm_client_handler`, `ws_handler`, and `ws_handler_with_stop`
- **src-tauri/web-client/kvm-template.html**: Video element comment updated, codec dropdown replaced with VP9/VP8 options, `KVM_CONFIG.codec` changed to `"{{codec}}"`, script load order changed (vpx-decoder.js first, h264-decoder.js as legacy fallback)
- **src-tauri/web-client/kvm-client.js**: `currentCodec` default changed to `config.codec || "vp9"`, decoder init order swapped, codec dropdown change listener added, `handleServerInfo`/`handleStreamInfo`/`handleWebRTCFrame` defaults changed to `"vp9"`, fallback config default changed to `"vp9"`, stale H.264 comments updated
- **src-tauri/web-client/kvm-template-parts.js**: Codec dropdown initialization changed from `"h264"` to `config.codec || "vp9"`

## [5.0.7] - 2026-02-09

### WebRTC DataChannel Transport for Low-Latency Streaming

### Bug Fixes
- **TCP Head-of-Line Blocking**: Video/audio/cursor data sent over WebSocket TCP caused head-of-line blocking -- a single lost packet stalled all subsequent frames until retransmitted
- **rustls CryptoProvider Panic**: Both `ring` (via webrtc/dtls) and `aws-lc-rs` (via axum-server) features were enabled on rustls, causing a runtime panic "no process-level CryptoProvider" because rustls could not auto-detect which provider to use
- **STUN Server Delay**: External STUN servers (`stun.l.google.com:19302`) added 5-30 second ICE gathering delays on LAN connections where NAT traversal is not needed
- **30-Second Video Startup Delay**: Video frames were silently dropped when the WebRTC DataChannel was not yet ready during negotiation, causing no video to appear until the channel opened

### Improvements
- **WebRTC DataChannel Transport**: Added three DataChannels for media delivery -- video (unreliable/unordered UDP, fire-and-forget), audio (reliable), cursor (reliable) -- eliminating TCP head-of-line blocking for real-time streaming
- **Hybrid Transport Architecture**: WebSocket retained for SDP/ICE signaling, keyboard/mouse input, and control messages; WebRTC used for all media data
- **WebSocket Fallback During Negotiation**: Video/audio/cursor frames fall back to WebSocket binary when the DataChannel is not ready or encounters errors, ensuring immediate video display during WebRTC setup
- **LAN-Optimized ICE**: No external STUN servers configured -- host candidates are sufficient for same-network peers, enabling sub-second ICE connection
- **Explicit CryptoProvider**: `rustls::crypto::ring::default_provider().install_default()` called at startup to resolve the ring/aws-lc-rs conflict
- **Protocol Version 3**: Binary protocol bumped from v2 to v3 with `webrtc_enabled` field in `ServerInfo` so clients can detect WebRTC support
- **Backpressure Control**: Video DataChannel checks `buffered_amount()` before sending; frames are dropped if buffer exceeds configured threshold to prevent unbounded memory growth

### Technical Changes
- **src-tauri/Cargo.toml**: Added `webrtc = "0.17"` (pure Rust WebRTC) and `bytes = "1"` dependencies
- **src-tauri/src/main.rs**: Added `rustls::crypto::ring::default_provider().install_default()` after `env_logger::init()`
- **src-tauri/src/rdengine/webrtc_transport.rs** (new): `WebRtcTransport` struct, `WebRtcConfig` (default/lan), `WebRtcEvent` enum, three DataChannels, SDP offer/answer, ICE candidate handling, backpressure control, `close()` cleanup
- **src-tauri/src/rdengine/connection.rs**: Added `enable_webrtc`/`webrtc_config` to `ConnectionConfig`, WebRTC transport setup with SDP offer, ICE event forwarding, video/audio/cursor routing through DataChannel with WebSocket fallback, signaling message handling (`webrtc_answer`, `webrtc_ice_candidate`)
- **src-tauri/src/rdengine/protocol.rs**: Added `webrtc_enabled: bool` to `ServerInfo`, protocol_version 3
- **src-tauri/src/rdengine/mod.rs**: Added `pub mod webrtc_transport` and re-exports
- **src-tauri/src/network/server/websocket.rs**: Both handlers use `enable_webrtc: true` with `WebRtcConfig::lan()`
- **src-tauri/web-client/webrtc-transport.js** (new): Client-side `WebRtcTransport` class, RTCPeerConnection management, DataChannel handlers, ICE candidate forwarding, connection state monitoring
- **src-tauri/web-client/kvm-client.js**: Added `webrtcTransport`/`webrtcEnabled`/`webrtcConnected` state, `handleWebRTCOffer()` creates transport and routes frames to existing handlers, signaling message handling, cleanup on disconnect
- **src-tauri/web-client/kvm-template.html**: Added `<script src="/static/webrtc-transport.js"></script>`
- **docs/RDENGINE_STREAMING_IMPLEMENTATION.md**: Updated architecture overview, transport table, signaling flow, module structure, performance comparison, three-column comparison table

## [5.0.6] - 2026-02-09

### Persistent TLS Certificates and Infinite WebSocket Reconnection

### Bug Fixes
- **TLS Certificate Regenerated on Every Restart**: Self-signed TLS certificate was regenerated each time the server restarted, causing browsers to silently reject the new wss:// certificate (close code 1006) and fail all WebSocket connections
- **Reconnection Stops After 5 Attempts**: WebSocket reconnection had a hard cap of 5 retries, after which the client permanently gave up and required a manual page reload
- **Reconnect Counter Never Incremented**: `reconnectAttempts` was never incremented in the `ws.onclose` handler, making the retry count meaningless
- **Overlapping Reconnect Timers**: No guard against multiple `setTimeout` reconnect timers running simultaneously, causing duplicate WebSocket connections
- **Stale WebSocket Event Handlers**: Old WebSocket `onmessage`/`onerror` handlers were not nulled before closing, allowing callbacks to fire on the previous socket after a new connection was created
- **Decoder State Not Reset**: VPX decoder was left in a stale state after reconnection (closed decoder, wrong `needsKeyframe` flag), causing decode errors on the new stream
- **VideoFrame Memory Leak**: Pending `VideoFrame` references from the previous connection were never closed, leaking GPU memory

### Improvements
- **TLS Certificate Persistence**: Certificates are now saved to `~/.local/share/clever-kvm/server.crt` and `server.key`, reused across restarts, and only regenerated if missing or corrupt
- **Infinite Reconnection**: Changed from 5 attempts to infinite retry with exponential backoff (1-10 second cap), with page reload fallback after 30 consecutive failures
- **Clean Connection Teardown**: New `cleanupConnection()` method properly nulls WebSocket handlers, cancels animation frames, closes pending VPX frames, and clears host cursor state
- **Decoder Reinitialization**: New `resetDecoderState()` destroys and reinitializes both VPX and H264 decoders on each reconnection
- **Canvas Cleared to Black**: Display clears to solid black on disconnect instead of showing a frozen last frame
- **Always-Visible Status Display**: Status display moved outside OSD overlay with `z-index: 200` so reconnection messages are visible without hovering
- **Troubleshooting Tips**: Reconnection states now show common troubleshooting steps (check server running, refresh page, check network, check firewall)
- **Loading Spinner Reorder**: Spinner positioned above the title text with description and tips below

### Technical Changes
- **src-tauri/src/network/server/server.rs**: Added `get_cert_dir()` returning `~/.local/share/clever-kvm/`, added `load_or_generate_cert()` that loads existing cert/key from disk or generates and persists new ones, `WebSocketServer::new()` calls `load_or_generate_cert()` instead of `generate_self_signed_cert()`
- **src-tauri/web-client/kvm-client.js**: `maxReconnectAttempts` from 5 to `Infinity`, added `_reconnectTimer`/`_isReconnecting`/`_consecutiveFailures`/`_maxConsecutiveFailuresBeforeReload` state, added `cleanupConnection()`, `resetDecoderState()`, `_cancelPendingReconnect()`, `_scheduleReconnect()`, `clearCanvasToBlack()`, duplicate-call guard in `connect()`, troubleshooting tips in `updateStatus()`, removed attempt counts from status messages
- **src-tauri/web-client/kvm-template.html**: Moved `.status-display` from inside `.osd-overlay` to directly inside `#screen`, reordered children to spinner/h2/p/`.status-tips`
- **src-tauri/web-client/kvm-client.css**: `.status-display` with `z-index: 200` and `pointer-events: none`, flexbox column with CSS `order` properties, `.status-tips` bullet list with `:empty { display: none }`, spinner 36px

## [5.0.5] - 2026-02-09

### Low-Latency Default Quality and Adaptive Quality Fixes

### Bug Fixes
- **Quality Update Protocol Mismatch**: Client `applyQualityLevel()` sent numeric quality values (65/80/95) but server expected string names ("low"/"balanced"/"high") -- adaptive quality changes never reached the encoder
- **Quality Change Message Type**: Client `switchQuality()` sent `type: 'quality_change'` but server only parsed `type: 'quality_update'` -- manual quality dropdown changes were silently ignored
- **QoS Auto-Ramp to Max on LAN**: QoS controller aggressively increased fps by 10% and bitrate by 10% every 3 seconds on low-latency LAN connections (RTT < 30ms), pushing to max_fps=60 and max_bitrate=12000 kbps and labeling quality as "high"
- **Client Default Quality Too High**: Web client initialized with `adaptiveQuality.currentLevel = 'high'`, `qualityLevel = 85`, and `currentQuality = 'medium'` -- every connection started requesting high-quality encoding

### Improvements
- **Low-Latency Encoder Defaults**: Default bitrate reduced from 4000 to 1500 kbps, framerate from 30 to 24fps, cpu_speed increased to 9 (VP9 maximum), max_quantizer raised to 56-63 for faster per-frame encoding
- **Low Quality Default**: Web client now defaults to low quality level on every connection for maximum streaming smoothness
- **Conservative QoS Adjustment**: QoS max_fps capped at 30, max_bitrate at 4000 kbps, LAN quality ramp changed from +10% to +1fps/+5% bitrate, initial quality level set to "low"
- **Capped Auto-Promotion**: Adaptive quality system can no longer auto-promote to "high" -- maximum auto-promotion is "medium", requires sustained excellent performance (<0.5% drops, fps >= 22)
- **Stricter Quality Upgrade Thresholds**: Auto quality only recommends "high" under excellent conditions (bandwidth >10Mbps, latency <20ms, packet loss <0.5%)
- **Lowered Quality Presets**: "high" from 60fps/8000kbps to 30fps/4000kbps, "balanced" from 30fps/4000kbps to 24fps/1500kbps, "low" bitrate from 1500 to 1000 kbps

### Technical Changes
- **src-tauri/src/rdengine/codec.rs**: Default bitrate 1500 kbps, framerate 24, cpu_speed 9, rc_max_quantizer 63 (init) / 52 (set_bitrate), encoder buffers 60/40/50ms, LAN preset 30fps/1x bitrate, balanced preset 24fps/0.75x bitrate
- **src-tauri/src/network/server/websocket.rs**: Standard config 24fps/1500kbps, ultra config 30fps/2000kbps
- **src-tauri/src/rdengine/connection.rs**: Default 24fps/1500kbps, mpsc(2) bridge channel, 50ms bridge timeout, preset adjustments (high: 30fps/4000kbps, balanced: 24fps/1500kbps, low: 15fps/1000kbps)
- **src-tauri/src/rdengine/qos.rs**: max_fps 30, min_bitrate 500, max_bitrate 4000, thresholds 80ms/20ms, initial quality "low", LAN ramp +1fps/+5% capped at "balanced"
- **src-tauri/web-client/kvm-client.js**: qualityLevel 50, adaptiveQuality.currentLevel "low", currentQuality "low", applyQualityLevel sends string names, switchQuality uses type "quality_update", auto-adapt caps at "medium", stricter thresholds
- **src-tauri/web-client/vpx-decoder.js**: Decoder queue drop threshold 3 to 8
- **docs/VIDEO_INPUT_FIXES.md**: Added Section 3 with 7 real-time streaming latency optimizations and latency budget table

## [5.0.3] - 2026-02-09

### Host Cursor Synchronization and Control Priority

### New Features
- **Host Cursor Tracking**: Added dedicated cursor service that polls the host system cursor position via X11 QueryPointer at ~30 Hz on a dedicated OS thread, with delta compression to only transmit updates when position or shape changes
- **Cursor Overlay Rendering**: Web client renders the host cursor as an SVG arrow overlay with "Host" label, positioned accurately over the remote screen content area using coordinate mapping
- **Host Control Priority**: When the host is actively moving the cursor, the client's native cursor is hidden and client mouse input is suppressed to prevent cursor fighting -- control returns to the client after 500ms of host inactivity
- **MSG_CURSOR Binary Protocol**: Extended rdengine protocol with MSG_CURSOR (0x03) messages -- 15 bytes per update containing position (x, y), cursor shape identifier, and visibility flag
- **Client Activity Awareness**: Host cursor overlay is automatically hidden when the web client user moves their own mouse -- overlay only appears when the remote host is the one moving the cursor, with smooth CSS opacity fade transitions

### Improvements
- **Default Cursor Style**: Changed client cursor from crosshair to standard default arrow cursor across all screen elements (#screen, canvas, video)
- **Cursor Shape Mapping**: 23 cursor shapes mapped from host to CSS cursor names (default, pointer, text, wait, crosshair, move, resize variants, grab, not-allowed, help, progress)
- **Client Override of Host Control**: Client mouse activity always takes precedence over host control mode -- if the client moves their mouse, input is never blocked and the native cursor is restored immediately

### Technical Changes
- **src-tauri/src/rdengine/cursor_service.rs** (new): CursorService with X11 LinuxCursorReader, CursorShape enum, CursorUpdate struct, encode_cursor_message(), CursorServiceConfig, dedicated thread with crossbeam channel, non-Linux FallbackCursorReader
- **src-tauri/src/rdengine/mod.rs**: Added cursor_service module and CursorService re-export
- **src-tauri/src/rdengine/connection.rs**: Cursor service startup, crossbeam-to-tokio bridge task, MSG_CURSOR sending in select! loop, cursor service cleanup
- **src-tauri/web-client/kvm-client.js**: hostCursor state, CURSOR_SHAPE_MAP, handleCursorMessage(), setHostControlling(), renderHostCursor(), setClientCursorStyle(), host control check in handleMouseEvent(), MSG_CURSOR dispatch, crosshair to default cursor, clientActive state tracking, setClientActive() method, hideHostCursor() with CSS fade-out, client activity precedence over host control
- **src-tauri/web-client/kvm-client.css**: Replaced crosshair with default cursor, added #host-cursor-overlay styles with SVG arrow, Host label badge, smooth transitions, shape-specific variants, opacity transition for smooth hide/show, .host-cursor-hidden class

## [5.0.2] - 2026-02-09

### Video Quality, Latency, Input Handling, and Mouse Coordinate Fixes

### Bug Fixes
- **Encoder set_bitrate Corruption**: Fixed `set_bitrate()` which reset all encoder configuration (dimensions, threading, CBR mode, buffer sizes) to libvpx defaults on every QoS bitrate adjustment, silently corrupting the encoder — now re-applies all custom settings before updating bitrate
- **QoS Bitrate Never Applied**: QoS controller calculated new bitrate values but never propagated them to the encoder — added `last_applied_bitrate` tracking in video service main loop to apply changes via `encoder.set_bitrate()`
- **Input Type Mismatch**: Server `InputMsg` enum expected `button` as `u8` and `code` as required `String`, while client sent `button` as string and omitted `code` — all input events silently failed JSON deserialization, breaking keyboard and mouse entirely
- **Mouse Coordinate Offset**: Mouse coordinate calculation used `getBoundingClientRect()` which includes `object-fit: contain` letterbox/pillarbox black bar areas — clicks and moves landed at wrong positions on the remote screen
- **Duplicate Mouse Events**: Mouse event listeners were attached to both `videoScreen`, `screenContainer`, and `realCanvas`, causing every mouse action to fire the handler twice due to event bubbling
- **Silent Input Errors**: Failed input message parsing was silently ignored — added warning-level logging with error details and raw message content for diagnostics

### Improvements
- **Video Quality**: Raised default bitrate from 2000 to 4000 kbps, lowered max quantizer from 56 to 40, raised min quantizer from 4 to 2 for sharper 1080p output
- **Video Latency**: Reduced encoder buffer sizes from 600/400/500ms to 150/100/120ms for low-latency LAN streaming, added `rc_dropframe_thresh = 0` to never drop frames
- **Encoder Speed**: Changed `cpu_speed` from 7 to 6 for better quality with minimal CPU cost increase
- **QoS Floor**: Raised minimum bitrate floor from 400 to 800 kbps to keep quality usable under congestion
- **Quality Presets**: Updated low preset from 800 to 1500 kbps, balanced from 2000 to 4000 kbps
- **Ping Frequency**: Reduced ping interval from 5000ms to 2000ms for faster QoS feedback

### Technical Changes
- **src-tauri/src/rdengine/codec.rs**: Encoder defaults, buffer sizes, `set_bitrate()` bug fix preserving all encoder settings, preset config updates
- **src-tauri/src/rdengine/video_service.rs**: Default bitrate 4000 kbps, QoS-to-encoder bitrate propagation
- **src-tauri/src/rdengine/connection.rs**: Default bitrate 4000 kbps, input parsing rewrite for flexible types, error logging, quality preset updates
- **src-tauri/src/rdengine/qos.rs**: Minimum bitrate floor raised to 800 kbps
- **src-tauri/src/rdengine/protocol.rs**: `InputMsg` enum rewrite — `button` as `serde_json::Value`, `code` optional, `key_code` with alias, individual modifier booleans with serde aliases, `parse_button_value()` helper
- **src-tauri/web-client/kvm-client.js**: Added `getContentRect()` for object-fit:contain coordinate mapping, fixed mouse/touch coordinate calculation, mouse button numeric format, keyboard `code` field, removed duplicate event listeners, `stopPropagation` on canvas, ping interval 2000ms
- **docs/VIDEO_INPUT_FIXES.md** (new): Documentation of all video quality, latency, and input fixes

## [5.0.1] - 2026-02-06

### VP9 Encoder Fix, SHM Capture, and Decoder Error Recovery

### Bug Fixes
- **VPX Encoder Init Crash**: Replaced `libvpx-sys 1.4.2` (376-byte `vpx_codec_enc_cfg_t`) with `bindgen 0.70` to generate FFI bindings from system libvpx headers at build time, matching the required 504-byte struct layout for libvpx 1.14.0
- **VP9 Decode Black Screen**: Added hardware-to-software decoder fallback when WebCodecs hardware VP9 decoder rejects valid frames, plus `needsKeyframe` tracking to prevent sending delta frames to a freshly configured decoder
- **Keyframe Wasted After Reinit**: When the decoder is closed and a keyframe arrives, the decoder now reinitializes and retries the current keyframe instead of discarding it
- **spawn_blocking Race Condition**: Replaced per-frame `spawn_blocking` calls with persistent crossbeam-to-tokio bridge tasks to prevent orphaned tasks from stealing frames
- **WebSocket Binary Format**: Set `ws.binaryType = 'arraybuffer'` on connection open to avoid async Blob-to-ArrayBuffer conversion overhead
- **Health Monitor False Reconnect**: Initialized `lastFrameTime` to `Date.now()` instead of 0 to prevent false stale-connection detection
- **VP9 Codec String Level**: Changed from hardcoded Level 1.0 to dynamic level selection based on resolution (Level 3.1 for 1080p, Level 4.1 for 4K)
- **Server URL Protocol**: Fixed `get_server_url` to return `https://` instead of `http://`
- **JSON Ping Handler**: Added server ping/pong JSON message handler for connection health monitoring
- **Keyframe Request Log**: Fixed log message from "Requesting H.264 keyframe" to "Requesting keyframe"

### New Features
- **X11 SHM Zero-Copy Capture**: Added `scrap_capture.rs` ported from RustDesk's `scrap` library, using POSIX shared memory and XCB SHM extension for ~1-2ms capture latency (vs ~8-15ms with `get_image`)
- **Startup Pipeline Verification**: Video service now captures and encodes a test frame during startup to verify the entire pipeline works before entering the main loop
- **Bindgen VPX Bindings**: Build-time FFI generation from system libvpx headers ensures struct layout compatibility regardless of installed libvpx version

### Technical Changes
- **src-tauri/vpx_ffi.h** (new): Bindgen header including vpx_codec.h, vpx_encoder.h, vpx_decoder.h, vpx_image.h, vp8cx.h, vp8dx.h
- **src-tauri/src/core/scrap_capture.rs** (new): X11 SHM capturer with BGRA-to-RGBA conversion, frame deduplication, and non-Linux platform stubs
- **src-tauri/build.rs**: Added `generate_vpx_bindings()` using `bindgen::Builder`
- **src-tauri/Cargo.toml**: Removed `libvpx-sys = "1.4"`, added `bindgen = "0.70"`, added `x11rb` SHM feature
- **src-tauri/src/rdengine/codec.rs**: Replaced `use vpx_sys::*` with bindgen-generated module and constant aliases
- **src-tauri/src/rdengine/video_service.rs**: Added scrap SHM capture with native fallback and startup verification
- **src-tauri/src/rdengine/connection.rs**: Persistent bridge tasks for crossbeam-to-tokio channel forwarding
- **src-tauri/web-client/vpx-decoder.js**: HW-to-SW fallback, needsKeyframe tracking, dynamic VP9 level, keyframe retry on reinit
- **src-tauri/web-client/kvm-client.js**: ArrayBuffer binaryType, ping handler, keyframe on all errors, lastFrameTime fix

## [5.0.0] - 2026-02-06

### RDEngine VP9 Streaming with HTTPS Support

### New Features
- **RDEngine Streaming Module**: New RustDesk-inspired VP8/VP9 streaming engine using libvpx software encoding via direct FFI bindings
- **Dedicated Video Thread**: Screen capture, frame deduplication, BGRA-to-I420 color conversion, and VPX encoding on a dedicated OS thread (not async)
- **Opus Audio over WebSocket**: Audio capture via cpal with Opus encoding, sent as binary WebSocket frames instead of WebRTC
- **Binary Protocol**: Minimal-overhead length-prefixed binary framing (23 bytes per video frame header, 19 bytes per audio frame header)
- **Adaptive QoS**: RTT-based quality control that adjusts FPS (5-60) and bitrate (400-12000 kbps) automatically
- **Frame Deduplication**: Byte-compare frames before encoding to skip unchanged screens, reducing CPU usage on static desktops
- **HTTPS with Self-Signed TLS**: Runtime certificate generation using rcgen with local IP SANs, served via axum-server with rustls
- **VP8/VP9 WebCodecs Decoder**: Browser-side hardware-accelerated VP9 decoding via WebCodecs VideoDecoder API
- **RDEngine Binary Protocol Client**: Web client parses 0x01 (video), 0x02 (audio), 0x05/0x06 (ping/pong) binary messages

### Technical Stack
- **Video Encoding**: libvpx-sys 1.4 (VP8/VP9 via direct C FFI)
- **Audio Encoding**: cpal 0.15 (capture) + opus 0.3 (encoding)
- **Channels**: crossbeam-channel 0.5 + crossbeam-queue 0.3 (lock-free MPMC)
- **TLS**: axum-server 0.7 (tls-rustls) + rcgen 0.12 + rustls 0.23
- **Allocator**: mimalloc 0.1

### Removed Components
- **Old Streaming Module**: Removed 13 files (~6,500 lines) including H.264 encoder, realtime codec, YUV420 encoder, ultra-low latency handler, integrated handler, low-latency pipeline
- **Relay Server**: Removed entire relay-server directory (24 files) including Actix Web server, device registry, mDNS discovery, WebSocket relay, dashboard templates
- **Relay Client**: Removed relay_client.rs and RelayStatus.vue
- **Audio Module**: Removed old audio/engine.rs and audio/mod.rs
- **Unused Dependencies**: Removed tokio-tungstenite, tracing, reqwest, hostname, webrtc, zed-scap, image, av-data, rayon, webm, matroska
- **Old Documentation**: Removed H264_STREAMING_IMPLEMENTATION.md and WEBM_YUV420_ENHANCEMENT.md

### Frontend Updates
- **Simplified Settings**: Replaced H.264/hardware acceleration controls with VP9 codec info badge and bitrate/fps sliders
- **Updated Presets**: Simplified to gaming (12000kbps/60fps), desktop (6000kbps/30fps), lowBandwidth (2000kbps/15fps)
- **Removed Relay UI**: Removed RelayStatus component, relay state management, and relay-related composable functions
- **Display Naming**: Renamed monitor references to display throughout the frontend

## [4.1.0] - 2025-08-14

### Screen Capture System Overhaul: Native scap Integration

### New Features
- **Native scap Screen Capture**: Migrated from `xcap` to `scap` 0.0.8 for cross-platform screen recording
- **Linux Desktop Portal Support**: Full integration with XDG Desktop Portals for secure screen capture
- **Multi-Format Frame Support**: Handles BGRA, RGB, RGBx, BGRx, XBGR, BGR0, and YUV frame formats
- **Clean API Architecture**: Simplified screen capture interface following `scap` best practices
- **Permission Management**: Proper permission checking and requesting for screen capture access

### Screen Capture Implementation
- **Cross-Platform Compatibility**: Works on Windows, macOS, and Linux with native APIs
- **Desktop Portal Integration**: Seamless integration with Linux desktop environments (GNOME, KDE, etc.)
- **Automatic Format Conversion**: Real-time conversion between frame formats (BGRA→RGBA, etc.)
- **Monitor Enumeration**: Simplified monitor detection with fallback for systems without available displays
- **Cursor Capture Support**: Optional cursor overlay with platform-specific optimizations

### Bug Fixes
- **Monitor Detection**: Fixed "Monitor not found: 0" errors on Linux systems
- **Permission Handling**: Improved desktop portal permission flow for screen capture
- **Frame Format Issues**: Resolved frame type compatibility across different platforms
- **API Cleanup**: Removed deprecated `get_all_targets` duplicated code and streamlined implementation

### Code Cleanup
- **Removed Duplicated Code**: Eliminated redundant `get_all_targets` usage and complex target filtering
- **Simplified Initialization**: Streamlined capturer creation following `scap` example patterns  
- **Clean Dependencies**: Removed unused imports and simplified module structure
- **Standard API Usage**: Aligned with official `scap` documentation and examples

### Technical Improvements
- **Memory Efficiency**: Reduced memory overhead with optimized frame handling
- **Error Handling**: Enhanced error reporting for desktop portal and permission issues
- **Logging Integration**: Added comprehensive logging for screen capture operations
- **Performance Optimization**: Streamlined frame capture pipeline with reduced latency

## [3.0.0] - 2025-08-12

### Major Architecture Update: Native WebM Implementation

### New Features
- **Native WebM Encoding**: Completely replaced FFmpeg with pure Rust libraries
- **Zero External Dependencies**: Eliminated all FFmpeg requirements across all platforms
- **50% Smaller Binaries**: Reduced installer size and memory footprint significantly
- **Ultra-Low Latency**: Sub-50ms total latency with performance budgeting system
- **Native Opus Audio**: CD-quality audio with WebM container synchronization

### Removed Dependencies
- **FFmpeg Completely Eliminated**: No longer required on any platform
- **Removed Scripts**: Deleted `fix-ffmpeg.sh` and `fix-ffmpeg.bat` troubleshooting tools
- **Simplified Workflows**: Updated GitHub Actions to remove all FFmpeg installation steps
- **Clean Build Process**: No more vcpkg, pkg-config, or external codec dependencies

### Native WebM Stack
- **WebM Container**: `webm = "1.1"` and `matroska = "0.14"` for multiplexing
- **VP8 Video Encoding**: Direct VP8 with YUV420 color space optimization
- **Opus Audio Codec**: `opus = "0.3"` with multiple quality profiles (96k-320k)
- **Image Processing**: `image = "0.24"` for optimized YUV420 color conversion
- **Performance**: `parking_lot = "0.12"`, `rayon = "1.8"`, `mimalloc = "0.1"`

### Performance Improvements
- **Memory Usage**: 50% reduction compared to FFmpeg-based solutions
- **Binary Size**: Significantly smaller installers without external codec libraries  
- **Latency**: Sub-50ms end-to-end for competitive gaming and real-time interaction
- **Quality**: 40-60% better compression efficiency with native YUV420 processing
- **Stability**: Elimination of DLL dependency issues and codec installation problems

### Developer Experience
- **Simplified Setup**: No external dependencies to install or configure
- **Faster Builds**: No FFmpeg compilation or linking required
- **Cross-Platform**: Consistent behavior across Windows, macOS, and Linux
- **Self-Contained**: All required codecs built into the application binary

---

## [1.1.1] - 2025-07-24 (DEPRECATED - FFmpeg-based)

### Bug Fixes
- **FFmpeg Runtime**: Fixed missing FFmpeg DLLs causing "avcodec-61.dll was not found" errors on Windows
- **macOS Cross-compilation**: Resolved architecture conflicts in universal binary builds
- **Dependencies**: Upgraded to ffmpeg-sys-next 7.1.0 for better version consistency

### Improvements
- **Windows Bundling**: FFmpeg DLLs are now automatically bundled with Windows installers
- **Linux Dependencies**: Added FFmpeg libraries to .deb package dependencies
- **Build Process**: Streamlined FFmpeg installation using AnimMouse/setup-ffmpeg with platform-specific versions
- **Version Alignment**: Synchronized FFmpeg binary version (7.1) with Rust binding versions

### Distribution
- **Windows**: .msi and .exe installers now include all required FFmpeg DLLs
- **Linux**: .deb packages automatically install FFmpeg dependencies via package manager
- **macOS**: Universal binaries include FFmpeg libraries for both x86_64 and ARM64

### Development
- **Workflows**: Updated GitHub Actions to bundle FFmpeg libraries during build process
- **Cross-platform**: Improved build consistency across Windows, macOS, and Linux
- **Dependencies**: Hybrid approach using AnimMouse/setup-ffmpeg + platform-specific dev libraries

### Technical Changes
- Added `"resources": ["libs/*.dll"]` to tauri.conf.json for Windows DLL bundling
- Updated .deb dependencies to include libavcodec59, libavformat59, libavutil57, etc.
- Removed conflicting brew FFmpeg installation on macOS for universal builds
- Set platform-specific FFmpeg versions (macOS: 7.1, others: 7.1)

### Impact
- **End Users**: No longer need to separately install FFmpeg
- **Developers**: Simplified build process with consistent FFmpeg versions
- **Distribution**: Self-contained installers work out-of-the-box

### [1.1.0] - 2025-07-22
- **Auto-Updater Implementation**: Added comprehensive auto-updater functionality
  - Automatic update detection on app startup
  - Manual update checks via UI button
  - Cryptographically signed updates for security
  - User-friendly update dialog with progress tracking
  - Background downloads with one-click installation
  - Cross-platform support (Windows, macOS, Linux)
- **Enhanced Build System**: Improved GitHub Actions workflows with update signing
  - Fixed FFmpeg dependency installation for all platforms
  - Fixed Ubuntu linker error (libxcb-randr0-dev missing dependency)
  - Fixed macOS compilation errors (async Send trait issues, missing Key variants)
  - Fixed Windows FFmpeg build with proper vcpkg integration
  - Added comprehensive system dependency management
  - Optimized Windows builds with Chocolatey
  - Enhanced macOS builds with Homebrew integration
  - Improved Linux builds with proper FFmpeg dev libraries
- **Troubleshooting Tools**: Added FFmpeg build troubleshooting scripts
  - Cross-platform dependency checker (`fix-ffmpeg.sh` / `fix-ffmpeg.bat`)
  - Automatic FFmpeg installation and configuration
  - Environment variable setup for build success
- **Documentation**: Added comprehensive auto-updater documentation and guides
  - Enhanced README with FFmpeg installation instructions
  - Detailed BUILD.md with troubleshooting guides
  - Complete pull request description with technical details
- **Testing Tools**: Added scripts for testing and validating updater functionality
 - Initial VP8 encoding implementation
 - WebSocket-based streaming
 - Cross-platform desktop application
 - Multi-monitor support
 - Audio streaming capabilities

### [1.0.0] - 2025-07-22
- Initial release
- Basic KVM functionality
- VP8 video encoding
- WebRTC audio support
- Cross-platform compatibility (Windows, macOS, Linux)

---

## Release Process

To create a new release:

1. Update version numbers using the prepare-release script:
   ```bash
   ./scripts/prepare-release.sh X.Y.Z
   ```

2. Update this CHANGELOG.md with new features and fixes

3. Commit changes:
   ```bash
   git add .
   git commit -m "Release vX.Y.Z"
   ```

4. Create and push tag:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

5. GitHub Actions will automatically:
   - Build for all platforms
   - Create installers
   - Create GitHub release with artifacts
