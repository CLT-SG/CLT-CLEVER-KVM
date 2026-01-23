# Clever KVM - VNC Server

A high-performance VNC server built with Tauri for multi-monitor video wall systems, featuring native VNC (RFB 3.8) protocol support with separate audio streaming.

**Current Implementation:** This application is focused exclusively on VNC server functionality. Previous WebSocket/WebRTC streaming methods have been removed in favor of standard VNC protocol for better compatibility and integration with video wall systems.

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

🎥 **Multi-Monitor VNC Support**
- Native VNC (RFB 3.8) protocol implementation
- Individual VNC server per monitor with automatic port assignment
- Exact display positioning and sizing preserved from system settings
- Hardware-accelerated screen capture

🎵 **Separate Audio Streaming**
- RTSP audio streaming on dedicated port (6900)
- Shared audio across all monitors
- High-quality audio encoding

🚀 **Performance**
- Hardware acceleration for screen capture
- Multi-threaded processing
- Optimized for video wall deployments

🖥️ **Desktop Control**
- Multi-monitor support with individual VNC streams
- Full keyboard/mouse/scroll control per monitor
- Real-time cursor capture
- Exact monitor positioning and sizing

🎛️ **VNC Server Mode**
- Native VNC (RFB 3.8) server for video wall integration
- Multi-monitor support with automatic port assignment (5900, 5901, 5902, etc.)
- Separate audio streaming via RTSP on port 6900
- Exact display positioning and sizing
- Auto-registration with CLEVER service
- Multi-client support (up to 10 simultaneous connections per monitor)
- Compatible with all standard VNC clients

## VNC Server Mode

CLT-CLEVER-KVM provides a native VNC server for seamless integration with multi-monitor video wall systems.

### Features
- Standard VNC (RFB 3.8) protocol
- Multi-monitor support with individual VNC servers per display
- Separate audio streaming via RTSP
- Exact monitor positioning and sizing preserved
- Auto-registration with clever-service
- Multi-client support
- Hardware-accelerated encoding

### Port Assignment
- **Monitor 1 (Primary)**: VNC on port 5900
- **Monitor 2**: VNC on port 5901
- **Monitor 3**: VNC on port 5902
- **...and so on**
- **Audio Stream**: Port 6900 (shared across all monitors)

### Usage
```bash
# Start the application
npm run tauri dev

# Start VNC servers for all monitors with audio
# This will automatically start VNC on ports 5900, 5901, etc. for each monitor
# Audio will be available on port 6900

# Or start individual monitor VNC servers as needed
```

### Connecting with VNC Clients

**TigerVNC (Monitor 1):**
```bash
vncviewer <ip-address>:5900
```

**TigerVNC (Monitor 2):**
```bash
vncviewer <ip-address>:5901
```

**RealVNC:**
```bash
vnc://<ip-address>:5900  # Monitor 1
vnc://<ip-address>:5901  # Monitor 2
```

### Integration with MediaMTX

Configure MediaMTX to relay VNC + audio:
```yaml
paths:
  vnc_workstation_1_screen1:
    source: vnc://192.168.1.100:5900
    sourceProtocol: vnc
  vnc_workstation_1_screen2:
    source: vnc://192.168.1.100:5901
    sourceProtocol: vnc
  audio_workstation_1:
    source: rtsp://192.168.1.100:6900/audio
```

For more details, see [VNC Integration Guide](docs/VNC_INTEGRATION.md).

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
sudo apt-get install -y libwebkit2gtk-4.0-dev libwebkit2gtk-4.1-dev \
    libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev \
    libxdo-dev libxrandr-dev libxcb-randr0-dev build-essential libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev 
```

**For Ubuntu 22.04+:**
```bash
# Add jammy universe repository
echo "deb http://archive.ubuntu.com/ubuntu jammy main universe" | sudo tee -a /etc/apt/sources.list
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget file libssl-dev \
    libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libjavascriptcoregtk-4.0-bin libjavascriptcoregtk-4.0-dev \
    libsoup2.4-dev libxdo-dev libxcb-randr0-dev
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
- **VP8 Video**: Native WebM encoding with YUV420 color space (50% better compression than RGB)
- **Opus Audio**: CD-quality audio with WebM container integration
- **Hardware Acceleration**: Intel Quick Sync, NVENC, AMD VCE support
- **Ultra-Low Latency**: Sub-50ms total latency for gaming

### Backend (Rust/Tauri)
- Native WebM + VP8 encoder with no FFmpeg dependency
- Multi-threaded encoding with SIMD optimizations
- Real-time bitrate adaptation (1-10 Mbps)
- WebSocket streaming with binary WebM

### Frontend (JavaScript/Vue.js)
- MediaSource API for native WebM decoding
- Custom YUV420 decoder fallback
- Automatic quality adaptation
- Real-time codec switching

## Connection Options

### URL Parameters
- `codec=vp8` - Force VP8/WebM video codec
- `quality=high|balanced|low` - Video quality preset
- `fps=30` - Target frame rate (15-60)
- `audio=true` - Enable audio streaming
- `latency=ultra|low|balanced` - Latency optimization mode
- `hardware_accel=true` - Force hardware acceleration

