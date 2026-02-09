# RDEngine VP9 Streaming Implementation - Updated 5th February 2026

## Overview

This document describes the RDEngine streaming system — a low-latency video/audio streaming engine for the Clever KVM application. The architecture is inspired by [RustDesk](https://github.com/rustdesk/rustdesk)'s approach to remote desktop streaming, using VP8/VP9 encoding via libvpx and a **WebRTC DataChannel transport** for real-time peer-to-peer delivery.

RDEngine replaces the previous H.264 pipeline and relay server architecture with a simpler, more maintainable system that delivers significantly lower latency through UDP-based WebRTC DataChannels, with an automatic fallback to WebSocket binary transport when WebRTC is unavailable.

## Architecture

### Design Principles

Borrowed from RustDesk's proven patterns, enhanced with WebRTC transport:

1. **Dedicated OS threads** — video capture/encode runs on a `std::thread`, not async tokio tasks (avoids jitter from task switching)
2. **Frame deduplication** — byte-compare raw frames before encoding (skip unchanged screens)
3. **Buffer reuse** — pre-allocated YUV buffers recycled across frames
4. **Adaptive QoS** — dynamic FPS/bitrate adjustment based on network RTT
5. **Binary protocol** — minimal-overhead length-prefixed frames (no protobuf dependency)
6. **Separated channels** — video and control messages on independent paths to prevent head-of-line blocking
7. **WebRTC DataChannels** — UDP-based transport for video/audio/cursor (no TCP head-of-line blocking)
8. **Graceful fallback** — automatic fallback to WebSocket binary if WebRTC negotiation fails

### Transport Architecture

The system uses a **hybrid transport model**:

| Data Type | Transport | Reliability | Why |
|-----------|-----------|-------------|-----|
| Video frames | WebRTC DataChannel | Unreliable, unordered | Lowest latency; dropped frames don't stall subsequent frames |
| Audio frames | WebRTC DataChannel | Reliable | Opus needs ordered delivery for continuous playback |
| Cursor updates | WebRTC DataChannel | Reliable | Small messages, must arrive |
| Input events | WebSocket (JSON) | Reliable (TCP) | Must be reliable; low frequency; direction: client → server |
| Control messages | WebSocket (JSON) | Reliable (TCP) | Keyframe requests, QoS updates |
| SDP/ICE signaling | WebSocket (JSON) | Reliable (TCP) | WebRTC negotiation only happens once |

### High-Level Flow

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              SERVER (Rust/Tauri)                             │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────────┐   │
│  │  Native Screen   │───▶│  BGRA → I420     │───▶│  VPX Encoder         │   │
│  │  Capture         │    │  Color Convert   │    │  (VP9 via libvpx)    │   │
│  │  (Platform API)  │    │                  │    │                      │   │
│  └──────────────────┘    └──────────────────┘    └──────────────────────┘   │
│         │                        │                         │                │
│         ▼                        ▼                         ▼                │
│    BGRA Frame              I420 YUV Data             VP9 Bitstream          │
│    (Native)                (Dedup Check)             (Compressed)           │
│                                                            │                │
│  ┌──────────────────┐                                      │                │
│  │  Audio Capture   │───▶ Opus Encoder ───────────────▶   Binary            │
│  │  (cpal)          │    (48kHz stereo)                 Protocol            │
│  └──────────────────┘                                      │                │
│                                                            │                │
│  ┌──────────────────┐                    ┌─────────────────┴───────────┐    │
│  │  QoS Controller  │◀── RTT ───────┐   │  WebRtcTransport            │    │
│  │  (Adaptive FPS,  │               │   │  ┌─ video DC (unreliable) ──┼─┐  │
│  │   Bitrate)       │               │   │  ├─ audio DC (reliable)   ──┼─┤  │
│  └──────────────────┘               │   │  └─ cursor DC (reliable)  ──┼─┤  │
│                                     │   └─────────────────────────────┘ │  │
│                                     │                                   │  │
│  ┌──────────────────────────────────┼───────────────────────────────┐   │  │
│  │  WebSocket (signaling + input)   │                               │   │  │
│  │  ├─ SDP offer/answer            │  ◄───── signaling ──────────► │   │  │
│  │  ├─ ICE candidates              │                               │   │  │
│  │  ├─ Input (keyboard/mouse)      │  ◄───── client input         │   │  │
│  │  └─ Control (keyframe, QoS)     │                               │   │  │
│  └──────────────────────────────────┼───────────────────────────────┘   │  │
│                                     │                                   │  │
└─────────────────────────────────────┼───────────────────────────────────┼──┘
                                      │                                   │
                              WebSocket (TCP)                   WebRTC DC (UDP)
                                      │                                   │
┌─────────────────────────────────────┼───────────────────────────────────┼──┐
│                              CLIENT (Browser)                           │  │
├─────────────────────────────────────┼───────────────────────────────────┼──┤
│                                     │                                   │  │
│  ┌──────────────────────────────────┼───────────────────────────────┐   │  │
│  │  WebSocket Handler              │                               │   │  │
│  │  ├─ SDP answer generation       │                               │   │  │
│  │  ├─ ICE candidate forwarding    │                               │   │  │
│  │  ├─ Input events (send)         │                               │   │  │
│  │  └─ Control messages            │                               │   │  │
│  └──────────────────────────────────┼───────────────────────────────┘   │  │
│                                     │                                   │  │
│  ┌──────────────────────────────────┼───────────────────────────────┐   │  │
│  │  WebRtcTransport (JS)           │                               │   │  │
│  │  ├─ "video" DC ─► handleBinaryVideoFrame() ─► VideoDecoder     │◄──┘  │
│  │  ├─ "audio" DC ─► handleAudioFrame()                           │      │
│  │  └─ "cursor" DC ─► handleCursorMessage()                       │      │
│  └──────────────────────────────────┼───────────────────────────────┘      │
│                                     │                                      │
│  ┌──────────────────────┐    ┌──────────────────┐  ┌─────────────────┐    │
│  │  Canvas Rendering    │◀───│  WebCodecs        │◀─│  Binary Frame   │    │
│  │  (drawImage)         │    │  VideoDecoder     │  │  Parser         │    │
│  │                      │    │  (VP9 HW decode)  │  │  (Header Parse) │    │
│  └──────────────────────┘    └──────────────────┘  └─────────────────┘    │
│                                                                           │
│  ┌──────────────────────┐                                                 │
│  │  Input Handler       │───▶ JSON messages (mouse, keyboard, scroll)     │
│  │  (keyboard/mouse)    │         via WebSocket (reliable TCP)            │
│  └──────────────────────┘                                                 │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### WebRTC Signaling Flow

```
Server                              Client
  │                                    │
  │──── server_info (webrtc_enabled) ──►│  (1) Tell client WebRTC is available
  │                                    │
  │──── webrtc_offer (SDP) ───────────►│  (2) Server creates DataChannels + SDP offer
  │                                    │
  │◄─── webrtc_answer (SDP) ──────────│  (3) Client creates PeerConnection + answer
  │                                    │
  │◄──► webrtc_ice_candidate ────────►│  (4) ICE candidate exchange (both directions)
  │                                    │
  │====== DataChannels established =====│  (5) Video/audio/cursor flow via UDP
  │                                    │
  │◄──► WebSocket (input/control) ───►│  (6) Continues for reliable messages
  │                                    │
```

## Module Structure

```
src-tauri/src/rdengine/
├── mod.rs                 # Module root and public re-exports
├── codec.rs               # VPX encoder wrapper (VP8/VP9 via libvpx-sys FFI)
├── video_service.rs       # Dedicated video capture/encode thread per display
├── audio_service.rs       # Audio capture (cpal) + Opus encoding thread
├── connection.rs          # WebSocket + WebRTC connection handler (per-client session)
├── protocol.rs            # Binary frame protocol definitions
├── qos.rs                 # Adaptive quality control (FPS/bitrate from RTT)
└── webrtc_transport.rs    # WebRTC DataChannel transport (peer connection + channels)

src-tauri/web-client/
├── kvm-client.js          # Main KVM client (WebSocket + WebRTC integration)
├── webrtc-transport.js    # Client-side WebRTC DataChannel transport
├── vpx-decoder.js         # VP9 WebCodecs decoder
└── kvm-template.html      # HTML entry point
```

### Module Dependencies

```
connection.rs
    ├── video_service.rs ──▶ codec.rs
    │       └── qos.rs
    ├── audio_service.rs
    ├── protocol.rs
    └── webrtc_transport.rs  ◄── NEW: WebRTC DataChannel transport
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

## WebRTC Transport (`webrtc_transport.rs`)

Manages a WebRTC PeerConnection with three DataChannels for video, audio, and cursor data. Uses the pure-Rust `webrtc` crate (v0.17) — no C/C++ WebRTC dependencies.

### Configuration

| Parameter | Default | LAN Mode | Description |
|-----------|---------|----------|-------------|
| ICE Servers | `stun:stun.l.google.com:19302` | None | STUN for NAT traversal (not needed on LAN) |
| Video Buffer Size | 16 MB | 32 MB | Max buffered data before dropping frames |
| Video Unordered | `true` | `true` | Disable ordering for lowest latency |
| Video Max Retransmits | `0` | `0` | Fire-and-forget — no retransmission |
| ICE Disconnected Timeout | 5s | 5s | Time before marking peer disconnected |
| ICE Failed Timeout | 10s | 10s | Time before marking connection failed |
| ICE Keepalive | 2s | 2s | Interval between STUN keepalives |

### DataChannels

| Channel | Label | Ordered | Max Retransmits | Use Case |
|---------|-------|---------|-----------------|----------|
| Video | `"video"` | No | 0 (fire-and-forget) | VP9 encoded frames; latest frame matters, old ones don't |
| Audio | `"audio"` | Yes | ∞ (reliable) | Opus encoded frames; needs ordered delivery |
| Cursor | `"cursor"` | Yes | ∞ (reliable) | Host cursor position/shape; small messages |

### Backpressure Control

The video DataChannel checks `buffered_amount()` before sending each frame. If the buffer exceeds `video_buffer_size`, the frame is silently dropped rather than accumulating memory. This prevents slow clients from causing unbounded memory growth on the server.

### Event Model

The transport emits events via a `tokio::sync::mpsc` channel:

- `VideoChannelReady` — Video DataChannel opened
- `AudioChannelReady` — Audio DataChannel opened
- `CursorChannelReady` — Cursor DataChannel opened
- `ConnectionStateChanged(state)` — ICE connection state transitions
- `IceCandidate(json)` — Local ICE candidate to forward via WebSocket signaling
- `Error(message)` — Transport-level errors

## Connection Handler (`connection.rs`)

Manages a single WebSocket + WebRTC client session. Orchestrates all services.

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
     "protocol_version": 3,         ◄── v3 = WebRTC support
     "webrtc_enabled": true
   }
3. If WebRTC enabled:
   a. Create WebRtcTransport with DataChannels
   b. Generate SDP offer → send via WebSocket
   c. Wait for SDP answer + ICE candidates from client (via WebSocket)
   d. Spawn event forwarder task for ICE candidates → WebSocket
4. Start VideoService thread (capture → encode → broadcast)
5. Start AudioService thread (if enabled)
6. Enter tokio::select! loop:
   a. video_rx.recv() → send via WebRTC DataChannel (fallback: WebSocket binary)
   b. audio_rx.recv() → send via WebRTC DataChannel (fallback: WebSocket binary)
   c. cursor_rx.recv() → send via WebRTC DataChannel (fallback: WebSocket binary)
   d. ws.recv()       → handle text control/input/signaling messages
   e. ctrl_rx.recv()  → send outbound control messages (inc. ICE candidates)
   f. ping timer      → send ping, measure RTT
   g. qos timer       → adjust FPS/bitrate if needed
7. On disconnect: stop VideoService, AudioService, close WebRTC transport
```

### Message Multiplexing

The handler uses `tokio::select!` to multiplex:
- **Video frames** from `crossbeam_channel::Receiver<VideoFrame>` → WebRTC DataChannel (or WebSocket fallback)
- **Audio frames** from `crossbeam_channel::Receiver<AudioFrame>` → WebRTC DataChannel (or WebSocket fallback)
- **Cursor updates** from `crossbeam_channel::Receiver<CursorUpdate>` → WebRTC DataChannel (or WebSocket fallback)
- **Client messages** from the WebSocket stream (including WebRTC signaling)
- **Control messages** from the internal `ctrl_rx` channel (ICE candidates, QoS updates)
- **Periodic timers** for ping/pong and QoS adjustments

### WebRTC Fallback Strategy

Each frame send attempts WebRTC first. On failure:
- `Ok(false)` — DataChannel not ready or buffer full → frame silently dropped (acceptable for video)
- `Err(e)` — Send error → frame sent via WebSocket binary as fallback
- No WebRTC transport → all frames go through WebSocket binary (legacy mode)

## Browser Client

### WebRTC Transport (`webrtc-transport.js`)

Client-side counterpart to the Rust `WebRtcTransport`. Manages the browser's `RTCPeerConnection` and DataChannel handlers.

**Key responsibilities:**
- Receives SDP offer from server → creates `RTCPeerConnection` → generates SDP answer
- Handles incoming DataChannels (`"video"`, `"audio"`, `"cursor"`) created by the server
- Routes binary DataChannel messages to existing `handleBinaryVideoFrame()`, `handleAudioFrame()`, and `handleCursorMessage()` handlers
- Forwards local ICE candidates to the server via WebSocket

**Integration with `kvm-client.js`:**
```javascript
// When server sends webrtc_offer via WebSocket:
this.webrtcTransport = new WebRtcTransport({
    onVideoFrame: (data) => this.handleBinaryVideoFrame(new Uint8Array(data)),
    onAudioFrame: (data) => this.handleBinaryMessage(new Uint8Array(data)),
    onCursorUpdate: (data) => this.handleCursorMessage(new Uint8Array(data)),
    onStateChange: (state) => { /* update UI */ },
    onIceCandidate: (json) => ws.send(JSON.stringify({
        type: 'webrtc_ice_candidate', candidate: json
    })),
});
const answer = await this.webrtcTransport.handleOffer(offerSdp);
ws.send(JSON.stringify({ type: 'webrtc_answer', sdp: answer }));
```

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
| `webrtc` | 0.17 | Pure Rust WebRTC (DataChannels + media over UDP) |

### Browser (Frontend)

| API / Library | Purpose | Support |
|---------------|---------|---------|
| WebCodecs `VideoDecoder` | VP9 hardware decoding | Chrome 94+, Edge 94+, Safari 16.4+ |
| Web Audio API | Opus audio playback | All modern browsers |
| `RTCPeerConnection` | WebRTC DataChannel transport | All modern browsers |
| `RTCDataChannel` | Binary data transfer (UDP-based) | All modern browsers |
| WebSocket | Signaling + input events (fallback for media) | All modern browsers |

## Performance Characteristics

| Metric | WebRTC Transport | WebSocket Fallback | Notes |
|--------|-----------------|-------------------|-------|
| End-to-end latency | 10–25ms | 20–40ms | WebRTC avoids TCP HOL blocking |
| Video encode time | 2–8ms | 2–8ms | Same VP9 encoder in both modes |
| Frame dedup rate | 60–95% | 60–95% | Static desktop = high skip rate |
| Bandwidth (1080p) | 2–6 Mbps | 2–6 Mbps | VP9 at 30fps |
| Bandwidth (1080p) | 6–12 Mbps | 6–12 Mbps | VP9 at 60fps |
| CPU usage (server) | 5–15% | 5–15% | VP9 software encode on modern CPU |
| Memory (server) | ~80MB | ~50MB | WebRTC adds ~30MB for peer connection |
| Frame drop tolerance | Excellent | Poor | UDP drops don't stall subsequent frames |
| NAT traversal | Built-in (ICE) | Requires port forwarding | WebRTC handles NAT automatically |

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

| Feature | Old (H.264 Pipeline) | RDEngine v2 (WebSocket) | RDEngine v3 (WebRTC) |
|---------|---------------------|------------------------|---------------------|
| Video codec | H.264 (HW-dependent) | VP9 via libvpx | VP9 via libvpx |
| Audio | WebRTC peer connection | Opus over WebSocket | Opus over WebRTC DataChannel |
| Video transport | fMP4 container | WebSocket binary (TCP) | WebRTC DataChannel (UDP) |
| Protocol | fMP4 container | Minimal binary framing | Same binary framing over DataChannel |
| Transport reliability | TCP (reliable) | TCP (reliable) | UDP (unreliable, configurable) |
| HOL blocking | Yes (TCP) | Yes (TCP) | No (UDP DataChannels) |
| NAT traversal | None | None | ICE (STUN/TURN) |
| Encryption | TLS | TLS | DTLS (built into WebRTC) |
| Relay server | Actix Web relay | Removed (direct only) | Removed (peer-to-peer) |
| Thread model | Async tokio tasks | Dedicated OS threads | Dedicated OS threads |
| Frame dedup | None | Byte-compare before encode | Byte-compare before encode |
| QoS | None | RTT-based adaptive FPS/bitrate | RTT-based adaptive FPS/bitrate |
| End-to-end latency | 50-100ms | 20-40ms | 10-25ms |
| Dependencies | webrtc, zed-scap, av-data | libvpx-sys, cpal, crossbeam | + webrtc crate (pure Rust) |
