# RDEngine VP9 Streaming Implementation

## Overview

This document describes the RDEngine streaming system — a low-latency video/audio streaming engine for the Clever KVM application. The architecture is inspired by [RustDesk](https://github.com/rustdesk/rustdesk)'s approach to remote desktop streaming, using VP8/VP9 encoding via libvpx and a minimal binary protocol over WebSocket.

RDEngine replaces the previous H.264 pipeline and relay server architecture with a simpler, more maintainable system that delivers comparable latency with fewer moving parts and no hardware-specific dependencies.

## Architecture

### Design Principles

Borrowed from RustDesk's proven patterns:

1. **Dedicated OS threads** — video capture/encode runs on a `std::thread`, not async tokio tasks (avoids jitter from task switching)
2. **Frame deduplication** — byte-compare raw frames before encoding (skip unchanged screens)
3. **Buffer reuse** — pre-allocated YUV buffers recycled across frames
4. **Adaptive QoS** — dynamic FPS/bitrate adjustment based on network RTT
5. **Binary protocol** — minimal-overhead length-prefixed frames (no protobuf dependency)
6. **Separated channels** — video and control messages on independent paths to prevent head-of-line blocking

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SERVER (Rust/Tauri)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────────┐  │
│  │  Native Screen   │───▶│  BGRA → I420     │───▶│  VPX Encoder         │  │
│  │  Capture         │    │  Color Convert   │    │  (VP9 via libvpx)    │  │
│  │  (Platform API)  │    │                  │    │                      │  │
│  └──────────────────┘    └──────────────────┘    └──────────────────────┘  │
│         │                        │                         │               │
│         ▼                        ▼                         ▼               │
│    BGRA Frame              I420 YUV Data             VP9 Bitstream         │
│    (Native)                (Dedup Check)             (Compressed)          │
│                                                            │               │
│  ┌──────────────────┐                                      │               │
│  │  Audio Capture   │───▶ Opus Encoder ───────────────▶   Binary           │
│  │  (cpal)          │    (48kHz stereo)                 Protocol           │
│  └──────────────────┘                                      │               │
│                                                            │               │
│  ┌──────────────────┐                                      │               │
│  │  QoS Controller  │◀── RTT from ping/pong ──────────┐   │               │
│  │  (Adaptive FPS,  │                                  │   │               │
│  │   Bitrate)       │                                  │   │               │
│  └──────────────────┘                                  │   │               │
│                                                        │   │               │
└────────────────────────────────────────────────────────┼───┼───────────────┘
                                                         │   │
                                                    WebSocket (Binary)
                                                         │   │
┌────────────────────────────────────────────────────────┼───┼───────────────┐
│                              CLIENT (Browser)          │   │               │
├────────────────────────────────────────────────────────┼───┼───────────────┤
│                                                        │   ▼               │
│  ┌──────────────────────┐    ┌──────────────────┐  ┌─────────────────┐    │
│  │  Canvas Rendering    │◀───│  WebCodecs        │◀─│  Binary Frame   │    │
│  │  (drawImage)         │    │  VideoDecoder     │  │  Parser         │    │
│  │                      │    │  (VP9 HW decode)  │  │  (Header Parse) │    │
│  └──────────────────────┘    └──────────────────┘  └─────────────────┘    │
│                                                                           │
│  ┌──────────────────────┐    ┌──────────────────┐                         │
│  │  OpusDecoder         │◀───│  Audio Frame      │                         │
│  │  (Web Audio API)     │    │  Parser           │                         │
│  └──────────────────────┘    └──────────────────┘                         │
│                                                                           │
│  ┌──────────────────────┐                                                 │
│  │  Input Handler       │───▶ JSON messages (mouse, keyboard, scroll)     │
│  │  (keyboard/mouse)    │                                                 │
│  └──────────────────────┘                                                 │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src-tauri/src/rdengine/
├── mod.rs              # Module root and public re-exports
├── codec.rs            # VPX encoder wrapper (VP8/VP9 via libvpx-sys FFI)
├── video_service.rs    # Dedicated video capture/encode thread per display
├── audio_service.rs    # Audio capture (cpal) + Opus encoding thread
├── connection.rs       # WebSocket connection handler (per-client session)
├── protocol.rs         # Binary frame protocol definitions
└── qos.rs              # Adaptive quality control (FPS/bitrate from RTT)
```

### Module Dependencies

```
connection.rs
    ├── video_service.rs ──▶ codec.rs
    │       └── qos.rs
    ├── audio_service.rs
    └── protocol.rs
```

## Codec Layer (`codec.rs`)

Wraps libvpx-sys raw FFI bindings in a safe Rust API. Supports both VP8 and VP9 codecs.

### Encoder Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| Codec | VP9 | VP8 for lower CPU, VP9 for better compression |
| Width/Height | Display resolution | Auto-detected from screen capture |
| Bitrate | 2000 kbps | CBR rate control for predictable bandwidth |
| FPS | 30 | Target frame rate at encoder level |
| Error Resilience | Enabled | Tolerates packet loss without full corruption |
| Keyframe Interval | On-demand | No periodic keyframes; sent on client request |
| Speed | 6 (VP9) / 6 (VP8) | Realtime speed profile for low CPU usage |

### Key Functions

- **`VpxEncoder::new(config)`** — Initialize encoder with libvpx FFI (`vpx_codec_enc_init`)
- **`encode(input)`** — Encode a frame (BGRA or I420 input), returns compressed packets
- **`request_keyframe()`** — Force next frame to be a keyframe
- **`rgba_to_i420()`** — Color space conversion with BT.601 coefficients

### VPX Codec Selection

| Codec | Compression | CPU Usage | Browser Support |
|-------|------------|-----------|-----------------|
| VP8 | Good | Low | All modern browsers |
| VP9 | ~30% better | Medium | Chrome 29+, Firefox 28+, Edge 79+ |

VP9 is the default. VP8 is available for lower-power devices or broader compatibility.

## Video Service (`video_service.rs`)

Runs the capture → dedup → encode → broadcast loop on a dedicated OS thread.

### Thread Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Video Service Thread (std::thread)          │
│                                                             │
│   loop {                                                    │
│       1. Sleep until next frame time (QoS FPS pacing)       │
│       2. Capture screen (platform-native API)               │
│       3. Compare with previous frame bytes (dedup)          │
│          └─ If identical → skip encoding                    │
│       4. Convert BGRA → I420 (pre-allocated buffer)         │
│       5. Encode with VPX encoder                            │
│       6. Wrap in binary protocol message                    │
│       7. Send to crossbeam broadcast channel                │
│       8. Update statistics (counters, timing)               │
│   }                                                         │
└─────────────────────────────────────────────────────────────┘
```

### Frame Deduplication

Before encoding, the current frame bytes are compared against the previous frame. If frames are byte-identical (common during static desktop), encoding is skipped entirely. This saves significant CPU on idle screens — a pattern borrowed directly from RustDesk's `video_service.rs`.

### Client Subscription

Multiple clients can subscribe to the same `VideoService` via crossbeam channels:

```rust
let (video_tx, video_rx) = crossbeam_channel::bounded(30);
video_service.subscribe(video_tx);
```

The bounded channel (capacity 30) acts as a quality gate — if a slow client falls behind, oldest frames are dropped rather than accumulating memory.

## Audio Service (`audio_service.rs`)

Captures system audio via cpal and encodes with Opus codec.

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| Sample Rate | 48000 Hz | Standard for Opus codec |
| Channels | 2 (stereo) | System audio is typically stereo |
| Opus Bitrate | 128 kbps | High quality, adjustable |
| Frame Duration | 10 ms | Lowest latency Opus frame size |

### Audio Pipeline

```
System Audio → cpal callback → Ring Buffer → Opus encode → Binary message → Channel
```

The cpal audio callback writes samples into a `parking_lot::Mutex<Vec<f32>>` buffer. A separate thread drains this buffer every frame duration (10ms), encodes it with Opus, wraps in the binary protocol, and sends to the broadcast channel.

## Binary Protocol (`protocol.rs`)

All frames are sent as binary WebSocket messages using a minimal length-prefixed format.

### Message Envelope

```
[1B type] [4B payload_length (LE)] [payload...]
```

### Message Types

| Type ID | Name | Direction |
|---------|------|-----------|
| 0x01 | Video Frame | Server → Client |
| 0x02 | Audio Frame | Server → Client |
| 0x03 | Cursor | Server → Client |
| 0x04 | Clipboard | Bidirectional |
| 0x05 | Ping | Bidirectional |
| 0x06 | Pong | Bidirectional |

### Video Frame Payload

```
[1B codec] [1B flags] [4B width (LE)] [4B height (LE)] [8B timestamp_ms (LE)] [data...]
```

- **Codec**: `0x01` = VP8, `0x02` = VP9, `0x03` = H.264 (reserved)
- **Flags**: `0x01` = keyframe
- **Total header overhead**: 5 (envelope) + 18 (video header) = 23 bytes per frame

### Audio Frame Payload

```
[1B codec] [4B sample_rate (LE)] [1B channels] [8B timestamp_ms (LE)] [data...]
```

- **Codec**: `0x10` = Opus
- **Total header overhead**: 5 (envelope) + 14 (audio header) = 19 bytes per frame

### Control Messages (JSON over Text WebSocket)

Sent as JSON text messages (not binary). Tagged with `"type"` field:

```json
// Client requests keyframe
{"type": "request_keyframe"}

// Client reports latency
{"type": "network_stats", "latency": 15}

// Client adjusts bitrate
{"type": "bitrate_update", "bitrate_kbps": 4000}
```

### Input Messages (JSON over Text WebSocket)

```json
{"type": "mousemove", "x": 512.0, "y": 384.0}
{"type": "mousedown", "x": 100.0, "y": 200.0, "button": 0}
{"type": "keydown", "key": "a", "code": "KeyA"}
```

## QoS Controller (`qos.rs`)

Adaptive quality control inspired by RustDesk's `video_qos.rs`.

### Algorithm

```
Every 3 seconds:
  smoothed_rtt = (all-time min RTT + recent 10-sample min) / 2

  if smoothed_rtt > 100ms:
      fps *= 0.8          # Reduce by 20%
      bitrate *= 0.8      # Reduce by 20%
      quality_level = "low"

  elif smoothed_rtt < 30ms:
      fps *= 1.1          # Increase by 10%
      bitrate *= 1.1      # Increase by 10%
      quality_level = "high"

  else:
      keep current settings
      quality_level = "balanced"

  Clamp fps to [5, 60]
  Clamp bitrate to [400, 12000] kbps
```

### RTT Measurement

Ping/pong messages are exchanged every ~2 seconds. The server records round-trip time and feeds it into the QoS controller. The smoothed RTT uses a min-window approach:
- 50% weight on the all-time minimum (floor estimate)
- 50% weight on the minimum of the last 10 samples (recent trend)

This mirrors RustDesk's approach, which prioritizes stability over reactivity.

## Connection Handler (`connection.rs`)

Manages a single WebSocket client session. Orchestrates all services.

### Connection Lifecycle

```
1. Client connects via WebSocket
2. Server sends ServerInfo JSON:
   {
     "type": "server_info",
     "width": 1920,
     "height": 1080,
     "codec": "vp9",
     "framerate": 30,
     "bitrate_kbps": 2000,
     "audio_enabled": true,
     "protocol_version": 2
   }
3. Start VideoService thread (capture → encode → broadcast)
4. Start AudioService thread (if enabled)
5. Enter tokio::select! loop:
   a. video_rx.recv() → send binary frame to client
   b. audio_rx.recv() → send binary frame to client
   c. ws.recv()       → handle text control/input messages
   d. ping timer      → send ping, measure RTT
   e. qos timer       → adjust FPS/bitrate if needed
6. On disconnect: stop VideoService, AudioService, clean up
```

### Message Multiplexing

The handler uses `tokio::select!` to multiplex:
- **Video frames** from `crossbeam_channel::Receiver<VideoFrame>`
- **Audio frames** from `crossbeam_channel::Receiver<AudioFrame>`
- **Client messages** from the WebSocket stream
- **Periodic timers** for ping/pong and QoS adjustments

## Browser Client

### VP9 Decoding (`vpx-decoder.js`)

Uses the WebCodecs API (`VideoDecoder`) for hardware-accelerated VP9 decoding:

```javascript
const decoder = new VideoDecoder({
  output: (frame) => {
    ctx.drawImage(frame, 0, 0);
    frame.close();
  },
  error: (e) => console.error('Decode error:', e)
});

decoder.configure({
  codec: 'vp09.00.10.08', // VP9 Profile 0
  codedWidth: 1920,
  codedHeight: 1080
});
```

### Binary Protocol Parsing (`kvm-client.js`)

The client detects RDEngine binary frames by checking the first byte:

```javascript
handleBinaryMessage(data) {
  const type = data[0];
  if (type === 0x01) {  // MSG_VIDEO_FRAME
    const payloadLen = new DataView(data.buffer).getUint32(1, true);
    const codec = data[5];
    const flags = data[6];
    const width = new DataView(data.buffer).getUint32(7, true);
    const height = new DataView(data.buffer).getUint32(11, true);
    const timestamp = Number(new DataView(data.buffer).getBigUint64(15, true));
    const frameData = data.slice(23);
    // Decode with WebCodecs VideoDecoder
  }
}
```

## Cross-Platform Screen Capture

The video service uses the existing platform-native capture layer:

| Platform | API | Module |
|----------|-----|--------|
| Windows | GDI (GetDC, BitBlt) | `core/native_capture.rs` |
| Linux (X11) | X11 (XGetImage) via x11rb | `core/native_capture.rs` |
| macOS | Core Graphics (CGDisplayCreateImage) | `core/native_capture.rs` |

All platforms output BGRA frames, which the video service converts to I420 for VPX encoding.

## Dependencies

### Rust (Backend)

| Crate | Version | Purpose |
|-------|---------|---------|
| `libvpx-sys` | 1.4 | VP8/VP9 encoding via libvpx FFI |
| `libc` | 0.2 | C types for libvpx FFI |
| `cpal` | 0.15 | Cross-platform audio capture |
| `opus` | 0.3 | Opus audio codec |
| `crossbeam-channel` | 0.5 | Lock-free MPMC channels |
| `crossbeam-queue` | 0.3 | Lock-free bounded queues |
| `parking_lot` | 0.12 | High-performance locks |
| `mimalloc` | 0.1 | Microsoft's high-performance allocator |

### Browser (Frontend)

| API | Purpose | Support |
|-----|---------|---------|
| WebCodecs `VideoDecoder` | VP9 hardware decoding | Chrome 94+, Edge 94+, Safari 16.4+ |
| Web Audio API | Opus audio playback | All modern browsers |
| WebSocket (binary) | Frame transport | All modern browsers |

## Performance Characteristics

| Metric | Typical Value | Notes |
|--------|--------------|-------|
| End-to-end latency | 20–40ms | LAN, depends on display refresh rate |
| Video encode time | 2–8ms | VP9 realtime preset, 1080p |
| Frame dedup rate | 60–95% | Static desktop = high skip rate |
| Bandwidth (1080p) | 2–6 Mbps | VP9 at 30fps |
| Bandwidth (1080p) | 6–12 Mbps | VP9 at 60fps |
| CPU usage (server) | 5–15% | VP9 software encode on modern CPU |
| Memory (server) | ~50MB | With mimalloc allocator |

## Configuration

### Quality Presets

| Preset | Bitrate | FPS | Use Case |
|--------|---------|-----|----------|
| Gaming | 12000 kbps | 60 | Low-latency gaming/video |
| Desktop | 6000 kbps | 30 | General remote work |
| Low Bandwidth | 2000 kbps | 15 | Slow networks/mobile |

### Server Start Options

```json
{
  "monitor_id": 0,
  "codec": "vp9",
  "audio": true
}
```

## Comparison with Previous Architecture

| Feature | Old (H.264 Pipeline) | New (RDEngine VP9) |
|---------|---------------------|-------------------|
| Video codec | H.264 (HW-dependent) | VP9 via libvpx (software) |
| Audio | WebRTC peer connection | Opus over WebSocket |
| Protocol | fMP4 container | Minimal binary framing |
| Relay server | Actix Web relay | Removed (direct only) |
| Thread model | Async tokio tasks | Dedicated OS threads |
| Frame dedup | None | Byte-compare before encode |
| QoS | None | RTT-based adaptive FPS/bitrate |
| Complexity | ~6,500 lines (13 files) | ~2,200 lines (6 files) |
| Dependencies | webrtc, zed-scap, av-data, image | libvpx-sys, cpal, crossbeam |
