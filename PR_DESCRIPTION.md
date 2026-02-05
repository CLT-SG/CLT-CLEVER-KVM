# Fix Windows Screen Capture Compatibility Issues

## Problem

The application failed to compile and run on Windows due to breaking API changes between the `scap`/`zed-scap` screen capture library and `windows-capture 1.5.0`. The original dependencies had incompatible version requirements causing build failures and runtime crashes when attempting screen capture.

## Solution

Replaced the problematic `scap`/`zed-scap` dependency with native platform-specific screen capture implementations. This provides:

- Stable screen capture using direct platform API calls
- Cross-platform support for Windows, Linux (X11), and macOS
- Windows: GDI (GetDC, BitBlt, GetDIBits) for maximum compatibility
- Linux: X11 library (XGetImage) with RandR extension for multi-monitor support
- macOS: Core Graphics (CGDisplayCreateImage) for Quartz display capture
- No external dependency conflicts
- Reliable RGBA frame output for streaming

## Changes Made

### Core Changes
- **src-tauri/Cargo.toml**: Replaced `scap` with `zed-scap 0.0.8-zed` and pinned `windows-capture` to version `1.4.4` to avoid breaking changes
- **src-tauri/Cargo.toml**: Added platform-specific dependencies for Linux (x11rb with randr) and macOS (core-graphics, core-foundation)
- **src-tauri/src/core/native_capture.rs**: New cross-platform native screen capture module supporting Windows GDI, Linux X11, and macOS Core Graphics
- **src-tauri/src/core/capture.rs**: Refactored to use the new native capture backend with cross-platform documentation
- **src-tauri/src/core/mod.rs**: Added native_capture module export

### Streaming Updates
- **src-tauri/src/streaming/codecs/realtime_codec.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/codecs/yuv420_encoder.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/enhanced/ultra_low_latency.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/handlers/integrated_handler.rs**: Updated monitor enumeration to use native capture
- **src-tauri/src/streaming/handlers/ultra_stream.rs**: Simplified fallback capture to RGBA format

### Configuration
- **src-tauri/tauri.conf.json**: Simplified Debian package dependencies

### Web Client Fixes
- **src-tauri/web-client/kvm-client.js**: Fixed black screen issue by setting default screen dimensions to 1920x1080 instead of 0x0
- **src-tauri/web-client/kvm-client.js**: Fixed mouse coordinate calculation to use dynamically created canvas as target element
- **src-tauri/web-client/kvm-client.js**: Added bounds checking and dimension validation for coordinate scaling
- **src-tauri/web-client/kvm-client.js**: Fixed RGBA frame data copying to prevent ArrayBuffer reuse issues
- **src-tauri/web-client/kvm-client.js**: Added connection health monitoring with automatic reconnection on stale connections
- **src-tauri/web-client/kvm-client.css**: Fixed cursor visibility from 'none' to 'crosshair' for remote control

### Input Handling Fixes
- **src-tauri/src/streaming/handlers/ultra_stream.rs**: Added input event parsing and handling for mouse/keyboard events
- **src-tauri/src/streaming/handlers/ultra_stream.rs**: Changed InputHandler to use Arc<parking_lot::Mutex> for thread-safe access
- **src-tauri/src/streaming/handlers/realtime_stream.rs**: Added input event parsing and handling for mouse/keyboard events
- **src-tauri/src/streaming/handlers/realtime_stream.rs**: Changed InputHandler to use Arc<parking_lot::Mutex> for thread-safe access

### H.264 Hardware-Accelerated Streaming
- **src-tauri/src/streaming/codecs/h264_encoder.rs**: New H.264 hardware encoder with auto-detection for NVENC, QuickSync, AMF, VAAPI, and VideoToolbox
- **src-tauri/src/streaming/codecs/mod.rs**: Added h264_encoder module export
- **src-tauri/src/streaming/handlers/low_latency_pipeline.rs**: New low-latency streaming pipeline with target latency under 20ms on LAN
- **src-tauri/src/streaming/handlers/mod.rs**: Added low_latency_pipeline module export
- **src-tauri/src/network/server/websocket.rs**: Updated to use new H.264 low-latency pipeline
- **src-tauri/web-client/h264-decoder.js**: New WebCodecs-based H.264 decoder for browser-side hardware acceleration
- **src-tauri/web-client/kvm-client.js**: Added H.264 frame handling and decoder integration
- **src-tauri/web-client/kvm-template.html**: Added h264-decoder.js script reference
- **docs/H264_STREAMING_IMPLEMENTATION.md**: Comprehensive technical documentation for H.264 streaming implementation
- **README.md**: Updated with H.264 streaming features and removed legacy VP8/WebM references

