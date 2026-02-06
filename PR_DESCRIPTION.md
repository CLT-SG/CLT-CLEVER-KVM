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