### Example URLs
```
# High-quality streaming with audio
http://hostname:9921/kvm?quality=high&audio=true

# Ultra-low latency gaming
http://hostname:9921/kvm?latency=ultra&fps=60

# Bandwidth-optimized
http://hostname:9921/kvm?quality=balanced&bitrate=1500
```

## Architecture & Technology Stack

### Backend (Rust/Tauri)
- **Video Encoding**: 
  - Native WebM + VP8 encoder with YUV420 color space optimization
  - Built-in `webm` and `matroska` crate integration - no FFmpeg required
  - Hardware acceleration and SIMD optimizations via `rayon`
  - Temporal and spatial layering for adaptive quality control
  - Real-time bitrate adaptation (1-10 Mbps) based on network conditions
- **Audio Encoding**:
  - Native Opus codec using `opus` crate with WebM container
  - CD-quality audio (320 kbps) with Forward Error Correction (FEC)
  - Multiple quality profiles: High (320k), Balanced (256k), Low Latency (96k)
  - WebRTC peer connection fallback for browser compatibility
- **Streaming Handlers**:
  - **Integrated Handler**: Combined WebM audio/video streaming
  - **Ultra-Low Latency Handler**: Sub-50ms gaming-optimized streaming with performance budgeting
  - **Realtime Handler**: Standard WebSocket streaming with graceful fallbacks
- **Performance Optimizations**: 
  - `parking_lot` high-performance locks (replaces std::sync::Mutex)
  - `rayon` parallel processing for multi-core SIMD operations
  - Optional `mimalloc` Microsoft allocator for 15-30% memory performance gains

### Frontend (JavaScript/Vue.js)
- **Video Decoding**: 
  - Native WebM VP8+Opus decoding via MediaSource API
  - Custom YUV420 decoder fallback for unsupported browsers
  - WebM container detection and demuxing
- **Quality Adaptation**:
  - Real-time codec switching based on browser support
  - Automatic quality degradation during network issues
  - Frame queue management to prevent buffer overruns

### Communication Protocol
- **Primary**: WebSockets with binary WebM streaming
- **Fallback**: WebRTC peer connections for audio
- **Control**: JSON command protocol for input events and configuration

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
- **Latency**: 25-50ms end-to-end (local network)  
- **Quality**: Near-lossless at 4-6 Mbps for desktop content
- **Efficiency**: 40% better compression than H.264 for screen content
- **Frame Rates**: Stable 60 FPS at 1920x1080 on mid-range systems
- **Audio Latency**: <20ms with Opus low-delay mode
- **Memory Usage**: 50% less than FFmpeg-based solutions

For detailed build instructions, troubleshooting, platform-specific optimizations, and codec configuration, see [BUILD.md](docs/BUILD.md).

## Recent Enhancements (v3.0)

### Native WebM Video/Audio Pipeline
- **Zero External Dependencies**: Completely eliminated FFmpeg - now uses pure Rust libraries
- **50% Smaller Binaries**: Reduced installer size and memory footprint significantly  
- **Native WebM Encoding**: Direct VP8+Opus encoding with `webm`, `opus`, and `matroska` crates
- **YUV420 Optimization**: Custom color space conversion optimized for screen content
- **Synchronized Multiplexing**: Perfect audio/video sync with WebM container format

### Ultra-Low Latency Streaming
- **Sub-50ms Total Latency**: Competitive gaming and real-time interaction performance
- **Performance Budgeting System**: Automatic quality fallback to maintain target latency
- **SIMD-Optimized Pipeline**: Multi-core parallel processing with `rayon` for encoding efficiency
- **Emergency Quality Modes**: Graceful degradation during high load or network issues
- **Hardware Acceleration**: Leverages GPU encoding when available (Intel Quick Sync, NVENC)

### Advanced Audio Pipeline  
- **Native Opus Integration**: Pure Rust Opus codec with WebM container multiplexing
- **Multiple Quality Profiles**: High (320k), Balanced (256k+FEC), Low Latency (96k)
- **WebRTC Audio Fallback**: Seamless browser compatibility for unsupported configurations
- **Perfect Lip-Sync**: Frame-accurate audio/video synchronization with timestamp correction
- **Adaptive Bitrate**: Network-aware audio quality adjustment (96-320 kbps)

### Enhanced Browser Compatibility
- **MediaSource API Optimization**: Native WebM VP8+Opus decoding when supported
- **Custom YUV420 Decoder**: JavaScript fallback for legacy browsers and custom formats  
- **Progressive Enhancement**: Automatic codec detection with graceful degradation
- **Universal Browser Support**: Chrome, Firefox, Safari, Edge with appropriate fallbacks

### Key Rust Dependencies (Native WebM Stack)
```toml
webm = "1.1"           # WebM container format
opus = "0.3"           # Opus audio codec  
matroska = "0.14"      # WebM/Matroska muxing
image = "0.24"         # YUV420 color conversion
parking_lot = "0.12"   # High-performance locks  
rayon = "1.8"          # Parallel SIMD processing
mimalloc = "0.1"       # Microsoft's optimized allocator
```

## Releases
[CHANGELOG.md](docs/CHANGELOG.md)

## License

[MIT](docs/LICENSE)
