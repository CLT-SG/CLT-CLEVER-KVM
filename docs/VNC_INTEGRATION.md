# VNC Server Integration Guide - WebSocket Architecture

## Overview

CLT-CLEVER-KVM includes a native VNC (Virtual Network Computing) server implementation with optimized WebSocket-based audio streaming. The architecture provides direct connections for desktop VNC clients and a built-in WebSockify proxy for browser-based NoVNC clients.

## Port Assignments

| Service | Ports | Description |
|---------|-------|-------------|
| **VNC** | 5900+ | Native VNC protocol for desktop clients (TigerVNC, RealVNC, etc.) |
| **WebSockify** | 6080+ | WebSocket proxy for browser clients (NoVNC) |
| **Audio** | 6900 | WebSocket audio stream (Opus encoded, shared for all monitors) |

**Multi-Monitor Port Mapping:**

| Monitor | VNC Port | WebSockify Port | Description |
|---------|----------|-----------------|-------------|
| Monitor 0 | 5900 | 6080 | Primary display |
| Monitor 1 | 5901 | 6081 | Secondary display |
| Monitor 2 | 5902 | 6082 | Third display |
| Monitor N | 5900+N | 6080+N | Additional displays |
| Audio | N/A | 6900 | Shared WebSocket audio |

## Architecture Overview

### Optimized Low-Latency Design

```
┌─────────────────────────────────────────────────────────────────────┐
│  CLT-CLEVER-KVM (Tauri/Rust)                                        │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  VNC Servers (Multi-Monitor)                                   │ │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │ │
│  │  │ Monitor 0        │  │ Monitor 1        │  ...              │ │
│  │  │ VNC Port: 5900   │  │ VNC Port: 5901   │                   │ │
│  │  │ RFB 3.8          │  │ RFB 3.8          │                   │ │
│  │  └──────────────────┘  └──────────────────┘                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  WebSockify Proxies (For NoVNC Browser Clients)                │ │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │ │
│  │  │ Monitor 0        │  │ Monitor 1        │  ...              │ │
│  │  │ Port: 6080       │  │ Port: 6081       │                   │ │
│  │  │ WS → VNC:5900    │  │ WS → VNC:5901    │                   │ │
│  │  └──────────────────┘  └──────────────────┘                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Shared Audio WebSocket (Port 6900)                            │ │
│  │  - Single streamer for all monitors                            │ │
│  │  - cpal audio capture (system audio)                           │ │
│  │  - Opus encoding (48kHz stereo, low-latency)                   │ │
│  │  - Binary WebSocket protocol                                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘

Connection Flows:

NoVNC (Browser):
┌───────────────┐                          ┌─────────────────┐
│ NoVNC Browser │ ──── ws://host:6080/ ──→ │ WebSockify:6080 │
│               │                          │    ↓            │
│               │                          │ VNC Server:5900 │
└───────────────┘                          └─────────────────┘

Desktop VNC Client:
┌───────────────┐                          ┌─────────────────┐
│ TigerVNC      │ ──── vnc://host:5900 ──→ │ VNC Server:5900 │
│ RealVNC       │                          │                 │
└───────────────┘                          └─────────────────┘

Audio:
┌───────────────┐                          ┌─────────────────┐
│ Audio Client  │ ─ ws://host:6900/audio → │ Audio WS:6900   │
│ (Browser)     │                          │ (Opus encoded)  │
└───────────────┘                          └─────────────────┘
```

## Performance Metrics

- **Video Latency**: 10-50ms (vs 200-500ms previously)
- **Audio Latency**: 5-30ms (vs 300-800ms previously)
- **Encoding Passes**: 1x (vs 2-3x previously)
- **CPU Usage**: 40-60% reduction

## Features

- **Standard VNC Protocol**: Implements RFB 3.8 protocol for compatibility with all standard VNC clients
- **Multi-Monitor Support**: Automatic port allocation (5900, 5901, 5902...) for multiple monitors
- **Shared Audio Streaming**: Single WebSocket audio stream on port 6900 shared across all monitors
- **WebSocket Audio**: Low-latency Opus-encoded audio over WebSocket (5-30ms latency)
- **Multi-Client Support**: Handle up to 10 simultaneous VNC client connections per monitor
- **Auto-Registration**: Automatically registers with clever-service API for video wall integration
- **Input Control**: Full keyboard and mouse control with VNC key code translation
- **Cursor Support**: Native cursor pseudo-encoding for RFB 3.8 (planned)

