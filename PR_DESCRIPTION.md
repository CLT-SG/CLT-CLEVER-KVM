# Replace H.264/Relay Architecture with RDEngine VP9 Streaming and HTTPS

## Problem

The previous streaming architecture had several issues:

1. H.264 hardware encoding required GPU-specific drivers (NVENC, QuickSync, AMF, VideoToolbox) making it unreliable across different machines
2. The relay server added unnecessary complexity for LAN-only KVM usage (separate Actix Web project with device registry, mDNS discovery, WebSocket relay)
3. The old streaming module was ~6,500 lines across 13 files with deep dependency chains (webrtc, zed-scap, av-data, image, rayon)
4. WebCodecs API was blocked when accessing the web client from LAN devices via HTTP because browsers require a secure context (HTTPS or localhost) for WebCodecs

## Solution

Replaced the entire H.264/relay streaming stack with a RustDesk-inspired VP9 streaming engine (rdengine) and added self-signed HTTPS for WebCodecs support:

1. New rdengine module (~2,200 lines, 6 files) using VP8/VP9 software encoding via libvpx -- works on any machine without GPU dependencies
2. Removed relay server entirely (24 files deleted) -- direct WebSocket streaming only
3. Removed old streaming module (13 files deleted) and audio module (2 files deleted)
4. Added self-signed TLS certificate generation at runtime using rcgen, served via axum-server with rustls for HTTPS
5. Updated web client with VP9 WebCodecs decoder and rdengine binary protocol support

## Changes Made

### New Files - RDEngine Streaming Module
- **src-tauri/src/rdengine/mod.rs**: Module root with public re-exports
- **src-tauri/src/rdengine/codec.rs**: VPX encoder wrapper (VP8/VP9 via libvpx-sys FFI) with BGRA/RGBA to I420 color conversion
- **src-tauri/src/rdengine/video_service.rs**: Dedicated OS thread for capture, dedup, encode, broadcast loop
- **src-tauri/src/rdengine/audio_service.rs**: Audio capture (cpal) with Opus encoding on dedicated thread
- **src-tauri/src/rdengine/connection.rs**: WebSocket connection handler with separated video/control paths
- **src-tauri/src/rdengine/protocol.rs**: Binary frame protocol definitions (video, audio, ping/pong)
- **src-tauri/src/rdengine/qos.rs**: Adaptive quality control (FPS/bitrate from RTT measurement)

### New Files - Web Client
- **src-tauri/web-client/vpx-decoder.js**: VP8/VP9 WebCodecs decoder with hardware acceleration support

### New Files - Documentation
- **docs/RDENGINE_STREAMING_IMPLEMENTATION.md**: Technical documentation for the rdengine architecture

### Modified Files - HTTPS/TLS Support
- **src-tauri/src/network/server/server.rs**: Replaced TcpListener with axum-server bind_rustls, added self-signed certificate generation using rcgen with local IP SANs
- **src-tauri/src/app/commands.rs**: Changed server URL from http to https, removed relay commands, simplified to rdengine settings
- **src-tauri/Cargo.toml**: Added axum-server (tls-rustls), rcgen, rustls, rustls-pemfile, libvpx-sys, cpal, opus, crossbeam-channel, crossbeam-queue, gethostname; removed tokio-tungstenite, tracing, reqwest, hostname, webrtc, zed-scap, image, av-data, rayon, webm, matroska

### Modified Files - Web Client
- **src-tauri/web-client/kvm-client.js**: Added VP8/VP9 decoder integration, rdengine binary protocol parsing, updated server_info handling for codec and protocol_version
- **src-tauri/web-client/kvm-template.html**: Added vpx-decoder.js script reference

### Modified Files - Backend Cleanup
- **src-tauri/src/main.rs**: Removed relay commands, added rdengine module
- **src-tauri/src/network/mod.rs**: Removed relay_client module
- **src-tauri/src/network/server/websocket.rs**: Updated to use rdengine ConnectionHandler
- **src-tauri/src/network/server/handlers.rs**: Simplified handler configuration
- **src-tauri/src/app/state.rs**: Removed RelayClient/RelayStatus, simplified settings to bitrate/fps/codec

