# CLT-CLEVER-KVM Architecture

## Overview

CLT-CLEVER-KVM is a high-performance, multi-monitor VNC server application built with Tauri (Rust + Vue.js). It provides low-latency screen sharing with integrated WebSocket-based audio streaming, designed specifically for video wall and digital signage applications.

## System Architecture

### High-Level Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     CLT-CLEVER-KVM Application               │
│                        (Tauri/Rust)                          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │            Frontend (Vue.js)                        │   │
│  │  - ServerControls.vue                              │   │
│  │  - ServerStatus.vue                                │   │
│  │  - Configuration UI                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                   │
│                          │ Tauri IPC                        │
│                          ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │            Backend (Rust)                           │   │
│  │                                                      │   │
│  │  ┌──────────────────────────────────────────────┐  │   │
│  │  │  VNC Server Manager                          │  │   │
│  │  │  - Multi-monitor coordination               │  │   │
│  │  │  - Port allocation (5900+n)                 │  │   │
│  │  │  - Server lifecycle management              │  │   │
│  │  └──────────────────────────────────────────────┘  │   │
│  │                                                      │   │
│  │  ┌──────────────────────────────────────────────┐  │   │
│  │  │  VNC Server (per monitor)                    │  │   │
│  │  │  - RFB 3.8 protocol                         │  │   │
│  │  │  - Screen capture (xcap)                    │  │   │
│  │  │  - Input handling                           │  │   │
│  │  │  - Client management                        │  │   │
│  │  └──────────────────────────────────────────────┘  │   │
│  │                                                      │   │
│  │  ┌──────────────────────────────────────────────┐  │   │
│  │  │  WebSocket Audio Streamer (per monitor)      │  │   │
│  │  │  - Audio capture (cpal)                     │  │   │
│  │  │  - Opus encoding (low-latency)              │  │   │
│  │  │  - WebSocket server                         │  │   │
│  │  │  - Client broadcasting                      │  │   │
│  │  └──────────────────────────────────────────────┘  │   │
│  │                                                      │   │
│  └─────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
                          │
                          │ TCP/WebSocket
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                    Network Clients                            │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────┐               │
│  │  VNC Clients     │  │  WebSocket       │               │
│  │  - TigerVNC      │  │  Audio Clients   │               │
│  │  - RealVNC       │  │  - Browser       │               │
│  │  - NoVNC         │  │  - Custom apps   │               │
│  └──────────────────┘  └──────────────────┘               │
└──────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. VNC Server Manager (`src-tauri/src/vnc/manager.rs`)

**Responsibilities:**
- Manage multiple VNC server instances (one per monitor)
- Manage shared audio streamer for all monitors
- Handle automatic port allocation
- Provide unified status reporting

**Key Features:**
- Port allocation: VNC starts at 5900+n, Audio shared on 6900
- Single shared audio streamer for all monitors (since audio is system-wide)
- Monitor detection and configuration
- Lifecycle management (start/stop/status)

### 2. VNC Server (`src-tauri/src/vnc/server.rs`)

**Responsibilities:**
- Implement RFB 3.8 protocol
- Handle VNC client connections
- Capture screen content
- Process keyboard and mouse input

**Key Features:**
- Multi-client support (up to 10 clients per server)
- Raw encoding (future: Tight, ZRLE, H.264)
- Real-time screen capture using xcap
- Low-latency TCP streaming

**Protocol Flow:**
```
Client                          Server
  │                              │
  ├──── RFB 003.008 ─────────────>
  <──── RFB 003.008 ──────────────┤
  │                              │
  ├──── Security Type ────────────>
  <──── Security Result ──────────┤
  │                              │
  ├──── ClientInit ───────────────>
  <──── ServerInit ───────────────┤
  │                              │
  ├──── SetEncodings ─────────────>
  │                              │
  ├──── FramebufferUpdateRequest ─>
  <──── FramebufferUpdate ────────┤
  │                              │
  ├──── KeyEvent / PointerEvent ──>
  │                              │
```

### 3. WebSocket Audio Streamer (`src-tauri/src/vnc/audio_websocket.rs`)