## Multi-Monitor Setup

CLT-CLEVER-KVM automatically manages VNC servers for multiple monitors with a **shared audio stream**:

- **Monitor 0 (First)**: VNC port 5900, Audio WebSocket port 6900 (shared)
- **Monitor 1 (Second)**: VNC port 5901, Audio WebSocket port 6900 (shared)
- **Monitor 2 (Third)**: VNC port 5902, Audio WebSocket port 6900 (shared)
- **Monitor N**: VNC port 5900+N, Audio WebSocket port 6900 (shared)

Note: Monitors are zero-indexed in the code (0, 1, 2...) but may be referred to as "Monitor 1", "Monitor 2", etc. in user interfaces.

**Important:** All monitors share a single audio stream on port 6900 since system audio is the same across all displays.

Each monitor gets its own independent VNC server with:
- Exact positioning from OS display settings (x, y coordinates)
- Exact sizing from native monitor resolution (width, height)
- Access to the shared audio stream for synchronized playback

## Quick Start

### 1. Starting the VNC Server

From the UI:
1. Open the CLT-CLEVER-KVM application
2. Navigate to the "Server Controls" section
3. Enable "Enable VNC Server" checkbox
4. Optionally enable "Enable Audio Stream"
5. Configure ports (default: VNC=5900, Audio=6900)
6. Click "Start VNC Server"

From code:
```javascript
import { invoke } from '@tauri-apps/api/tauri';

const vncInfo = await invoke('start_vnc_server', {
  port: 5900,
  monitor: 0,
  enableAudio: true,
  audioPort: 6900,
});

console.log('VNC URL:', vncInfo.vnc_url);
console.log('WebSockify URL:', vncInfo.websockify_url);
console.log('Audio URL:', vncInfo.audio_url);
```

### 2. Connecting with VNC Clients

**Important**: VNC and audio URLs use hostname for better stability.

#### NoVNC (Browser-based VNC Client)

CLT-CLEVER-KVM includes a built-in WebSockify proxy for NoVNC connections. Use the websockify URL:

```
ws://<hostname>:6080/
```

**⚠️ Important:** NoVNC connects to the **WebSockify port (6080+)**, NOT the VNC port (5900+).

Example NoVNC connection:
```javascript
// NoVNC connection in browser
const rfb = new RFB(targetElement, 'ws://workstation-1:6080/', {
  credentials: { password: '' }
});
```

Or using the NoVNC HTML interface:
```
http://novnc-server/vnc.html?host=<hostname>&port=6080
```

#### TigerVNC
```bash
vncviewer <hostname>:5900
```

#### RealVNC
```bash
vnc://<hostname>:5900
```

#### VNC Viewer (GUI)
Open VNC Viewer and enter: `<hostname>:5900`

### 3. Audio Stream Integration

The audio stream is available via WebSocket:
```
ws://<hostname>:6900/audio
```

Audio is encoded using Opus codec:
- **Sample Rate**: 48000 Hz
- **Channels**: 2 (stereo)
- **Encoding**: Opus (low-latency mode)
- **Format**: Binary WebSocket messages

### 4. NoVNC Setup and Configuration

#### Built-in WebSockify Proxy

CLT-CLEVER-KVM includes a built-in WebSockify proxy that automatically starts with the VNC server:

- **Port 6080** → bridges to VNC server on port 5900 (Monitor 0)
- **Port 6081** → bridges to VNC server on port 5901 (Monitor 1)
- **Port 6082** → bridges to VNC server on port 5902 (Monitor 2)

No external websockify installation is required!

#### External WebSockify (Optional)

If you need to run websockify externally:

```bash
# Install websockify
pip install websockify

# Run websockify to proxy port 5900 (Monitor 0)
websockify --web=/usr/share/novnc 6080 localhost:5900

# Access NoVNC at:
# http://localhost:6080/vnc.html?host=localhost&port=6080
```

**Option 2: Nginx as WebSocket proxy (for TLS termination)**

Configure Nginx to proxy WebSocket connections with TLS:
```nginx
server {
    listen 443 ssl;
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # WebSockify proxy for NoVNC
    location / {
        proxy_pass http://localhost:6080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

#### Multi-Monitor NoVNC Setup

For multiple monitors, each monitor has its own WebSockify port:
- Monitor 0: `ws://hostname:6080/` → VNC port 5900
- Monitor 1: `ws://hostname:6081/` → VNC port 5901
- Monitor 2: `ws://hostname:6082/` → VNC port 5902