### Modified Files - Frontend Cleanup
- **src/App.vue**: Removed RelayStatus component and relay-related state
- **src/components/server/AdvancedSettings.vue**: Replaced H.264/hardware settings with VP9 codec info and simplified bitrate/fps controls
- **src/components/server/PresetSelector.vue**: Updated presets to gaming/desktop/lowBandwidth
- **src/components/server/ServerConfiguration.vue**: Renamed monitor to display
- **src/components/server/index.js**: Removed RelayStatus export
- **src/constants/presets.js**: Simplified presets to bitrate and fps only

### Deleted Files - Old Streaming Module (13 files)
- src-tauri/src/streaming/ (mod.rs, codecs/, enhanced/, handlers/)

### Deleted Files - Audio Module (2 files)
- src-tauri/src/audio/ (mod.rs, engine.rs)

### Deleted Files - Relay Server (24 files)
- relay-server/ (entire directory including src/, templates/, static/)

### Deleted Files - Relay Client and Frontend
- src-tauri/src/network/relay_client.rs
- src/components/server/RelayStatus.vue

### Deleted Files - Old Documentation
- docs/H264_STREAMING_IMPLEMENTATION.md
- docs/WEBM_YUV420_ENHANCEMENT.md
- CHANGELOG.md (root level, moved to docs/)

## Testing

- Verified cargo check passes with 0 errors
- Confirmed rdengine module compiles with libvpx-sys bindings
- Verified HTTPS server starts with self-signed certificate
- Web client auto-detects https to wss for WebSocket connection
- VP9 WebCodecs decoder initializes in secure context

## [5.0.1] Fix VPX Encoder Init, SHM Capture, and VP9 Decoder Recovery

### Problem

1. `libvpx-sys 1.4.2` defines `vpx_codec_enc_cfg_t` as 376 bytes, but system libvpx 1.14.0 requires 504 bytes. When `vpx_codec_enc_config_default()` writes 504 bytes into a 376-byte struct, it causes stack corruption and encoder initialization failure.
2. Screen capture used `get_image()` over the X11 socket (~8-15ms per 1080p frame), which is too slow for real-time streaming.
3. The WebCodecs VP9 hardware decoder rejects valid VP9 frames with "EncodingError: Decoding error" but the client had no software fallback, resulting in a permanent black screen.
4. `spawn_blocking` was called per-frame to receive from crossbeam channels, causing race conditions where orphaned tasks steal frames.
5. WebSocket `binaryType` defaulted to `Blob`, requiring async ArrayBuffer conversion on every frame.
6. Connection health monitor initialized `lastFrameTime` to 0, causing false stale-connection detection and unnecessary reconnections.
7. VP9 codec string hardcoded Level 1.0 (`vp09.00.10.08`) for all resolutions, causing decoder configuration rejection at 1080p.

### Solution

1. Replaced `libvpx-sys` crate with `bindgen 0.70` that generates FFI bindings from the system-installed libvpx headers at build time, guaranteeing struct layouts match exactly (504 bytes confirmed).
2. Added `scrap_capture.rs` with X11 SHM zero-copy screen capture ported from RustDesk's `scrap` library (~1-2ms per frame vs ~8-15ms), with automatic fallback to native capture on non-Linux platforms.
3. Added VP9 decoder hardware-to-software fallback chain: tracks consecutive decode errors, switches `hardwareAcceleration` from `prefer-hardware` to `prefer-software` after first failure, retries keyframes after decoder reinit instead of wasting them, and adds `needsKeyframe` tracking to skip delta frames after configure/error.
4. Replaced per-frame `spawn_blocking` with persistent bridge tasks that use a single `tokio::task::spawn_blocking` loop per channel (video and audio), forwarding frames through a tokio mpsc channel.
5. Set `ws.binaryType = 'arraybuffer'` immediately on WebSocket open.
6. Initialized `lastFrameTime` to `Date.now()` in both constructor and connection open handler.
7. VP9 level is now dynamically selected based on actual resolution (Level 3.1 for 1080p, Level 4.1 for 4K, etc.).

### Changes Made

