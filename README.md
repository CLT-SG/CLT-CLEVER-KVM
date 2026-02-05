# Clever KVM

A high-performance remote desktop system built with Tauri, featuring native H.264 video encoding with hardware acceleration and ultra-low latency streaming.

## Quick Start

### Setup and Build

1. Clone the repository:
   ```bash
   git clone https://github.com/CLTSG/CLT-CLEVER-KVM.git
   cd clever-kvm
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Build the application:
   ```bash
   npm run tauri build
   ```

## Development

Start the development server with live reloading:

```bash
npm run tauri dev
```

## Features

🎥 **Advanced Video Streaming**
- **H.264 Encoding** with hardware acceleration (NVENC, QuickSync, AMF, VAAPI, VideoToolbox)
- YUV420 color space optimization (50% better compression than RGB)
- Ultra-low latency mode (<20ms end-to-end on LAN)
- WebCodecs-based browser decoding with hardware acceleration
- Adaptive quality (1-10 Mbps) and frame rates (15-60 FPS)

🖥️ **Native Screen Capture**
- **Cross-Platform Native APIs**: Direct platform API integration for maximum stability
- **Windows**: GDI capture (GetDC, BitBlt, GetDIBits) for universal Windows compatibility
- **Linux X11**: Native X11 library with RandR extension for multi-monitor support
- **macOS**: Core Graphics (CGDisplayCreateImage) for Quartz display capture
- **Multi-Format Frame Support**: Handles BGRA, RGB, and YUV formats with automatic conversion
- **Monitor Detection**: Automatic display enumeration with fallback for headless systems

🎵 **Professional Audio**
- Native Opus codec for high-quality audio streaming
- Multiple quality modes: High (320kbps), Balanced (256kbps), Low Latency (96kbps)
- Perfect audio/video synchronization

🚀 **Performance**
- Hardware acceleration (Intel Quick Sync, NVENC, AMD VCE/AMF, Apple VideoToolbox)
- Cross-platform H.264 hardware encoder detection
- Multi-threaded encoding with SIMD optimizations
- Zero external dependencies (no FFmpeg required)
- Sub-20ms latency on local network with H.264 pipeline

🖥️ **Desktop Control**
- Multi-monitor support
- Full keyboard/mouse/scroll control
- Real-time cursor capture with desktop portal integration
- Screen scaling options

## System Requirements

### Minimum Requirements
- **CPU**: Dual-core 2.0 GHz (Quad-core recommended)
- **RAM**: 4 GB (8 GB recommended for high-quality streaming)  
- **Network**: 10 Mbps upload bandwidth
- **GPU**: Hardware encoding support recommended

### Development Prerequisites
- [Node.js](https://nodejs.org/) v16+
- [Rust](https://www.rust-lang.org/) v1.67+
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

### Linux Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y build-essential curl wget file libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libxdo-dev libxcb-randr0-dev xdg-desktop-portal libpipewire-0.3-dev libopus-dev \
    libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev \
    patchelf libxrandr-dev
```

**Desktop Portal Requirements (Linux):**
```bash
# For screen capture functionality on Linux
sudo apt-get install -y xdg-desktop-portal xdg-desktop-portal-gtk

# For GNOME environments
sudo apt-get install -y xdg-desktop-portal-gnome

# For KDE environments  
sudo apt-get install -y xdg-desktop-portal-kde

# Restart desktop portal services after installation
systemctl --user restart xdg-desktop-portal xdg-desktop-portal-gtk
```

**For Ubuntu 24.04 (Latest):**
```bash
# Standard Ubuntu 24.04 packages (no additional repositories needed)
sudo apt update
sudo apt install -y build-essential curl wget file libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libxdo-dev libxcb-randr0-dev xdg-desktop-portal libpipewire-0.3-dev libopus-dev \
    libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev \
    patchelf libxrandr-dev
```

## Usage

1. Launch the Clever KVM application
2. Select your preferred quality preset from the dropdown
3. Click "Start Server" to begin the KVM service  
4. Use the displayed URL to access your computer from any browser

### Quality Optimization Tips
- **For Gaming**: Use `?latency=ultra&fps=60&hardware_accel=true`
- **For Presentations**: Use `?quality=high&audio=true&audio_quality=high`  
- **For Remote Work**: Use `?quality=balanced&fps=30&bitrate=3000`
- **For Slow Networks**: Use `?quality=low&fps=15&bitrate=1000`

