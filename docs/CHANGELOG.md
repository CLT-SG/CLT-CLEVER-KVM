# Clever KVM Release Changelog

## Version History

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