**Responsibilities:**
- Capture system audio
- Encode audio with Opus codec
- Stream audio over WebSocket
- Manage WebSocket client connections

**Key Features:**
- Low-latency Opus encoding (48kHz stereo)
- Binary WebSocket protocol
- Automatic client management
- Single-pass encoding (no transcoding)

**Audio Pipeline:**
```
System Audio
    │
    ▼
cpal (Audio Capture)
    │
    ▼
Opus Encoder (Low Delay)
    │
    ▼
WebSocket Server
    │
    ├──> Client 1
    ├──> Client 2
    └──> Client N
```

### 4. Screen Capture (`src-tauri/src/core/`)

**Responsibilities:**
- Multi-monitor screen capture
- Exact monitor positioning
- RGB/RGBA format conversion

**Key Features:**
- Uses xcap for cross-platform screen capture
- Exact monitor dimensions and positioning
- Efficient memory management

### 5. Input Handling (`src-tauri/src/vnc/input.rs`)

**Responsibilities:**
- Translate VNC key codes to OS-level input
- Handle keyboard events
- Handle mouse events (movement, clicks, scroll)

**Key Features:**
- X11 keysym to OS key translation
- Absolute mouse positioning
- Button state management

## Data Flow

### Video Streaming Flow

```
OS Display Buffer
    │
    ▼
xcap Screen Capture
    │
    ▼
RGBA Pixel Data
    │
    ▼
VNC Server (Raw Encoding)
    │
    ▼
TCP Socket (RFB Protocol)
    │
    ▼
VNC Client
```

### Audio Streaming Flow

```
System Audio Output
    │
    ▼
cpal Audio Device
    │
    ▼
Float32 Audio Samples
    │
    ▼
Opus Encoder (48kHz Stereo)
    │
    ▼
Compressed Audio Frames
    │
    ▼
WebSocket Binary Messages
    │
    ▼
WebSocket Client(s)
```

### Input Flow

```
VNC Client Input
    │
    ▼
RFB Protocol Messages
    │
    ▼
VNC Server Parser
    │
    ├──> KeyEvent ──────> enigo Input Simulation
    └──> PointerEvent ──> enigo Mouse Simulation
                            │
                            ▼
                       OS Input System
```

## Multi-Monitor Support

### Port Allocation Strategy

Each monitor gets its own VNC server but all share a single audio stream:

| Monitor | VNC Port | Audio Port | URL Pattern |
|---------|----------|------------|-------------|
| 0       | 5900     | 6900       | vnc://host:5900, ws://host:6900/audio |
| 1       | 5901     | 6900       | vnc://host:5901, ws://host:6900/audio |
| 2       | 5902     | 6900       | vnc://host:5902, ws://host:6900/audio |
| N       | 5900+N   | 6900       | vnc://host:5900+N, ws://host:6900/audio |

**Note:** All monitors share port 6900 for audio since system audio is the same across all displays.

### Monitor Configuration

- **Position**: Exact (x, y) coordinates from OS display settings
- **Size**: Native monitor resolution (width, height)
- **Primary**: Automatically detected primary monitor
- **Independent**: Each monitor runs completely independently

## Performance Characteristics

### Latency Metrics

| Component | Latency | Previous (RTSP) |
|-----------|---------|-----------------|
| Video (VNC) | 10-50ms | 200-500ms |
| Audio (WebSocket) | 5-30ms | 300-800ms |
| Input | <5ms | <5ms |
| Total End-to-End | 15-85ms | 500-1000ms |

### CPU Usage

- **WebSocket Architecture**: 40-60% reduction vs RTSP
- **Single-pass Encoding**: No transcoding overhead
- **Per-monitor Overhead**: ~5-10% CPU per active server

### Memory Usage

- **Per VNC Server**: ~50-100 MB
- **Per Audio Streamer**: ~10-20 MB
- **Base Application**: ~100-150 MB

## Technology Stack

### Backend (Rust)

- **Framework**: Tauri 1.5
- **VNC Protocol**: Custom RFB 3.8 implementation
- **Screen Capture**: xcap 0.0.10
- **Audio Capture**: cpal 0.15
- **Audio Encoding**: opus 0.3
- **WebSocket**: tokio-tungstenite 0.21
- **Async Runtime**: tokio 1.32
- **Input Simulation**: enigo 0.1

