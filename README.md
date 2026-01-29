# Clever KVM - Display Server for Video Walls

A high-performance display server built with Tauri for multi-monitor video wall systems, featuring native display streaming (RFB 3.8) protocol with separate audio streaming.

**Implementation Focus:** This application provides display server functionality for seamless integration with video wall systems using standard display streaming protocols.

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

🎥 **Multi-Monitor Display Support**
- Native display streaming (RFB 3.8) protocol implementation
- Individual display server per monitor with automatic port assignment
- Exact display positioning and sizing preserved from system settings
- Hardware-accelerated screen capture

🎵 **Separate Audio Streaming**
- WebSocket audio streaming on dedicated port (6900)
- Opus encoding for low-latency audio (5-30ms)
- Shared audio across all monitors

🚀 **Performance**
- Hardware acceleration for screen capture
- Multi-threaded processing
- Optimized for video wall deployments

🖥️ **Desktop Control**
- Multi-monitor support with individual display streams
- Full keyboard/mouse/scroll control per monitor
- Real-time cursor capture
- Exact monitor positioning and sizing

🎛️ **Display Server Mode**
- Native display streaming (RFB 3.8) server for video wall integration
- Multi-monitor support with automatic port assignment (5900, 5901, 5902, etc.)
- WebSocket-based audio streaming on port 6900 (Opus encoded)
- Exact display positioning and sizing
- Auto-registration with CLEVER service
- Multi-client support (up to 10 simultaneous connections per monitor)
- Compatible with all standard display clients
- Built-in WebSockify proxy for NoVNC browser clients

## Display Server Configuration

CLT-CLEVER-KVM provides a native display server for seamless integration with multi-monitor video wall systems.

### Features
- Standard RFB 3.8 protocol for display streaming
- Multi-monitor support with individual display servers per monitor
- Built-in WebSockify proxy for NoVNC browser clients
- WebSocket-based audio streaming with Opus encoding
- Exact monitor positioning and sizing preserved
- Auto-registration with clever-service
- Multi-client support
- Hardware-accelerated screen capture

### Port Assignment

| Service | Port Range | Description |
|---------|------------|-------------|
| **VNC** | 5900+ | Native VNC protocol (TigerVNC, RealVNC, etc.) |
| **WebSockify** | 6080+ | WebSocket proxy for NoVNC browser clients |
| **Audio** | 6900 | WebSocket audio stream (Opus, shared) |

**Per-Monitor Ports:**
- **Monitor 0 (Primary)**: VNC port 5900, WebSockify port 6080
- **Monitor 1**: VNC port 5901, WebSockify port 6081
- **Monitor 2**: VNC port 5902, WebSockify port 6082
- **...and so on**
- **Audio Stream**: Port 6900 (WebSocket, shared across all monitors)

### Usage
```bash
# Start the application
npm run tauri dev

# Start display servers for all monitors with audio
# This will automatically start:
# - VNC servers on ports 5900, 5901, etc. for each monitor
# - WebSockify proxies on ports 6080, 6081, etc. for NoVNC
# - Audio WebSocket on port 6900

# Or start individual monitor display servers as needed
```

### Connecting with Display Clients

**Note**: As of version 3.0, URLs use hostname instead of IP addresses for improved stability.

**Desktop VNC Clients (TigerVNC, RealVNC, etc.):**
```bash
# Connect to VNC port (5900+)
vncviewer <hostname>:5900   # Monitor 0
vncviewer <hostname>:5901   # Monitor 1
```

**NoVNC (Browser-based):**
```
# Connect to WebSockify port (6080+) - NOT the VNC port!
ws://<hostname>:6080/   # Monitor 0 (via WebSockify)
ws://<hostname>:6081/   # Monitor 1 (via WebSockify)
```

**Audio (WebSocket):**
```
ws://<hostname>:6900/audio   # Shared audio stream (Opus encoded)
```

For more details, see [Display Integration Guide](docs/VNC_INTEGRATION.md).

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
    libxdo-dev libxrandr-dev libxcb-randr0-dev build-essential \
    libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev 
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
2. Configure your display and audio settings
3. Click "Start Server" to begin streaming displays to your video wall system
4. Connect using compatible display clients on the configured ports

## Technology Stack

### Backend (Rust/Tauri)
- **Display Capture**: Hardware-accelerated screen capture with multi-monitor support
- **Display Streaming**: Native RFB 3.8 protocol implementation
- **WebSockify Proxy**: Built-in WebSocket-to-VNC proxy for NoVNC browser clients
- **Audio Streaming**: WebSocket server with Opus audio encoding (low-latency)
- **Performance**: Multi-threaded processing with hardware acceleration

### Frontend (JavaScript/Vue.js)
- Modern UI for server configuration and monitoring
- Real-time status updates
- Multi-monitor management interface

### Communication Protocols
- **VNC Display**: RFB 3.8 protocol (port 5900+) - for desktop clients
- **WebSockify**: WebSocket-to-VNC proxy (port 6080+) - for NoVNC browser clients
- **Audio**: WebSocket with Opus encoding (port 6900) - for audio streaming
- **Control**: Native Tauri commands for server management

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

See [UPDATER.md](docs/UPDATER.md) for more details.

This will automatically:
- Build for Windows, macOS, and Linux
- Create installers (.msi, .exe, .dmg, .deb, .AppImage)
- Upload to GitHub Releases

### Performance Benchmarks

On modern hardware, Clever KVM achieves:
- **Latency**: 25-50ms end-to-end (local network)  
- **Quality**: High-quality display streaming optimized for video walls
- **Frame Rates**: Stable 60 FPS at 1920x1080 on mid-range systems
- **Audio Latency**: <20ms with optimized audio encoding
- **Efficiency**: Hardware-accelerated encoding for minimal CPU usage

For detailed build instructions, troubleshooting, and platform-specific optimizations, see [BUILD.md](docs/BUILD.md).

## Releases

See [CHANGELOG.md](docs/CHANGELOG.md) for release history and updates.

## License

[MIT](docs/LICENSE)