## Technology Stack

### Video & Audio
- **H.264 Video**: Hardware-accelerated encoding with NVENC, QuickSync, AMF, VAAPI, VideoToolbox
- **Opus Audio**: CD-quality audio streaming
- **Hardware Acceleration**: Intel Quick Sync, NVENC, AMD VCE/AMF, Apple VideoToolbox
- **Ultra-Low Latency**: Sub-20ms total latency on LAN
- **WebCodecs Decoding**: Browser-side hardware-accelerated H.264 decoding

### Backend (Rust/Tauri)
- H.264 hardware-accelerated encoder with cross-platform support
- Multi-threaded encoding with SIMD optimizations
- Real-time bitrate adaptation (1-10 Mbps)
- WebSocket streaming with binary H.264/fMP4 frames

### Frontend (JavaScript/Vue.js)
- WebCodecs API for hardware-accelerated H.264 decoding
- Automatic codec detection and fallback
- Automatic quality adaptation
- Real-time performance monitoring

## Connection Options

### URL Parameters
- `quality=high|balanced|low` - Video quality preset
- `fps=30` - Target frame rate (15-60)
- `audio=true` - Enable audio streaming
- `latency=ultra|low|balanced` - Latency optimization mode
- `hardware_accel=true` - Force hardware acceleration

### Example URLs
```
# High-quality streaming with audio (H.264 default)
http://hostname:9921/kvm?quality=high&audio=true

# Ultra-low latency gaming (60fps)
http://hostname:9921/kvm?latency=ultra&fps=60

# Bandwidth-optimized
http://hostname:9921/kvm?quality=balanced&bitrate=1500
```

## Architecture & Technology Stack

### Backend (Rust/Tauri)
- **Screen Capture**:
  - Native platform APIs for cross-platform screen recording (no external dependencies)
  - Windows: GDI (GetDC, BitBlt, GetDIBits) for maximum compatibility
  - Linux: X11 library with RandR extension for multi-monitor support
  - macOS: Core Graphics (CGDisplayCreateImage) for Quartz display capture
  - Multi-format frame support: BGRA, RGB, YUV with automatic conversion
  - Automatic format conversion and cursor overlay capabilities
- **Video Encoding**: 
  - **H.264 Encoder** with cross-platform hardware acceleration
    - Windows: NVENC (NVIDIA), QuickSync (Intel), AMF (AMD)
    - Linux: NVENC, VAAPI (Intel/AMD)
    - macOS: VideoToolbox (automatic)
  - Hardware acceleration and SIMD optimizations via `rayon`
  - Real-time bitrate adaptation (1-10 Mbps) based on network conditions
- **Audio Encoding**:
  - Native Opus codec using `opus` crate for high-quality streaming
  - CD-quality audio (320 kbps) with Forward Error Correction (FEC)
  - Multiple quality profiles: High (320k), Balanced (256k), Low Latency (96k)
  - WebRTC peer connection for browser compatibility
- **Streaming Handlers**:
  - **Low-Latency H.264 Pipeline**: Sub-20ms streaming with fMP4 container format
  - **Realtime Handler**: Standard WebSocket streaming with graceful degradation
- **Performance Optimizations**: 
  - `parking_lot` high-performance locks (replaces std::sync::Mutex)
  - `rayon` parallel processing for multi-core SIMD operations
  - Optional `mimalloc` Microsoft allocator for 15-30% memory performance gains

### Frontend (JavaScript/Vue.js)
- **Video Decoding**: 
  - **WebCodecs H.264 Decoder** with hardware acceleration (Chrome 94+, Edge 94+, Safari 16.4+)
  - Canvas-based rendering for decoded frames
- **Quality Adaptation**:
  - Real-time performance monitoring
  - Automatic quality degradation during network issues
  - Frame queue management to prevent buffer overruns

### Communication Protocol
- **Primary**: WebSockets with binary H.264/fMP4 streaming
- **Control**: JSON command protocol for input events and configuration

### Relay Server (Rust/Actix Web)
The optional relay server enables multi-device management and remote access:

| Component | Technology | Purpose |
|-----------|------------|---------|
| **HTTP Server** | Actix Web 4 | High-performance async web framework |
| **WebSocket** | actix-ws | Real-time bidirectional video/input relay |
| **Templating** | Tera | Jinja2-style HTML templates with inheritance |
| **Static Files** | actix-files | CSS, JS, and asset serving |
| **mDNS** | mdns-sd | Zero-config service discovery on LAN |

