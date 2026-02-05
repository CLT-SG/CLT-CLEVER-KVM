# H.264 Low-Latency Streaming Implementation

## Overview [Updated 5th February 2026]

This document describes the implementation of the H.264-based low-latency video streaming system for the CLEVER KVM application. This implementation replaces the previous RGBA-based streaming approach to fix black screen issues and dramatically improve streaming performance.

## Problem Statement

### Previous Issues
- **Black Screen**: When accessing the KVM from a browser at `http://<host-ip>:9921/kvm`, users experienced black screens
- **High Latency**: Raw RGBA streaming resulted in large data transfers (~8MB/frame at 1080p)
- **No Compression**: Uncompressed video data overwhelmed network bandwidth
- **Canvas Rendering Latency**: Using Canvas 2D context for rendering added unnecessary latency

### Root Causes
1. RGBA frame data was too large for real-time streaming over WebSocket
2. No video compression was applied before transmission
3. Browser-side rendering was inefficient
4. TCP-based WebSocket added additional latency compared to UDP

## Cross-Platform Support

This implementation supports **all three major platforms**:

| Platform | Screen Capture | Hardware Encoding | Status |
|----------|----------------|-------------------|--------|
| **Windows** | GDI (BitBlt) / DXGI | NVENC, QuickSync, AMF | ✅ Full Support |
| **Linux (X11)** | X11 (XGetImage) | NVENC, VAAPI | ✅ Full Support |
| **macOS** | Core Graphics (CGDisplay) | VideoToolbox | ✅ Full Support |

### Platform-Specific Details

#### Windows
- **Screen Capture**: Uses GDI (`GetDC`, `BitBlt`, `GetDIBits`) for maximum compatibility
- **Hardware Encoding**: Detects NVENC, Intel QuickSync, and AMD AMF via DLL presence
- **Dependencies**: `windows-capture` crate for monitor enumeration

#### Linux (Ubuntu/X11)
- **Screen Capture**: Uses X11 library (`XGetImage`) via `x11rb` crate
- **Hardware Encoding**: Detects NVENC via `libnvidia-encode.so`, VAAPI via `/dev/dri/renderD128`
- **Multi-Monitor**: Full RandR support for monitor enumeration
- **Dependencies**: `x11rb` crate with `randr` feature

#### macOS
- **Screen Capture**: Uses Core Graphics (`CGDisplayCreateImage`) for Quartz display
- **Hardware Encoding**: Uses VideoToolbox (automatically available)
- **Dependencies**: `core-graphics` and `core-foundation` crates

## Solution Architecture

### High-Level Flow (Direct Connection)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SERVER (Rust/Tauri)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────────┐  │
│  │  Native Screen   │───▶│   H.264 Encoder  │───▶│  fMP4 Container     │  │
│  │  Capture         │    │  (HW/SW)         │    │  Packaging          │  │
│  │  (Platform API)  │    │                  │    │                     │  │
│  └──────────────────┘    └──────────────────┘    └──────────────────────┘  │
│           │                       │                        │               │
│           ▼                       ▼                        ▼               │
│      BGRA Frame             YUV420 Data              H.264 NAL Units      │
│      (Native)               (Converted)              (Compressed)          │
│                                                            │               │
└────────────────────────────────────────────────────────────┼───────────────┘
                                                             │
                                                    WebSocket (Binary)
                                                             │
┌────────────────────────────────────────────────────────────┼───────────────┐
│                              CLIENT (Browser)              │               │
├────────────────────────────────────────────────────────────┼───────────────┤
│                                                            ▼               │
│  ┌──────────────────────┐    ┌──────────────────┐    ┌─────────────────┐  │
│  │  WebCodecs           │◀───│  H.264 Decoder   │◀───│  Frame Parser   │  │
│  │  VideoDecoder        │    │  (HW Accelerated)│    │  (fMP4 Unpack)  │  │
│  │  (Hardware)          │    │                  │    │                 │  │
│  └──────────────────────┘    └──────────────────┘    └─────────────────┘  │
│           │                                                               │
│           ▼                                                               │
│  ┌──────────────────────┐                                                 │
│  │  Canvas/Video        │                                                 │
│  │  Element Rendering   │                                                 │
│  └──────────────────────┘                                                 │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### Relay Server Architecture (Multi-Device)