### H.264-Only Codec Standardization
- **src-tauri/src/streaming/codecs/realtime_codec.rs**: Changed CodecType enum from VP8 to H264, updated codec string matching
- **src-tauri/src/streaming/codecs/yuv420_encoder.rs**: Renamed error variants and config fields from WebM to H.264 naming
- **src-tauri/src/streaming/handlers/realtime_stream.rs**: Updated server_info codec to h264
- **src-tauri/src/streaming/handlers/integrated_handler.rs**: Changed all config presets to use H.264, renamed methods from webm_* to h264_*
- **src-tauri/src/streaming/enhanced/mod.rs**: Removed VP8 module references, updated documentation
- **src-tauri/src/streaming/enhanced/enhanced_audio.rs**: Renamed for_webm() to for_high_quality_streaming()
- **src-tauri/src/streaming/enhanced/ultra_low_latency.rs**: Updated comments to remove VP8 reference
- **src-tauri/src/network/server/websocket.rs**: Renamed WebMConfig to H264Config, updated handler function names
- **src-tauri/src/network/server/handlers.rs**: Changed default codec from vp8 to h264
- **src-tauri/src/app/state.rs**: Renamed vp8 option to hardware_accel
- **src-tauri/src/app/commands.rs**: Updated debug logging to use hardware_accel
- **src-tauri/src/README.md**: Updated directory structure documentation
- **src-tauri/Cargo.toml**: Updated keywords from vp8 to h264, commented out webm/matroska dependencies
- **src-tauri/tauri.conf.json**: Updated longDescription to reference H.264
- **src/composables/useServer.js**: Removed useVP8 setting, changed selectedCodec to h264
- **src/constants/presets.js**: Removed useVP8 flag from all presets
- **src/components/server/AdvancedSettings.vue**: Replaced VP8 codec selection with H.264 info badge
- **src-tauri/web-client/kvm-client.js**: Removed VP8/WebM decoder code, updated to H.264-only
- **src-tauri/web-client/kvm-template.html**: Updated codec dropdown to H.264 only
- **src-tauri/web-client/kvm-template-parts.js**: Updated codec initialization to h264
- **scripts/build.sh**: Updated build messages to reference H.264
- **scripts/build.bat**: Updated build messages to reference H.264

### Deleted Files
- **src-tauri/src/streaming/enhanced/enhanced_video_vp8.rs**: Removed obsolete VP8 encoder
- **src-tauri/src/streaming/enhanced/enhanced_video.rs**: Removed obsolete WebM-based encoder

### YUV Color Conversion Fix
- **src-tauri/web-client/kvm-client.js**: Fixed green screen issue by correcting YUV to RGB conversion from BT.601 limited range to full range
- **src-tauri/web-client/h264-decoder.js**: Fixed YUV to RGB conversion in both parseHighQualityYUV() and parseSliceDataLegacy() methods

### UDP Relay Server (New Component)
- **relay-server/Cargo.toml**: New Rust project configuration with tokio, socket2, serde, lz4_flex dependencies
- **relay-server/src/main.rs**: CLI entry point with argument parsing for port, bind address, compression, and timeout settings
- **relay-server/src/relay.rs**: Core UDP relay server implementation with packet routing and session management
- **relay-server/src/protocol.rs**: Binary protocol definitions with packet header, video/audio frame headers, and message types
- **relay-server/src/peer.rs**: Peer and room management with automatic timeout cleanup
- **relay-server/README.md**: Documentation for relay server usage and protocol specification

### Relay Server v2.0 (Actix Web + Tera Templates)
- **relay-server/src/device.rs**: Device registry for managing connected KVM devices with capabilities, stream state, and viewer tracking
- **relay-server/src/discovery.rs**: mDNS service discovery for automatic relay server detection on local network
- **relay-server/src/http_server.rs**: Actix Web HTTP server with Tera template rendering, REST API, and WebSocket routes
- **relay-server/src/ws_relay.rs**: WebSocket relay handler for video frame broadcasting and input event forwarding
- **relay-server/templates/base.html**: Base HTML template with SVG favicon
- **relay-server/templates/dashboard.html**: Device dashboard with real-time statistics and device cards
- **relay-server/templates/kvm_client.html**: KVM viewer page with H.264 decoder integration
- **relay-server/templates/error.html**: Error page template
- **relay-server/static/dashboard.css**: Dashboard styles with dark theme
- **relay-server/static/dashboard.js**: Auto-refresh dashboard JavaScript
- **relay-server/static/kvm-client.css**: KVM client styles
- **relay-server/static/kvm-client.js**: WebSocket KVM client with input handling
- **relay-server/static/h264-decoder.js**: H.264 decoder for relay client viewer
- **relay-server/Cargo.toml**: Added Actix Web 4, actix-ws, actix-files, tera, lazy_static, dashmap, chrono, uuid, mdns-sd, gethostname dependencies

### Tauri Relay Client Integration
- **src-tauri/src/network/relay_client.rs**: Relay client module for device registration, mDNS discovery, WebSocket streaming, and heartbeat
- **src-tauri/src/network/mod.rs**: Added relay_client module export
- **src-tauri/Cargo.toml**: Added tokio-tungstenite 0.24, tracing 0.1, reqwest 0.12, hostname 0.4 dependencies
- **src-tauri/src/README.md**: Updated with related components table

## Testing

- Verified screen capture works correctly on Windows
- Confirmed RGBA frame output is correctly formatted
- Tested streaming functionality with the web client
- Verified mouse cursor alignment between client and server
- Confirmed keyboard and mouse input events are processed correctly
- Tested connection recovery after stream freeze
- Verified H.264 hardware encoder detection on Windows (NVENC, QuickSync, AMF)
- Tested H.264 streaming with WebCodecs decoder in browser
- Confirmed low-latency pipeline achieves target latency on LAN
- Verified cross-platform H.264 support on Linux and macOS
- Fixed green screen video output by correcting YUV to RGB color space conversion
- Built and tested UDP relay server on Windows
- Verified relay server packet routing and session management
- Relay server v2.0 with Actix Web compiles and runs successfully
- Templates embedded at compile time work from any working directory
- Tauri app compiles with relay client dependencies