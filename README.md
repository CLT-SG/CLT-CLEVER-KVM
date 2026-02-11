# Clever KVM - Display Server for Video Walls

A high-performance remote desktop system built with Tauri, featuring VP9 video encoding via libvpx and ultra-low latency streaming over WebSocket.

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

**VP9 Video Streaming (RDEngine)**
- **VP8/VP9 encoding** via libvpx (cross-platform, no hardware-specific dependencies)
- Frame deduplication — skips encoding when screen is unchanged
- Adaptive QoS — FPS and bitrate adjust based on network RTT
- WebCodecs-based browser decoding with hardware acceleration
- Configurable quality presets: Gaming (60fps/12Mbps), Desktop (30fps/6Mbps), Low Bandwidth (15fps/2Mbps)

**Native Screen Capture**
- **Cross-Platform Native APIs**: Direct platform API integration for maximum stability
- **Windows**: GDI capture (GetDC, BitBlt, GetDIBits) for universal Windows compatibility
- **Linux X11**: Native X11 library with RandR extension for multi-monitor support
- **macOS**: Core Graphics (CGDisplayCreateImage) for Quartz display capture
- **Multi-Format Frame Support**: Handles BGRA, RGB, and YUV formats with automatic conversion
- **Monitor Detection**: Automatic display enumeration with fallback for headless systems

**Audio Streaming**
- Opus codec (48kHz stereo) via cpal + opus crate
- 10ms frame size for minimal latency
- Streamed alongside video over the same WebSocket connection

**Performance**
- Dedicated OS threads for capture/encode (not async — avoids jitter)
- Pre-allocated YUV buffer reuse across frames
- `parking_lot` high-performance locks
- `mimalloc` allocator for optimized memory allocation
- Sub-40ms latency on local network

**Desktop Control**
- Multi-monitor support
- Full keyboard/mouse/scroll control
- Real-time cursor capture with desktop portal integration
- Screen scaling options

**Matrix Dark Theme UI**
- **Modern Dark Interface**: Matrix-inspired dark theme with green accent colors (#00ff41)
- **Interactive Server Controls**: Clickable status ring to toggle server start/stop
- **SVG Icon System**: Professional Feather/Material Design style icons throughout
- **Responsive Layout**: Optimized content density with 660px window height
- **Tabbed Navigation**: Status, Configuration, Options, Logs, and Settings tabs with icons
- See [THEME.md](docs/THEME.md) for detailed theme documentation

## System Requirements

### Minimum Requirements
- **CPU**: Dual-core 2.0 GHz (Quad-core recommended)
- **RAM**: 4 GB (8 GB recommended for high-quality streaming)  
- **Network**: 10 Mbps upload bandwidth

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
2. Select your preferred quality preset (Gaming / Desktop / Low Bandwidth)
3. Click "Start Server" to begin the KVM service  
4. Use the displayed URL to access your computer from any browser

### Quality Presets

| Preset | Bitrate | FPS | Use Case |
|--------|---------|-----|----------|
| Gaming | 12 Mbps | 60 | Low-latency gaming/video |
| Desktop | 6 Mbps | 30 | General remote work |
| Low Bandwidth | 2 Mbps | 15 | Slow networks/mobile |

## Technology Stack

### Video & Audio (RDEngine)
- **VP9 Video**: Software encoding via libvpx (cross-platform, no HW-specific deps)
- **Opus Audio**: Low-latency stereo audio via cpal + opus crate
- **Binary Protocol**: Minimal-overhead length-prefixed frames over WebSocket
- **Adaptive QoS**: RTT-based FPS/bitrate adjustment
- **WebCodecs Decoding**: Browser-side hardware-accelerated VP9 decoding

### Backend (Rust/Tauri)
- Dedicated OS threads for capture/encode (not async — avoids jitter)
- Frame deduplication (byte-compare before encoding)
- Pre-allocated YUV buffer reuse
- `parking_lot` high-performance locks
- `mimalloc` allocator for optimized memory

### Frontend (JavaScript/Vue.js)
- WebCodecs API for hardware-accelerated VP9 decoding
- Binary frame parser with minimal header overhead
- JSON-based control/input protocol
- Vue 3.5 + Vite 6 management UI
- Matrix dark theme with CSS custom properties

### Communication Protocol
- **Video/Audio**: Binary WebSocket frames (length-prefixed)
- **Control/Input**: JSON text WebSocket messages

See [RDENGINE_STREAMING_IMPLEMENTATION.md](docs/RDENGINE_STREAMING_IMPLEMENTATION.md) for detailed architecture documentation.

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
- **Latency**: 20–40ms end-to-end on local network (VP9 pipeline)
- **Quality**: Near-lossless at 4-6 Mbps for desktop content
- **CPU Usage**: 5–15% with VP9 software encoding
- **Frame Rates**: Stable 60 FPS at 1920x1080
- **Audio Latency**: <20ms with Opus low-delay mode (10ms frames)
- **Frame Dedup**: 60–95% frames skipped on static desktops
- **Browser Decoding**: Hardware-accelerated via WebCodecs API

For detailed build instructions, troubleshooting, and platform-specific setup, see [BUILD.md](docs/BUILD.md).

## Recent Enhancements

### Matrix Dark Theme UI (v5.0.10)
- **Global Dark Theme**: CSS custom properties for colors, spacing, shadows, and effects
- **Interactive Status Ring**: Clickable control to toggle server start/stop with visual feedback
- **SVG Icon System**: Replaced emoji icons with professional inline SVG icons
- **Improved Content Density**: 10% height increase, 10% font reduction for better UX
- **Reorganized Settings**: UpdateChecker moved to Settings tab, removed unused Appearance section
- See [THEME.md](docs/THEME.md) for detailed theme documentation

### RDEngine VP9 Streaming (v4.1.0)
- **VP8/VP9 Encoding**: Cross-platform software encoding via libvpx
- **RustDesk-Inspired Architecture**: Dedicated threads, frame dedup, adaptive QoS
- **Binary Protocol**: Minimal-overhead framing replacing fMP4 container
- **Opus Audio**: Low-latency stereo streaming via cpal + opus
- **WebCodecs Decoding**: Hardware-accelerated VP9 decoding in browsers
- See [RDENGINE_STREAMING_IMPLEMENTATION.md](docs/RDENGINE_STREAMING_IMPLEMENTATION.md) for detailed documentation

### Native Cross-Platform Screen Capture (v4.1.0)
- **Platform-Native APIs**: Direct platform API implementations for maximum stability
- **Windows**: GDI capture (GetDC, BitBlt, GetDIBits)
- **Linux**: X11 library with RandR extension for multi-monitor support
- **macOS**: Core Graphics (CGDisplayCreateImage)
- **Zero External Dependencies**: Pure platform API implementation

### Key Rust Dependencies
```toml
# VP9 Encoding (RDEngine)
libvpx-sys = "1.4"
cpal = "0.15"
opus = "0.3"
crossbeam-channel = "0.5"

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
mimalloc = "0.1"       # Microsoft's optimized allocator
```

## Releases

See [CHANGELOG.md](docs/CHANGELOG.md) for release history and updates.

## License

[MIT](docs/LICENSE)