For scenarios requiring centralized device management and remote access, the optional Relay Server provides a hub for multiple KVM devices:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        RELAY SERVER (Rust/Actix Web)                        │
│                           http://{hostname}.local:8881                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────────┐   │
│  │   Actix Web 4    │  │   Tera Templates │  │    Device Registry     │   │
│  │   HTTP Server    │  │   (HTML Views)   │  │    (State Manager)     │   │
│  │                  │  │                  │  │                        │   │
│  │  GET /           │  │  base.html       │  │  - Device heartbeats   │   │
│  │  GET /dashboard  │  │  dashboard.html  │  │  - Viewer tracking     │   │
│  │  GET /kvm?host.. │  │  kvm_client.html │  │  - Stream config       │   │
│  │  GET /api/*      │  │  error.html      │  │  - Online status       │   │
│  │  GET /static/*   │  │                  │  │                        │   │
│  └────────┬─────────┘  └──────────────────┘  └────────────┬───────────┘   │
│           │                                               │               │
│           ▼                                               ▼               │
│  ┌────────────────────────────────────────────────────────────────────┐   │
│  │                      WebSocket Relay (actix-ws)                    │   │
│  │                                                                    │   │
│  │   GET /ws/device/{hostname}  ←── H.264 frames + heartbeats        │   │
│  │   GET /ws/viewer/{hostname}  ──→ H.264 frames to browser          │   │
│  │                              ←── Input events from browser         │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│           │                                               │               │
│           │  ┌────────────────────────────────────────┐   │               │
│           └──│         mDNS Discovery (mdns-sd)       │───┘               │
│              │  Service: _clever-kvm._tcp.local.      │                   │
│              └────────────────────────────────────────┘                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
         │                                                      │
         │ Device Connection                                    │ Viewer Connection
         │ (Video Producer)                                     │ (Video Consumer)
         ▼                                                      ▼
┌─────────────────────┐                              ┌─────────────────────┐
│   Tauri KVM App     │                              │   Web Browser       │
│   (Device)          │                              │   (Viewer)          │
│                     │                              │                     │
│  ┌───────────────┐  │                              │  ┌───────────────┐  │
│  │ Screen Capture│  │                              │  │ WebCodecs     │  │
│  │ H.264 Encoder │  │                              │  │ H.264 Decoder │  │
│  │ WS Publisher  │  │                              │  │ Input Handler │  │
│  └───────────────┘  │                              │  └───────────────┘  │
└─────────────────────┘                              └─────────────────────┘
```

### Relay Server Technology Stack

| Component | Technology | Purpose |
|-----------|------------|---------|
| **HTTP Server** | Actix Web 4 | High-performance async web framework (400k+ req/sec) |
| **WebSocket** | actix-ws | Low-latency bidirectional communication |
| **Templating** | Tera | Jinja2-style templates with inheritance |
| **Static Files** | actix-files | Efficient static asset serving |
| **mDNS** | mdns-sd | Zero-config service discovery |
| **Async Runtime** | Tokio | Multi-threaded I/O with work-stealing |

### Relay Server Templates

```
relay-server/templates/
├── base.html           # Base template with common layout, styles, scripts
├── dashboard.html      # Device list with status indicators (extends base)
├── kvm_client.html     # Full KVM viewer with H.264 decoder (extends base)
└── error.html          # Error pages with user-friendly messages (extends base)
```

**Template Variables (kvm_client.html):**
```javascript
window.KVM_CONFIG = {
    serverHostname: "{{ server_hostname }}",
    serverPort: "{{ server_port }}",
    deviceHostname: "{{ device_hostname }}",
    wsUrl: "ws://{{ server_hostname }}.local:{{ server_port }}/ws/viewer/{{ device_hostname }}",
    width: {{ width }},
    height: {{ height }},
    audio: {{ audio_enabled | lower }},
    codec: "{{ codec }}"
};
```

## Implementation Details

### 1. H.264 Hardware Encoder (`h264_encoder.rs`)

**Location**: `src-tauri/src/streaming/codecs/h264_encoder.rs`

#### Features
- Hardware acceleration detection (NVENC, QuickSync, AMF)
- Software fallback when hardware is unavailable
- RGBA/BGRA to YUV420 color space conversion
- SPS/PPS parameter set generation
- Configurable encoding presets

#### Hardware Detection (Cross-Platform)
```rust
fn detect_hardware_acceleration() -> HardwareAccel {
    // ============ Windows ============
    #[cfg(target_os = "windows")]
    {
        // Check for NVIDIA NVENC
        if std::path::Path::new("C:\\Windows\\System32\\nvEncodeAPI64.dll").exists() {
            return HardwareAccel::NVENC;
        }
        // Check for Intel QuickSync
        if std::path::Path::new("C:\\Windows\\System32\\libmfx64.dll").exists() ||
           std::path::Path::new("C:\\Windows\\System32\\mfxhw64.dll").exists() {
            return HardwareAccel::QuickSync;
        }
        // Check for AMD AMF
        if std::path::Path::new("C:\\Windows\\System32\\amfrt64.dll").exists() {
            return HardwareAccel::AMF;
        }
    }
    
    // ============ Linux ============
    #[cfg(target_os = "linux")]
    {
        // Check for NVIDIA NVENC
        if std::path::Path::new("/usr/lib/libnvidia-encode.so").exists() ||
           std::path::Path::new("/usr/lib/x86_64-linux-gnu/libnvidia-encode.so").exists() {
            return HardwareAccel::NVENC;
        }
        // Check for Intel VAAPI
        if std::path::Path::new("/dev/dri/renderD128").exists() {
            return HardwareAccel::QuickSync; // VAAPI
        }
    }
    
    // ============ macOS ============
    #[cfg(target_os = "macos")]
    {
        // VideoToolbox is always available on macOS
        return HardwareAccel::QuickSync; // VideoToolbox
    }
    
    HardwareAccel::Software
}
```

#### Configuration Presets
```rust
// Ultra-low latency (gaming, interactive)
H264Config::ultra_low_latency()
// - Framerate: 60 FPS
// - Bitrate: 6000 kbps
// - Keyframe interval: 30 frames
// - Target latency: 16ms

// Balanced (general use)
H264Config::balanced()
// - Framerate: 30 FPS
// - Bitrate: 4000 kbps
// - Keyframe interval: 60 frames
// - Target latency: 33ms

// High quality (presentation, movies)
H264Config::high_quality()
// - Framerate: 60 FPS
// - Bitrate: 8000 kbps
// - Keyframe interval: 60 frames
// - Target latency: 16ms
```

### 2. Low-Latency Pipeline (`low_latency_pipeline.rs`)

**Location**: `src-tauri/src/streaming/handlers/low_latency_pipeline.rs`

#### Architecture
The pipeline uses a multi-task async architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    LowLatencyPipeline                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │  Capture Task   │  │  Receive Task   │  │  Send Task  │ │
│  │  (Video Loop)   │  │  (Input/Ctrl)   │  │  (Output)   │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
│           │                    │                   │        │
│           ▼                    ▼                   ▼        │
│      frame_tx ────────────────────────────────▶ frame_rx   │
│                                                             │
│      control_tx ──────────────────────────────▶ control_rx │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### Video Capture Task (Cross-Platform)
```rust
// Maintains target framerate
let frame_interval = Duration::from_micros(1_000_000 / framerate as u64);

while running {
    // Capture screen using platform-native API:
    // - Windows: GDI (BitBlt) or DXGI Desktop Duplication
    // - Linux:   X11 (XGetImage)
    // - macOS:   Core Graphics (CGDisplayCreateImage)
    let rgba_data = capture.capture_rgba()?;
    
    // Encode to H.264
    let encoded = encoder.encode_rgba(&rgba_data, force_keyframe)?;
    
    // Package as fMP4 fragment
    let fragment = create_fmp4_fragment(&encoded, width, height, framerate);
    
    // Send to WebSocket sender
    frame_tx.send(fragment).await?;
}
```

#### Input Handling
The pipeline handles the following input events from the client:

| Event Type | Description |
|------------|-------------|
| `mousemove` | Mouse cursor movement |
| `mousedown` | Mouse button press |
| `mouseup` | Mouse button release |
| `wheel` | Mouse scroll wheel |
| `keydown` | Keyboard key press |
| `keyup` | Keyboard key release |
| `ping` | Latency measurement |
| `quality_update` | Quality adjustment request |

### 3. Frame Container Format

The H.264 frames are packaged in a custom container format for efficient transmission:

```
┌──────────────────────────────────────────────────────────────┐
│                    Frame Container Header                    │
├──────────────┬──────────────┬────────────────┬──────────────┤
│  Magic (4B)  │  Width (4B)  │  Height (4B)   │ Timestamp(8B)│
│   "H264"     │   uint32     │   uint32       │   uint64     │
├──────────────┼──────────────┼────────────────┼──────────────┤
│  Size (4B)   │  Flags (1B)  │    Data (variable)            │
│   uint32     │   bitfield   │    H.264 NAL units            │
└──────────────┴──────────────┴─────────────────────────────────┘

Flags:
  bit 0: Is keyframe (1 = keyframe, 0 = delta frame)
  bit 1-7: Reserved
```

### 4. Web Client H.264 Decoder (`h264-decoder.js`)

**Location**: `src-tauri/web-client/h264-decoder.js`

#### WebCodecs Integration
```javascript
class H264Decoder {
    async initialize(width, height) {
        // Check WebCodecs support
        if (!('VideoDecoder' in window)) {
            throw new Error('WebCodecs not supported');
        }
        
        // Create hardware-accelerated decoder
        this.decoder = new VideoDecoder({
            output: (frame) => this.handleDecodedFrame(frame),
            error: (e) => this.handleError(e)
        });
        
        // Configure for H.264 Baseline profile
        await this.decoder.configure({
            codec: 'avc1.42E01F', // H.264 Baseline Level 3.1
            hardwareAcceleration: 'prefer-hardware',
            optimizeForLatency: true
        });
    }
}
```

#### Frame Decoding Flow
```javascript
async decodeFrame(frameData) {
    // Parse container header
    const header = this.parseHeader(frameData);
    
    // Extract H.264 NAL units
    const nalData = frameData.slice(HEADER_SIZE);
    
    // Create encoded chunk
    const chunk = new EncodedVideoChunk({
        type: header.isKeyframe ? 'key' : 'delta',
        timestamp: header.timestamp,
        data: nalData
    });
    
    // Decode (hardware accelerated)
    this.decoder.decode(chunk);
}
```

### 5. Client Integration (`kvm-client.js`)

**Location**: `src-tauri/web-client/kvm-client.js`

#### H.264 Frame Detection
```javascript
handleBinaryVideoFrame(data) {
    const view = new DataView(data.buffer);
    
    // Check for H.264 magic header
    const magic = String.fromCharCode(
        view.getUint8(0),
        view.getUint8(1),
        view.getUint8(2),
        view.getUint8(3)
    );
    
    if (magic === 'H264') {
        this.handleH264VideoFrame(data);
    } else {
        // Fallback to legacy RGBA handling
        this.handleRGBAVideoFrame(data);
    }
}
```

## Protocol Messages

### Server → Client Messages

| Message Type | Description | Fields |
|--------------|-------------|--------|
| `stream_init` | Initial stream configuration | `width`, `height`, `framerate`, `codec`, `sps_b64`, `pps_b64` |
| `server_info` | Server information | `width`, `height`, `hostname`, `monitor`, `codec`, `audio` |
| `monitors` | Available monitors list | `monitors[]` |
| `keyframe` | Keyframe notification | `timestamp`, `size` |
| `stats` | Stream statistics | `fps`, `bitrate_kbps`, `latency_ms`, `frames_sent`, `frames_dropped` |
| `pong` | Ping response | `timestamp` |

### Client → Server Messages

| Message Type | Description | Fields |
|--------------|-------------|--------|
| `mousemove` | Mouse movement | `x`, `y` |
| `mousedown` | Mouse button press | `x`, `y`, `button` |
| `mouseup` | Mouse button release | `x`, `y`, `button` |
| `wheel` | Mouse scroll | `x`, `y`, `delta_x`, `delta_y` |
| `keydown` | Key press | `key`, `key_code` |
| `keyup` | Key release | `key`, `key_code` |
| `ping` | Latency measurement | `timestamp` |
| `quality_update` | Quality request | `quality` (0-100) |

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| End-to-end latency | < 20ms | LAN environment |
| Framerate | 60 FPS | Ultra-low latency mode |
| Bitrate | 4-8 Mbps | Adjustable based on quality |
| Frame size | < 50KB | Typical for 1080p |
| CPU usage | < 10% | With hardware acceleration |

## Browser Compatibility

### WebCodecs Support

| Browser | Version | Hardware Accel |
|---------|---------|----------------|
| Chrome | 94+ | ✅ Yes |
| Edge | 94+ | ✅ Yes |
| Opera | 80+ | ✅ Yes |
| Firefox | Behind flag | ⚠️ Limited |
| Safari | 16.4+ | ✅ Yes |

### Fallback Strategy
1. **Primary**: WebCodecs with hardware acceleration
2. **Secondary**: WebCodecs with software decoding
3. **Tertiary**: Canvas-based RGBA rendering (legacy)

## Configuration

### Server Configuration (`PipelineConfig`)

```rust
PipelineConfig {
    monitor_id: 0,           // Monitor to capture
    width: 0,                // 0 = native resolution
    height: 0,               // 0 = native resolution
    framerate: 60,           // Target FPS
    bitrate_kbps: 6000,      // Target bitrate
    keyframe_interval_sec: 2, // Keyframe interval
    enable_audio: false,     // Audio capture
    enable_hw_accel: true,   // Hardware acceleration
    target_latency_ms: 20,   // Target latency
}
```

### Encoder Configuration (`H264Config`)

```rust
H264Config {
    width: 1920,
    height: 1080,
    framerate: 60,
    bitrate_kbps: 6000,
    keyframe_interval: 120,  // Frames between keyframes
    preset: EncoderPreset::UltraFast,
    rate_control: RateControl::CBR,
    low_latency: true,
    slices: 1,
    target_latency_ms: 16,
}
```

## Troubleshooting

### Black Screen Issues

1. **Check Browser Console**
   ```
   Open DevTools (F12) → Console tab
   Look for: "🎬 H.264 Decoder: Initialized"
   ```

2. **Verify WebCodecs Support**
   ```javascript
   console.log('VideoDecoder' in window); // Should be true
   ```

3. **Check Server Logs**
   ```
   Look for: "🔗 Starting low-latency streaming session"
   Look for: "🎬 Video capture started: WxH @ Xfps"
   ```

### High Latency Issues

1. **Enable Hardware Acceleration**
   - **Windows**: Ensure GPU drivers are up to date (NVIDIA/Intel/AMD)
   - **Linux**: Install VAAPI drivers or NVIDIA encode library
   - **macOS**: VideoToolbox is automatic, ensure no throttling

2. **Reduce Quality Settings**
   ```rust
   PipelineConfig::balanced() // Instead of high_quality()
   ```

3. **Check Network**
   - Use wired connection instead of WiFi
   - Reduce network congestion

### Encoding Failures

1. **Check Hardware Support**
   ```
   Server log: "Using hardware acceleration: NVENC/QuickSync/AMF/Software"
   ```

2. **Verify Resolution**
   - Ensure resolution is divisible by 16
   - Try reducing to 1080p if using 4K

## File Reference

| File | Purpose |
|------|---------|
| `src-tauri/src/streaming/codecs/h264_encoder.rs` | H.264 encoder with cross-platform HW detection |
| `src-tauri/src/streaming/handlers/low_latency_pipeline.rs` | Streaming pipeline |
| `src-tauri/src/core/native_capture.rs` | Cross-platform screen capture (Windows/Linux/macOS) |
| `src-tauri/src/network/server/websocket.rs` | WebSocket handler integration |
| `src-tauri/web-client/h264-decoder.js` | Browser H.264 decoder |
| `src-tauri/web-client/kvm-client.js` | Main web client |
| `src-tauri/web-client/kvm-template.html` | HTML template |

## Platform-Specific Troubleshooting

### Windows
1. **Black Screen**: Ensure GPU drivers are up to date
2. **No Hardware Encoding**: Install latest NVIDIA/Intel/AMD drivers
3. **Permission Issues**: Run as administrator if screen capture fails

### Linux (Ubuntu/X11)
1. **Black Screen**: Check X11 permissions with `xhost +local:`
2. **No Hardware Encoding**: 
   - NVIDIA: Install `libnvidia-encode` package
   - Intel: Install `intel-media-va-driver` or `intel-media-va-driver-non-free`
3. **Permission Issues**: Ensure user is in `video` group: `sudo usermod -a -G video $USER`
4. **Wayland Users**: This implementation requires X11. Use `GDK_BACKEND=x11` or switch to X11 session

### macOS
1. **Black Screen**: Grant Screen Recording permission in System Preferences → Security & Privacy → Privacy
2. **Permission Denied**: The app must be signed or have proper entitlements
3. **Performance**: VideoToolbox hardware encoding is automatic on Apple Silicon and Intel Macs

## Future Improvements

1. **WebRTC Integration**: Replace WebSocket with WebRTC for UDP-based transport
2. **Adaptive Bitrate**: Dynamic bitrate adjustment based on network conditions
3. **B-Frame Support**: Add B-frames for better compression
4. **Audio Streaming**: Implement Opus audio encoding and streaming
5. **Multi-Monitor**: Support streaming multiple monitors simultaneously

## Changelog

### Version 4.1.0
- Implemented H.264 hardware-accelerated encoding
- Added WebCodecs-based browser decoding
- Created low-latency streaming pipeline
- Fixed black screen issues
- Added multiple quality presets
- **Cross-platform support**: Windows, Linux (X11), macOS
- Platform-specific screen capture implementations
- Platform-specific hardware encoder detection
