# Changelog

All notable changes to this project will be documented in this file.

## [4.2.0] - 2026-02-05

### Added
- Relay client auto-reconnection when relay server goes down with exponential backoff (2s to 60s)
- ReconnectConfig struct for configurable auto-reconnect settings (enabled, initial_delay, max_delay, max_attempts)
- RelayState::Reconnecting state for tracking reconnection progress
- set_relay_auto_reconnect Tauri command for toggling auto-reconnect feature
- Auto-reconnect toggle checkbox in RelayStatus Vue component
- Reconnection attempt counter display in UI during reconnection
- HTTPS/TLS support for relay server with automatic self-signed certificate generation
- New tls.rs module for TLS configuration and certificate management
- Dual HTTP (port 8881) and HTTPS (port 8443) server support for relay server
- CLI arguments for HTTPS configuration (--https, --https-port, --tls-cert, --tls-key)
- Secure WebSocket (wss://) support for HTTPS connections
- WebCodecs API secure context detection in H.264 decoder
- Visual notification in KVM client when using software decoding on HTTP
- Tauri relay client auto-registration on application startup with mDNS discovery
- Tauri commands for relay server management (discover, connect, disconnect, status, auto-connect)
- RelayStatus Vue component for displaying relay connection status in the UI
- Relay server connection state tracking in ServerState with RelayStatus struct
- H.264 hardware-accelerated video encoding with auto-detection for NVENC (NVIDIA), QuickSync (Intel), AMF (AMD), VAAPI (Linux), and VideoToolbox (macOS)
- Standalone UDP relay server for low-latency P2P video streaming (relay-server/)
- Relay Server v2.0 with Actix Web 4 and Tera templating engine for centralized device management
- Web dashboard for viewing connected devices with real-time statistics
- WebSocket-based video relay for streaming through the relay server
- mDNS service discovery for automatic relay server detection on local network
- Device registry with capabilities tracking, stream state management, and viewer counting
- Tauri relay client module for registering devices with relay server
- Compile-time template embedding using include_str!() for portable relay server binary
- Binary protocol with packet fragmentation and LZ4 compression support
- Room-based session management with automatic peer discovery
- High-quality YUV420 subsampled encoding with bilinear interpolation upscaling
- Low-latency streaming pipeline with target latency under 20ms on LAN
- WebCodecs-based H.264 decoder for browser-side hardware acceleration
- fMP4 container format for efficient H.264 frame transport
- New h264_encoder.rs module with cross-platform hardware encoder support
- New low_latency_pipeline.rs module for optimized streaming
- New h264-decoder.js for browser WebCodecs integration
- Comprehensive H.264 streaming documentation (docs/H264_STREAMING_IMPLEMENTATION.md)

### Changed
- Enhanced relay client heartbeat to detect connection loss after 3 consecutive failures
- Extended RelayStatus struct with auto_reconnect and reconnect_attempts fields
- Updated get_relay_status command to return real-time status from relay client
- Updated RelayStatus.vue with reconnecting state UI and visual feedback
- Updated relay server to support both HTTP and HTTPS with single HttpServer instance
- Updated kvm_client.html template to dynamically select WebSocket protocol (ws:// or wss://) based on page protocol
- Updated h264-decoder.js with secure context detection for WebCodecs API availability
- Updated kvm-client.js to show notification when using software decoding on non-HTTPS connections
- Updated relay server README.md with HTTPS documentation and usage instructions
- Updated ServerStatus.vue with Direct Access URL label and Local Network badge
- Replaced emoji icons with text labels in Vue components for professional appearance
- Updated App.vue to integrate RelayStatus component in Server Status tab
- Updated useServer.js composable with relay state management and functions
- Updated WebSocket handler to use new H.264 low-latency pipeline
- Updated kvm-client.js with H.264 frame handling support
- Updated README.md with H.264 streaming features
- Removed legacy VP8/WebM codec references in favor of H.264-only streaming
- Changed CodecType enum from VP8 to H264 in realtime_codec.rs
- Renamed YUV420 encoder error variants and config fields from WebM to H.264 naming
- Updated all streaming handler config presets to use H.264 container
- Renamed webm_with_audio() and webm_video_only() to h264_with_audio() and h264_video_only()
- Updated VideoStreamInfo codec/format to H264/h264 in integrated_handler.rs
- Renamed WebMConfig to H264Config in websocket.rs
- Changed default codec from vp8 to h264 in HTTP handlers
- Renamed vp8 server option to hardware_accel in state.rs
- Updated useServer.js to use hardware acceleration instead of VP8 flag
- Removed useVP8 flag from all presets in presets.js
- Replaced VP8 codec selection with H.264 info badge in AdvancedSettings.vue
- Updated build scripts to reference H.264 instead of VP8/WebM
- Updated Cargo.toml keywords from vp8 to h264
- Updated tauri.conf.json description to reference H.264
- Relay server rewritten from UDP-based to HTTP/WebSocket-based architecture using Actix Web
- Templates embedded at compile time for working directory independence
- Added tokio-tungstenite, tracing, reqwest, hostname dependencies to Tauri app for relay client

### Fixed
- Fixed relay client not recovering when relay server restarts or becomes unavailable
- Fixed WebCodecs API unavailability in relay server KVM viewer by implementing HTTPS support
- Fixed mixed content blocking when accessing KVM viewer via HTTPS (WebSocket now uses wss://)
- Fixed Tauri app devices not appearing in relay server dashboard after launch
- Fixed Tera template rendering error for boolean values in kvm_client.html (changed | lower filter to conditional block)
- Fixed server_port rendering as string instead of number in relay KVM client template
- Fixed wsUrl format in relay KVM client template to use correct server hostname
- Fixed green screen issue in H.264 decoder by correcting YUV to RGB color conversion formula
- Changed YUV to RGB conversion from BT.601 limited range to BT.601 full range in kvm-client.js and h264-decoder.js
- Fixed bilinear sampling in high-quality YUV decoder for smoother video output
- Fixed relay server KVM viewer "Device is not streaming" error by adding stream_start message to Tauri relay client WebSocket connection
- Fixed devices not appearing in registry when connecting via WebSocket without prior HTTP registration by adding ensure_device() auto-registration
- Fixed WebSocket connection handling to properly update device heartbeat and state on connect
- Improved relay KVM client error handling to show informational messages instead of errors for non-critical issues
- Changed WebCodecs unavailability message from warning to informational in h264-decoder.js

### Removed
- Deleted src-tauri/web-client folder (unused duplicate files)
- Deleted enhanced_video_vp8.rs (obsolete VP8 encoder)
- Deleted enhanced_video.rs (obsolete WebM-based encoder)
- Removed VP8/WebM decoder code from kvm-client.js
- Removed webm and matroska dependencies from Cargo.toml (commented out)

## [4.1.0] - 2026-02-04

### Fixed
- Fixed Windows screen capture compatibility issues caused by breaking API changes in windows-capture 1.5.0
- Resolved build failures due to scap/zed-scap dependency conflicts
- Fixed KVM web client black screen issue by correcting default screen dimensions
- Fixed mouse cursor alignment between client and actual screen coordinates
- Fixed mouse and keyboard input events not being processed by the server
- Fixed RGBA frame data copying to prevent ArrayBuffer reuse issues
- Fixed cursor visibility in CSS for remote control interface

### Changed
- Replaced scap screen capture library with native Windows GDI implementation for maximum stability
- Updated Cargo.toml to use zed-scap 0.0.8-zed with pinned windows-capture 1.4.4
- Refactored capture.rs to use native capture backend
- Updated all streaming codecs and handlers to use the new native capture module
- Simplified fallback capture to use RGBA format for better client compatibility
- Reduced Debian package dependencies to only require ffmpeg

### Added
- New native_capture.rs module with direct Windows GDI API implementation
- Native monitor enumeration support
- Improved error handling for screen capture operations
- Cross-platform native screen capture support for Linux X11 and macOS
- Linux X11 capture using x11rb library with RandR extension for multi-monitor support
- macOS capture using Core Graphics (CGDisplayCreateImage) API
- Platform-specific dependencies: x11rb for Linux, core-graphics/core-foundation for macOS
- Connection health monitoring with automatic stream freeze detection
- Automatic reconnection when connection becomes stale
- Input event parsing for mouse and keyboard events in streaming handlers
- Thread-safe input handler using Arc<parking_lot::Mutex>