### Frontend (Vue.js)

- **Framework**: Vue 3
- **Build Tool**: Vite
- **Tauri API**: @tauri-apps/api

### Performance Libraries

- **Memory Allocator**: mimalloc (Microsoft's high-performance allocator)
- **Locks**: parking_lot (faster than std::sync::Mutex)
- **Parallelism**: rayon (data parallelism)

## Security Considerations

### Current Security

- ⚠️ **No Authentication**: Currently uses "None" security type
- ⚠️ **No Encryption**: Plain TCP/WebSocket (no TLS)
- ✅ **Local Network**: Designed for trusted local networks
- ✅ **Firewall**: Configurable firewall rules

### Recommended Deployment

1. **Network Isolation**: Deploy on isolated VLAN
2. **Firewall Rules**: Restrict port access to known IPs
3. **VPN/SSH Tunneling**: For remote access
4. **Physical Security**: Secure physical access to machines

### Future Security

- [ ] VNC Authentication (password-based)
- [ ] TLS/SSL for VNC and WebSocket
- [ ] Certificate-based authentication
- [ ] Access control lists

## Configuration

### Environment Variables

```bash
# Logging level
RUST_LOG=info

# Enable high-performance allocator
CARGO_FEATURES=mimalloc
```

### Tauri Configuration

Located in `src-tauri/tauri.conf.json`:

```json
{
  "tauri": {
    "allowlist": {
      "all": true
    }
  }
}
```

### VNC Server Configuration

```rust
VncServerConfig {
    port: 5900,              // VNC port
    monitor_id: 0,           // Monitor index
    enable_audio: true,      // Enable audio streaming
    audio_port: Some(6900),  // Shared audio WebSocket port (all monitors use 6900)
    max_clients: 10,         // Max concurrent clients
    password: None,          // No password (future)
    hostname: Some("host"),  // Hostname for URLs
}
```

## Building and Deployment

### Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri:dev
```

### Production Build

```bash
# Build for production
npm run tauri:build

# Output:
# - Linux: .deb, .AppImage
# - Windows: .msi, .exe
# - macOS: .dmg, .app
```

### System Requirements

**Development:**
- Node.js 16+
- Rust 1.70+
- Platform-specific dependencies (GTK, WebKit, etc.)

**Runtime:**
- Modern CPU (multi-core recommended)
- 4GB+ RAM
- Network: Gigabit Ethernet recommended

## Monitoring and Debugging

### Logging

```bash
# Enable detailed logging
RUST_LOG=debug npm run tauri:dev

# Log locations
# - stdout/stderr in development
# - System logs in production
```

### Performance Monitoring

- Client count per VNC server
- Audio streamer client count
- Memory usage per component
- Network bandwidth utilization

### Debug Commands

```bash
# Check VNC server status
curl http://localhost:PORT/status

# Test WebSocket audio
wscat -c ws://localhost:6900/audio
```

## Migration from RTSP

### Before (RTSP Architecture)

```
VNC → FFmpeg → RTSP → MediaMTX → Client
Audio → RTSP AAC → FFmpeg → MediaMTX → HLS → Client
```

**Issues:**
- Multiple encoding passes
- High latency (200-800ms)
- Complex infrastructure
- High CPU usage

### After (WebSocket Architecture)

```
VNC → Client (direct)
Audio → Opus → WebSocket → Client (direct)
```

**Benefits:**
- Single-pass encoding
- Low latency (5-50ms)
- Simple architecture
- 40-60% lower CPU usage

## Future Roadmap

### Short Term
- [ ] Complete cursor pseudo-encoding
- [ ] TLS/SSL support
- [ ] Password authentication

### Medium Term
- [ ] Efficient video encodings (Tight, ZRLE)
- [ ] H.264 hardware encoding
- [ ] Adaptive quality

### Long Term
- [ ] Clipboard synchronization
- [ ] File transfer
- [ ] Multi-user access control
- [ ] Recording and playback

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

## License

MIT License - See [LICENSE](../LICENSE) for details.