#### New Files
- **src-tauri/vpx_ffi.h**: Bindgen header including all VPX system headers (vpx_codec.h, vpx_encoder.h, vpx_decoder.h, vpx_image.h, vp8cx.h, vp8dx.h)
- **src-tauri/src/core/scrap_capture.rs**: X11 SHM zero-copy screen capture ported from RustDesk's scrap library, with POSIX shared memory, XCB SHM extension, frame deduplication, BGRA-to-RGBA conversion, and non-Linux platform stubs

#### Modified Files - Backend
- **src-tauri/build.rs**: Added `generate_vpx_bindings()` function using `bindgen::Builder` to generate `vpx_ffi.rs` from system headers, added `cargo:rustc-link-lib=vpx`
- **src-tauri/Cargo.toml**: Removed `libvpx-sys = "1.4"`, added `bindgen = "0.70"` to build-dependencies, added `x11rb` SHM feature for zero-copy capture
- **src-tauri/src/core/mod.rs**: Added `scrap_capture` module export
- **src-tauri/src/rdengine/codec.rs**: Replaced `use vpx_sys::*` with bindgen-generated `mod vpx_ffi`, added explicit constant aliases for bindgen-prefixed enum values, updated union field access from `*pkt.data.frame_ref()` to `pkt.data.frame`, used `VPX_ENCODER_ABI_VERSION` from bindgen
- **src-tauri/src/rdengine/video_service.rs**: Added scrap SHM capture with native fallback, added startup capture+encode verification, improved logging
- **src-tauri/src/rdengine/connection.rs**: Replaced per-frame `spawn_blocking` with persistent crossbeam-to-tokio bridge tasks for both video and audio channels
- **src-tauri/src/app/commands.rs**: Fixed `get_server_url` to return `https://` instead of `http://`

#### Modified Files - Frontend
- **src-tauri/web-client/vpx-decoder.js**: Added `needsKeyframe` tracking, hardware-to-software decoder fallback on decode error, dynamic VP9 level selection based on resolution, keyframe retry after decoder reinit, diagnostic first-frame logging
- **src-tauri/web-client/kvm-client.js**: Request keyframe on all decoder errors (not just delta failures), set `ws.binaryType = 'arraybuffer'`, initialize `lastFrameTime` to `Date.now()`, added JSON ping/pong handler, fixed "H.264 keyframe" log to "keyframe"
- **src/components/server/ServerStatus.vue**: Simplified URL display (backend already returns full https URL with /kvm path)

### Testing

- Verified cargo build passes with 0 errors (85 warnings, all unused code)
- Confirmed bindgen generates correct struct sizes: `vpx_codec_enc_cfg_t` = 504 bytes, `vpx_codec_ctx_t` = 56 bytes, `vpx_image_t` = 136 bytes
- VP9 encode-decode roundtrip validated via C test program (37021 bytes, keyframe, Profile 0)
- Server starts successfully with "Server started successfully" log
- VPX encoder initializes without stack corruption

## [5.0.2] Fix Video Quality, Latency, Input Handling, and Mouse Coordinate Mapping

### Problem

1. Video quality was too low — default bitrate 2000 kbps and max quantizer 56 resulted in blurry, artifact-heavy output at 1080p.
2. Video latency was 1-2 seconds due to encoder buffer sizes set to 600/400/500ms.
3. `set_bitrate()` had a critical bug that reset all encoder configuration (dimensions, threading, CBR mode, buffer sizes) to libvpx defaults on every QoS bitrate adjustment, silently corrupting the encoder.
4. QoS bitrate adjustments calculated by the QoS controller were never applied to the encoder — the encoder continued using its initial bitrate value.
5. Keyboard and mouse input was completely broken — the server `InputMsg` enum expected `button` as `u8`, `code` as required `String`, and `modifiers` as a nested struct, while the client sent `button` as a string (`"left"`), omitted `code`, and sent individual `ctrlKey`/`altKey`/`shiftKey`/`metaKey` booleans.
6. Mouse coordinates were inaccurate because the coordinate calculation used `getBoundingClientRect()` which includes the `object-fit: contain` letterbox/pillarbox black bar areas, resulting in offset mouse positions.
7. Mouse events fired twice per action because listeners were attached to both `videoScreen`, `screenContainer`, and `realCanvas`, with events bubbling from canvas to container.