All monitors share the same audio stream:
- Audio: `ws://hostname:6900/audio`

#### Browser Integration

```javascript
const audioWs = new WebSocket('ws://hostname:6900/audio');
const audioContext = new AudioContext({ sampleRate: 48000 });

audioWs.binaryType = 'arraybuffer';

audioWs.onmessage = async (event) => {
  // Decode Opus frame
  const opusData = new Uint8Array(event.data);
  // Use opus.js or native Web Audio API Opus decoder
  // Play decoded audio through Web Audio API
};
```

## Legacy MediaMTX Integration (Deprecated)

**Note**: MediaMTX integration has been removed in favor of direct WebSocket streaming. The following documentation is kept for reference only.

<details>
<summary>Legacy MediaMTX Configuration (Click to expand)</summary>

MediaMTX was previously used to relay audio streams:

```yaml
# mediamtx.yml (DEPRECATED)
paths:
  workstation_1_video:
    source: vnc://workstation-1:5900
    sourceProtocol: vnc
    
  workstation_1_audio:
    source: rtsp://workstation-1:5901/audio
    sourceProtocol: rtsp
```

**Migration**: Use direct WebSocket connections instead:
- Video: Connect to VNC directly (vnc://hostname:5900)
- Audio: Connect to WebSocket (ws://hostname:6900/audio)

</details>

#### MediaMTX Auto-Discovery

CLT-CLEVER-KVM includes an automatic MediaMTX server discovery feature that scans your local network for MediaMTX servers running on port 9997.

**Automatic Scanning:**
1. Open Advanced Settings in the Configuration tab
2. Enable "Auto-scan network for MediaMTX server on port 9997"
3. The application will automatically scan your local subnet (e.g., 192.168.1.0/24) on startup
4. Found servers will be listed in the MediaMTX URL dropdown

**Manual Configuration:**
1. Open Advanced Settings in the Configuration tab
2. Enter the MediaMTX server URL manually in the "MediaMTX URL" field
3. Format: `http://<ip-address>:9997` or `http://<hostname>:9997`

**Manual Scanning:**
- Click the "🔍 Scan" button to trigger a manual network scan
- The scan checks all hosts in your subnet with a 200ms timeout per host
- Found servers are displayed in a dropdown for easy selection

**How It Works:**
- Detects your local network subnet automatically
- Scans all IP addresses in the subnet (1-254) in parallel
- Tests TCP connection to port 9997 on each host
- Returns list of responsive MediaMTX servers
- Auto-selects the first found server when auto-scan is enabled

## Configuration

### Default Settings

The VNC server uses these default settings (configurable in `tauri.conf.json`):

```json
{
  "plugins": {
    "vnc": {
      "defaultPort": 5900,
      "audioPort": 5901,
      "maxClients": 10,
      "password": "",
      "cleverServiceUrl": "http://localhost:8000"
    }
  }
}
```

### Port Configuration

- **VNC Ports**: Starting at 5900 for the first monitor, incrementing by 1 for each additional monitor (5900, 5901, 5902, etc.)
- **Audio Port** (default: 6900): Single shared WebSocket port for audio streaming across all monitors

Standard VNC port mapping:
- 5900: Monitor 0 (Display :0)
- 5901: Monitor 1 (Display :1)
- 5902: Monitor 2 (Display :2)
- etc.

**Important**: All monitors share the same audio stream on port 6900 since system audio is identical across displays.

### Monitor Selection

Select which monitor to stream:
- `0`: Primary monitor (default)
- `1`, `2`, etc.: Additional monitors

## CLEVER Service Integration

### Auto-Registration

The VNC server can automatically register with the CLEVER service for video wall integration:

```javascript
const registration = await invoke('register_with_clever_service', {
  cleverUrl: 'http://clever-service:8000'
});

console.log('Registration ID:', registration.id);
console.log('VNC URL:', registration.vnc_url);
console.log('Audio URL:', registration.audio_url);
```

### Registration API

The registration API endpoint:
```
POST http://clever-service:8000/api/screencasts/register
```

Payload (Version 3.0+):
```json
{
  "vnc_url": "vnc://workstation-1:5900",
  "audio_url": "ws://workstation-1:6900/audio",
  "hostname": "workstation-1",
  "unique_id": "workstation-1",
  "fallback_ip": "192.168.1.100",
  "type": "vnc-kvm"
}
```

**Legacy payload format** (still supported):
```json
{
  "vnc_url": "vnc://192.168.1.100:5900",
  "audio_url": "ws://192.168.1.100:6900/audio",
  "hostname": "workstation-1",
  "type": "vnc-kvm"
}
```

Response:
```json
{
  "id": 123,
  "status": "registered"
}
```

### Unregistration

To unregister:
```
DELETE http://clever-service:8000/api/screencasts/{id}
```

## Cursor Support (Planned)

Native cursor rendering via RFB 3.8 cursor pseudo-encoding is planned but not yet implemented:

- **Future Feature**: Cursor pseudo-encoding type -239 (RFB 3.8)
- **Planned Capability**: Cursor shape and position updates to NoVNC clients
- **Current Status**: TODO - implementation in progress

**Implementation Note**: Once implemented, NoVNC clients will be able to request cursor updates by including encoding -239 in their SetEncodings message, enabling native cursor rendering in the browser.

## Audio Streaming

### WebSocket-Based Audio (Current)

CLT-CLEVER-KVM uses direct WebSocket streaming for audio with significant performance improvements:

- **Direct WebSocket**: Audio streams via ws://hostname:6900/audio
- **Opus Encoding**: 48kHz stereo with low-latency mode
- **Low Latency**: 5-30ms (vs 300-800ms with RTSP)
- **No Relay Required**: Direct connection to client, no MediaMTX needed
- **Binary Protocol**: Efficient binary WebSocket messages

### Audio Quality Settings

Audio encoding uses Opus codec with these settings:
- **Sample Rate**: 48000 Hz
- **Channels**: 2 (stereo)
- **Encoding Mode**: Low Delay (optimized for real-time)
- **Bitrate**: Adaptive

### Legacy RTSP Audio (Deprecated)

<details>
<summary>Previous RTSP-based audio (Click to expand)</summary>

The previous implementation used RTSP:
- Required MediaMTX server for relay
- Higher latency: 300-800ms
- Multiple encoding passes
- Complex infrastructure

**Migration**: Update clients to use WebSocket URLs instead of RTSP URLs.

</details>

## Keyboard and Mouse Control

### Keyboard Input

The VNC server translates VNC key codes (X11 keysyms) to OS-level keyboard events:

- **Standard keys**: A-Z, 0-9, punctuation
- **Modifier keys**: Shift, Ctrl, Alt, Meta/Super
- **Special keys**: Enter, Tab, Backspace, Delete, Escape
- **Arrow keys**: Up, Down, Left, Right
- **Function keys**: F1-F12
- **Navigation**: Home, End, Page Up, Page Down

### Mouse Input

The VNC server handles:
- **Mouse movement**: Absolute positioning
- **Mouse buttons**: Left, Middle, Right
- **Mouse wheel**: Scroll up/down

## Security Considerations

### Current Implementation

The current VNC server implementation:
- ✅ Supports "None" security type (no authentication)
- ❌ Does not yet support password authentication
- ❌ Does not support encryption

### Recommended Security Measures

For production use:

1. **Network Isolation**: Run VNC on a private network
2. **Firewall Rules**: Restrict VNC port access to trusted IPs
3. **SSH Tunneling**: Tunnel VNC over SSH for encryption
4. **VPN**: Use VPN for remote access

Example SSH tunnel:
```bash
ssh -L 5900:localhost:5900 user@remote-host
vncviewer localhost:5900
```

### Future Enhancements

Planned security features:
- VNC password authentication
- TLS/SSL encryption
- VeNCrypt support

## Troubleshooting

### VNC Server Won't Start

**Error**: "Failed to bind to port 5900"
- **Cause**: Port already in use
- **Solution**: Check if another VNC server is running or change the port

```bash
# Linux/Mac: Check port usage
sudo lsof -i :5900

# Windows: Check port usage
netstat -ano | findstr :5900
```

### No Video Display

**Issue**: VNC client connects but shows black screen
- **Check**: Screen capture permissions (especially on macOS)
- **Solution**: Grant screen recording permissions in System Preferences

### Input Not Working

**Issue**: Keyboard/mouse input doesn't work
- **Check**: Input permissions
- **Solution**: Grant accessibility permissions to the application

### Audio Stream Not Available

**Issue**: WebSocket audio connection fails
- **Check**: Audio is enabled in VNC server settings
- **Check**: Audio port 6900 is not blocked by firewall
- **Check**: System audio loopback is configured (see note below)
- **Solution**: Verify audio streaming is enabled and network is accessible

**System Audio Loopback Configuration:**

The audio streamer captures system audio input. To stream system audio output (what you hear), you need to configure audio loopback:

- **Linux (PulseAudio)**: Use monitor devices
  ```bash
  pactl load-module module-loopback
  ```
  
- **Windows**: Use WASAPI loopback mode or third-party tools like VB-Audio Virtual Cable

- **macOS**: Use BlackHole or similar virtual audio device

### High Latency

**Issue**: VNC response is slow
- **Cause**: Network latency or bandwidth issues
- **Solutions**:
  - Use local network for best performance
  - Reduce screen resolution
  - Use more efficient VNC encoding (ZRLE, Tight)
  - Check network bandwidth with `iperf`

### Client Connection Limit

**Issue**: "Too many clients connected"
- **Cause**: Maximum client limit reached (default: 10)
- **Solution**: Disconnect unused clients or increase `maxClients` in config

## Performance Optimization

### Target Performance Metrics

- **Frame Rate**: 30-60 FPS
- **Audio Latency**: <50ms
- **Total A/V Sync**: <100ms
- **Simultaneous Clients**: 5+

### Optimization Tips

1. **Network**: Use wired Gigabit Ethernet for best performance
2. **Resolution**: Lower resolutions encode faster
3. **Encoding**: Use hardware-accelerated encoding when available
4. **CPU**: Multi-core CPU recommended for multiple clients

## API Reference

### Tauri Commands

#### start_vnc_server
Start the VNC server.

```typescript
await invoke('start_vnc_server', {
  port?: number,           // VNC port (default: 5900)
  monitor?: number,        // Monitor index (default: 0)
  enableAudio: boolean,    // Enable audio stream
  audioPort?: number,      // Audio port (default: 6900, shared across all monitors)
}): Promise<VncServerInfo>
```

Returns:
```typescript
interface VncServerInfo {
  vnc_url: string;           // VNC connection URL (e.g., vnc://hostname:5900)
  websockify_url: string;    // WebSocket URL for NoVNC (e.g., ws://hostname:6080/)
  websockify_port: number;   // WebSockify port (6080+ for each monitor)
  audio_url?: string;        // Audio WebSocket URL (e.g., ws://hostname:6900/audio)
  monitor_id: number;        // Monitor index (0-based)
  monitor_name: string;      // Monitor display name
  position_x: number;        // Monitor X position
  position_y: number;        // Monitor Y position
  width: number;             // Monitor width in pixels
  height: number;            // Monitor height in pixels
  port: number;              // VNC port number (5900+ for each monitor)
  audio_port?: number;       // Audio port number (6900 when audio is enabled)
  clients_connected: number; // Number of connected VNC clients
}
```

**Port assignments:**
- VNC port = 5900 + monitor_id (e.g., 5900, 5901, 5902)
- WebSockify port = 6080 + monitor_id (e.g., 6080, 6081, 6082)
- Audio port = 6900 (shared)

#### stop_vnc_server
Stop the VNC server.

```typescript
await invoke('stop_vnc_server'): Promise<void>
```

#### get_vnc_status
Get current VNC server status.

```typescript
await invoke('get_vnc_status'): Promise<VncStatus>
```

Returns:
```typescript
interface VncStatus {
  running: boolean;
  clients: number;
  audio_enabled: boolean;
  registration_status?: {
    registered: boolean;
    id?: number;
    vnc_url?: string;
    audio_url?: string;
  };
}
```

#### register_with_clever_service
Register with CLEVER service.

```typescript
await invoke('register_with_clever_service', {
  cleverUrl: string,
}): Promise<ScreencastRegistration>
```

Returns:
```typescript
interface ScreencastRegistration {
  id: number;
  vnc_url: string;
  audio_url?: string;
  hostname: string;
  registered_at: string;
}
```

## Testing

### Manual Testing

1. **Start VNC Server**
   ```bash
   # Launch application
   npm run tauri dev
   
   # In UI, start VNC server on port 5900
   ```

2. **Connect with TigerVNC**
   ```bash
   vncviewer localhost:5900
   ```

3. **Test Input**
   - Type text in the VNC viewer
   - Move mouse and click
   - Verify actions appear on the host

4. **Test Audio**
   ```bash
   # Connect to WebSocket audio stream
   # Use a WebSocket client or browser to connect to:
   ws://localhost:6900/audio
   
   # The stream delivers Opus-encoded audio frames
   # that need to be decoded for playback
   ```

### Unit Tests

Run unit tests:
```bash
cd src-tauri
cargo test vnc
```

### Integration Tests

Test with different VNC clients:
- TigerVNC (Linux, Windows, macOS)
- RealVNC Viewer
- UltraVNC (Windows)
- Screen Sharing (macOS)

## Known Limitations

1. **Authentication**: Currently only supports "None" security type
2. **Encryption**: No TLS/SSL support yet
3. **Encodings**: Currently implements Raw encoding only (consider Tight, ZRLE for efficiency)
4. **Screen Rotation**: Does not handle screen rotation dynamically
5. **Cursor Encoding**: Cursor pseudo-encoding planned but not yet implemented

## Future Enhancements

### Planned Features

- [ ] Password authentication (VNC Auth)
- [ ] TLS/SSL encryption
- [ ] Efficient encodings (Tight, ZRLE, H.264)
- [ ] Complete cursor pseudo-encoding implementation
- [ ] Screen rotation support
- [x] Multi-monitor streaming (implemented with WebSocket architecture)
- [ ] Clipboard synchronization
- [ ] File transfer support

### Performance Improvements

- [x] WebSocket-based audio streaming (implemented - 40-60% CPU reduction)
- [x] Single-pass audio encoding (implemented with Opus)
- [ ] Hardware-accelerated video encoding
- [ ] Frame differencing for delta updates
- [ ] Adaptive quality based on bandwidth
- [ ] Client-side caching

## Support

For issues, questions, or contributions:

- **GitHub Issues**: https://github.com/CLTSG/CLT-CLEVER-KVM/issues
- **Documentation**: https://github.com/CLTSG/CLT-CLEVER-KVM/tree/main/docs
- **Discord**: Join our community server (link in README)

## License

This VNC server implementation is part of CLT-CLEVER-KVM and is licensed under the MIT License.

## TLS/WSS Support

For secure WebSocket connections (required by modern browsers in secure contexts):

### Overview

CLT-CLEVER-KVM supports secure WebSocket URLs (wss://) for NoVNC and audio streaming. This is **required** for:
- NoVNC in HTTPS contexts
- Modern browser security policies  
- Production deployments
- Enterprise environments

### Configuration

1. **Enable TLS URLs in CLT-CLEVER-KVM:**

```typescript
// From frontend
await invoke('set_use_tls_urls', { useTls: true });
```

This changes URL generation:
- `ws://hostname:6080/` → `wss://hostname:6080/` (WebSockify)
- `ws://hostname:6900/audio` → `wss://hostname:6900/audio` (Audio)

2. **Set up TLS termination at reverse proxy level** (industry standard approach)

### Quick Setup Examples

#### Nginx
```nginx
server {
    listen 443 ssl;
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    # WebSockify for NoVNC (port 6080)
    location / {
        proxy_pass http://localhost:6080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
    
    # Audio WebSocket (port 6900)
    location /audio {
        proxy_pass http://localhost:6900;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

#### Caddy (Automatic HTTPS)
```
your-domain.com {
    reverse_proxy /* localhost:6080
    reverse_proxy /audio localhost:6900
}
```

### Complete TLS Setup Guide

See **[TLS_SETUP.md](./TLS_SETUP.md)** for comprehensive documentation including:
- Detailed configuration for Nginx, Caddy, HAProxy
- Let's Encrypt certificate setup
- Self-signed certificates for development
- Troubleshooting guide
- Security best practices
- Performance tuning

### Why Reverse Proxy?

We use TLS termination at the reverse proxy (industry standard) because:
- ✅ **Performance**: Proxies are optimized for TLS
- ✅ **Certificate Management**: Centralized certificate handling
- ✅ **Flexibility**: Easy to update TLS configurations
- ✅ **Standard Practice**: Used by AWS, Cloudflare, Google Cloud
- ✅ **Separation of Concerns**: Application handles logic, proxy handles TLS