**Features:**
- Web dashboard at `http://{hostname}.local:8881/` showing all connected devices
- Professional Tera templates (base.html, dashboard.html, kvm_client.html)
- Device registry with heartbeat monitoring
- WebSocket relay for video streams and input events
- REST API for device management
- mDNS advertisement for automatic discovery

See [relay-server/README.md](relay-server/README.md) for detailed documentation.

## Building and Distribution

### Quick Build

For a quick local build:

```bash
# Make the build script executable
chmod +x scripts/build.sh

# Build the application
./scripts/build.sh
```

### Automated Releases

To create a new release with automatic GitHub deployment:

```bash
# Prepare a new release version
./scripts/prepare-release.sh 1.0.0

# Commit and tag
git add .
git commit -m "Release v1.0.0"
git tag v1.0.0
git push origin v1.0.0
```

[UPDATER.md](docs/UPDATER.md)

This will automatically:
- Build for Windows, macOS, and Linux
- Create installers (.msi, .exe, .dmg, .deb, .AppImage)
- Upload to GitHub Releases

### Performance Benchmarks

On modern hardware, Clever KVM achieves:
- **Latency**: <20ms end-to-end on local network (H.264 pipeline)
- **Quality**: Near-lossless at 4-6 Mbps for desktop content
- **CPU Usage**: <10% with hardware-accelerated H.264 encoding
- **Frame Rates**: Stable 60 FPS at 1920x1080 on mid-range systems
- **Audio Latency**: <20ms with Opus low-delay mode
- **Memory Usage**: 50% less than FFmpeg-based solutions
- **Browser Decoding**: Hardware-accelerated via WebCodecs API

For detailed build instructions, troubleshooting, platform-specific optimizations, and codec configuration, see [BUILD.md](docs/BUILD.md).

## Recent Enhancements

### Relay Server v2.0 (February 2026)
- **Actix Web 4**: High-performance async HTTP server (replaces previous implementation)
- **Tera Templates**: Professional Jinja2-style HTML templating with inheritance
- **Web Dashboard**: Modern device management interface at `http://{hostname}.local:8881/`
- **WebSocket Relay**: Efficient video stream relay using actix-ws
- **mDNS Discovery**: Zero-config service discovery for automatic device detection
- **Static File Serving**: Optimized CSS/JS delivery via actix-files
- See [relay-server/README.md](relay-server/README.md) for detailed documentation

### H.264 Low-Latency Streaming Pipeline (v4.1.0)
- **H.264 Hardware Encoding**: Cross-platform hardware-accelerated H.264 encoding
  - Windows: NVENC (NVIDIA), QuickSync (Intel), AMF (AMD)
  - Linux: NVENC, VAAPI
  - macOS: VideoToolbox (automatic)
- **WebCodecs Browser Decoding**: Hardware-accelerated H.264 decoding in browsers
- **Sub-20ms Latency**: Optimized fMP4 container format for minimal buffering
- **Cross-Platform Support**: Full support for Windows, Linux (X11), and macOS
- See [H264_STREAMING_IMPLEMENTATION.md](docs/H264_STREAMING_IMPLEMENTATION.md) for detailed documentation

### Native Cross-Platform Screen Capture (v4.1.0)
- **Platform-Native APIs**: Direct platform API implementations for maximum stability
- **Windows**: GDI capture (GetDC, BitBlt, GetDIBits)
- **Linux**: X11 library with RandR extension for multi-monitor support
- **macOS**: Core Graphics (CGDisplayCreateImage)
- **Zero External Dependencies**: Pure platform API implementation

### Key Rust Dependencies
```toml
# Screen Capture (Platform-Specific)
[target.'cfg(windows)'.dependencies]
windows-capture = "=1.4.4"

[target.'cfg(target_os = "linux")'.dependencies]
x11rb = { version = "0.13", features = ["randr"] }

[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.24"
core-foundation = "0.10"

# Performance
parking_lot = "0.12"   # High-performance locks  
rayon = "1.8"          # Parallel SIMD processing
mimalloc = "0.1"       # Microsoft's optimized allocator
```

## Releases
[CHANGELOG.md](docs/CHANGELOG.md)

## License

[MIT](docs/LICENSE)