### Solution

1. Raised default bitrate to 4000 kbps, lowered max quantizer from 56 to 40, and increased min quantizer from 4 to 2 for better quality when bandwidth is available.
2. Reduced encoder buffer sizes from 600/400/500ms to 150/100/120ms for low-latency LAN streaming. Added `rc_dropframe_thresh = 0` to prefer lower quality over frame drops.
3. Fixed `set_bitrate()` to re-apply all custom encoder settings (dimensions, threading, CBR mode, buffer sizes, keyframe config, error resilience) on top of defaults before calling `vpx_codec_enc_config_set()`, so only the bitrate changes while all other settings are preserved. Added early return when the bitrate has not changed.
4. Added QoS-to-encoder bitrate propagation in the video service main loop by tracking `last_applied_bitrate` and calling `encoder.set_bitrate()` when the QoS value differs.
5. Rewrote the `InputMsg` enum in `protocol.rs` to accept flexible types: `button` as `serde_json::Value` (parses both string and number via `parse_button_value()` helper), `code` as `Option<String>`, `key_code` with `#[serde(alias = "keyCode")]`, individual modifier booleans with `#[serde(alias = "ctrlKey")]` etc., and `monitor_id` as `Option<serde_json::Value>`. Added error logging for failed input parsing. Updated client to send `button: e.button` (numeric) and `code: e.code`.
6. Added `getContentRect()` method to compute the actual rendered content rectangle within an element using `object-fit: contain`, excluding black bar areas. Updated `handleMouseEvent()` and `handleTouchEvent()` to use content-relative coordinates and ignore clicks in the black bars.
7. Removed duplicate `videoScreen` mouse event listeners in `setupInputHandlers()`, and added `e.stopPropagation()` to `realCanvas` listeners in `initializeOptimizedCanvas()` to prevent event bubbling to the container handler.

### Changes Made

#### New Files
- **docs/VIDEO_INPUT_FIXES.md**: Documentation detailing all video quality, latency, and input fixes with before/after comparisons

#### Modified Files - Backend
- **src-tauri/src/rdengine/codec.rs**: Raised default bitrate to 4000 kbps, lowered max quantizer to 40, reduced buffer sizes to 150/100/120ms, added `rc_dropframe_thresh = 0`, changed `cpu_speed` from 7 to 6, fixed `set_bitrate()` to preserve all encoder settings, updated preset configs
- **src-tauri/src/rdengine/video_service.rs**: Raised default bitrate to 4000 kbps, added `last_applied_bitrate` tracking and QoS-to-encoder bitrate propagation in main loop
- **src-tauri/src/rdengine/connection.rs**: Raised default bitrate to 4000 kbps, rewrote input parsing to handle flexible `InputMsg` types, added error logging for failed input parsing, updated quality presets (low: 1500 kbps, balanced: 4000 kbps)
- **src-tauri/src/rdengine/qos.rs**: Raised `min_bitrate_kbps` from 400 to 800
- **src-tauri/src/rdengine/protocol.rs**: Rewrote `InputMsg` enum with `serde_json::Value` for `button` and `monitor_id`, made `code` optional, added `key_code` with serde alias, added individual modifier boolean fields with aliases, added `parse_button_value()` helper, added `#[serde(default)]` on wheel fields

#### Modified Files - Frontend
- **src-tauri/web-client/kvm-client.js**: Added `getContentRect()` for object-fit:contain coordinate mapping, fixed `handleMouseEvent()` and `handleTouchEvent()` to use content-relative coordinates, changed mouse button from string to numeric, added `code: e.code` to keyboard events, removed duplicate `videoScreen` mouse listeners, added `e.stopPropagation()` on `realCanvas` listeners, reduced ping interval from 5000ms to 2000ms

### Testing

- Verified cargo check passes with 0 errors (97 warnings)
- Mouse coordinates now map correctly to the remote screen regardless of browser window aspect ratio
- Input events (keyboard, mouse click, mouse move, scroll) successfully reach the server and are processed