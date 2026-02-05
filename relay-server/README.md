# CLEVER KVM Relay Server v2.0

A high-performance relay server for CLEVER KVM devices with web dashboard, device discovery, and low-latency video streaming.

## Technology Stack

| Component | Technology | Purpose |
|-----------|------------|----------|
| **HTTP Server** | Actix Web 4 | High-performance async web framework |
| **WebSocket** | actix-ws | Real-time bidirectional communication |
| **Templating** | Tera | Jinja2-style HTML templates |
| **Static Files** | actix-files | CSS, JS, and asset serving |
| **mDNS** | mdns-sd | Service discovery on local network |
| **Async Runtime** | Tokio | Multi-threaded async I/O |

## Features

- **Web Dashboard**: Modern web interface at `http://{hostname}.local:8881/` showing all connected devices
- **Tera Templates**: Professional HTML templates with inheritance and dynamic rendering
- **mDNS Service Discovery**: Automatic discovery by Tauri KVM apps using `.local` hostnames
- **Device Registry**: Manages connected devices with heartbeat monitoring
- **WebSocket Relay**: Relays video streams from devices to viewers
- **REST API**: Device registration, status, and management endpoints
- **Static File Serving**: CSS, JavaScript, and assets served via `/static/`
- **UDP Relay** (optional): Low-latency UDP-based streaming for advanced use

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     RELAY SERVER                                │
│                  (Linux/macOS/Windows)                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │   Web Dashboard │  │   Device        │  │   mDNS        │  │
│  │   (HTTP)        │  │   Registry      │  │   Discovery   │  │
│  │   Port 8881     │  │                 │  │               │  │
│  └────────┬────────┘  └────────┬────────┘  └───────────────┘  │
│           │                    │                               │
│           ▼                    ▼                               │
│  ┌────────────────────────────────────────────────────────┐   │
│  │              WebSocket Relay                           │   │
│  │   - Device connections (/ws/device/{hostname})         │   │
│  │   - Viewer connections (/ws/viewer/{hostname})         │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
         │                                         │
         │ Video Frames                            │ Video Frames
         │ + Heartbeats                            │ + Input Events
         ▼                                         ▼
┌─────────────────┐                      ┌─────────────────┐
│  Tauri KVM App  │                      │  Web Browser    │
│  (Device)       │                      │  (Viewer)       │
│                 │                      │                 │
│  - Captures     │                      │  - Views        │
│  - Encodes      │                      │  - Controls     │
│  - Publishes    │                      │                 │
└─────────────────┘                      └─────────────────┘
```

## Project Structure

```
relay-server/
├── Cargo.toml              # Rust dependencies
├── src/
│   ├── main.rs             # Entry point with Actix runtime
│   ├── http_server.rs      # Actix Web routes and handlers
│   ├── ws_relay.rs         # WebSocket relay logic
│   ├── device.rs           # Device registry and models
│   ├── discovery.rs        # mDNS service discovery
│   ├── peer.rs             # Peer/session management
│   ├── protocol.rs         # Binary protocol definitions
│   └── relay.rs            # Core relay server logic
├── templates/
│   ├── base.html           # Base template (header, footer, styles)
│   ├── dashboard.html      # Device dashboard (extends base)
│   ├── kvm_client.html     # KVM viewer page (extends base)
│   └── error.html          # Error pages (extends base)
└── static/
    ├── kvm-client.css      # KVM client styles
    ├── kvm-client.js       # KVM client JavaScript
    └── h264-decoder.js     # H.264 WebCodecs decoder
```

### Template System

The relay server uses **Tera** templates with a base template inheritance pattern:

```html
{# templates/base.html - Base template #}
<!DOCTYPE html>
<html>
<head>
    <title>{% block title %}CLEVER KVM{% endblock %}</title>
    {% block head %}{% endblock %}
</head>
<body>
    {% block content %}{% endblock %}
    {% block scripts %}{% endblock %}
</body>
</html>

{# templates/dashboard.html - Extends base #}
{% extends "base.html" %}
{% block title %}Dashboard - CLEVER KVM{% endblock %}
{% block content %}
    <h1>Connected Devices</h1>
    {% for device in devices %}
        <div class="device-card">{{ device.display_name }}</div>
    {% endfor %}
{% endblock %}
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

## Usage

```bash
# Start relay server with HTTP (8881) and HTTPS (8443)
./clever-relay

# Custom ports with verbose logging
./clever-relay --port 9000 --https-port 9443 --verbose

# Disable HTTPS (not recommended - WebCodecs won't work)
./clever-relay --https false

# Use custom TLS certificates
./clever-relay --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem

# Full options
./clever-relay \
    --bind 0.0.0.0 \
    --port 8881 \
    --https-port 8443 \
    --https true \
    --mdns true \
    --verbose
```

## Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `-p, --port` | 8881 | HTTP/WebSocket port |
| `--https-port` | 8443 | HTTPS port (for WebCodecs API support) |
| `-b, --bind` | 0.0.0.0 | Address to bind to |
| `--https` | true | Enable HTTPS server (required for WebCodecs in browsers) |
| `--tls-cert` | - | Path to TLS certificate file (PEM format). Self-signed if not provided |
| `--tls-key` | - | Path to TLS private key file (PEM format). Self-signed if not provided |
| `--mdns` | true | Enable mDNS service advertisement |
| `--enable-udp` | false | Enable UDP relay on port 9922 |
| `-v, --verbose` | false | Enable debug logging |

## HTTPS / TLS Support

The relay server includes built-in HTTPS support, which is **required** for the WebCodecs API to work in browsers. WebCodecs provides hardware-accelerated H.264 video decoding for smooth, low-latency video playback.

### How It Works

- **Automatic Certificate Generation**: If no certificate is provided, the server generates a self-signed certificate automatically
- **Certificate Storage**: Self-signed certificates are saved to the local data directory for reuse across restarts
- **Dual Server**: Both HTTP (port 8881) and HTTPS (port 8443) servers run simultaneously

### Using Self-Signed Certificates

When you first access the HTTPS URL, your browser will show a security warning. To proceed:

1. **Chrome/Edge**: Click "Advanced" → "Proceed to {hostname} (unsafe)"
2. **Firefox**: Click "Advanced..." → "Accept the Risk and Continue"
3. **Safari**: Click "Show Details" → "visit this website"

This warning appears because the certificate is self-signed and not issued by a trusted Certificate Authority. For local network use, this is safe to accept.

### Using Your Own Certificates

For production or to avoid browser warnings, provide your own certificates:

```bash
./clever-relay --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem
```

### WebCodecs API

When accessing the KVM client via HTTPS (`https://{relay}.local:8443/kvm?hostname={device}`):

- **Hardware Acceleration**: WebCodecs API decodes H.264 video using GPU hardware
- **Lower Latency**: Typically <5ms decode time vs >20ms for software decoding
- **Lower CPU Usage**: GPU handles video decoding, freeing CPU for other tasks
- **Better Quality**: Higher frame rates and smoother playback

When accessing via HTTP, the client falls back to software decoding, which still works but may have higher latency and CPU usage.

## Web Dashboard

Access the dashboard at:
- **HTTP**: `http://{hostname}.local:8881/`
- **HTTPS**: `https://{hostname}.local:8443/` (recommended for WebCodecs)

### Features

- **Device List**: Shows all connected KVM devices with status
- **Auto-Refresh**: Updates device list every 5 seconds
- **Quick Connect**: Click on a device to open the KVM viewer
- **Device Info**: Displays hostname, capabilities, viewer count, and stream config

## KVM Client

Access a device's KVM at:
- **HTTP**: `http://{relay}.local:8881/kvm?hostname={device}` (software decoding)
- **HTTPS**: `https://{relay}.local:8443/kvm?hostname={device}` (hardware-accelerated)

### Features

- **H.264 Hardware Decoding**: Uses WebCodecs API for smooth playback (HTTPS required)
- **Low Latency**: WebSocket-based frame delivery
- **Input Control**: Mouse and keyboard passthrough
- **Adaptive Quality**: Auto-adjusts based on network conditions
- **Automatic Fallback**: Falls back to software decoding on HTTP connections

## REST API

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/devices` | List all connected devices |
| GET | `/api/devices/{hostname}` | Get device details |
| POST | `/api/register` | Register a device |
| POST | `/api/heartbeat` | Device heartbeat |
| POST | `/api/unregister` | Unregister a device |
| GET | `/api/health` | Server health check |

### Register Device

```bash
curl -X POST http://relay.local:8881/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "my-device",
    "display_name": "My KVM Device",
    "capabilities": {
      "supports_h264": true,
      "supports_audio": true,
      "max_width": 1920,
      "max_height": 1080,
      "max_fps": 60
    }
  }'
```

### List Devices

```bash
curl http://relay.local:8881/api/devices
```

Response:
```json
{
  "devices": [
    {
      "hostname": "device1",
      "display_name": "Living Room PC",
      "state": "streaming",
      "viewer_count": 1,
      "capabilities": {...}
    }
  ],
  "total": 1
}
```

## WebSocket Protocol

### Device Connection

Connect to `/ws/device/{hostname}` after REST registration.

**Messages from Device:**
- Binary: H.264 video frames
- JSON: `{"type": "stream_config", ...}` - Stream configuration

**Messages to Device:**
- JSON: `{"type": "input", ...}` - Input events from viewers

### Viewer Connection

Connect to `/ws/viewer/{hostname}` to view a device.

**Messages to Viewer:**
- Binary: H.264 video frames
- JSON: `{"type": "config", ...}` - Stream configuration

**Messages from Viewer:**
- JSON: `{"type": "input", ...}` - Mouse/keyboard input

## mDNS Service

The relay server advertises itself via mDNS:

- **Service Type**: `_clever-kvm._tcp.local.`
- **Instance Name**: `clever-relay-{hostname}`
- **TXT Records**:
  - `version=2.0.0`
  - `port=8881`
  - `protocol=websocket`

### Discovery (Tauri App)

```rust
// Tauri apps auto-discover relay servers
let relays = RelayClient::discover_relays(Duration::from_secs(3)).await?;
for relay in relays {
    println!("Found: {} at {}:{}", relay.hostname, relay.address, relay.port);
}
```

## Performance

- **Latency**: <10ms end-to-end (local network)
- **Bandwidth**: 5-20 Mbps depending on quality
- **Concurrent Viewers**: Tested with 10+ viewers per device
- **Memory**: ~50MB baseline, +10MB per active stream

## Troubleshooting

### mDNS Not Working

1. Ensure mDNS/Bonjour is enabled on your network
2. Check firewall allows UDP port 5353
3. On Linux, install `avahi-daemon`

### High Latency

1. Reduce video quality in Tauri app settings
2. Use wired network connection
3. Check for network congestion

### Device Not Appearing

1. Verify device is registered: `curl http://relay:8881/api/devices`
2. Check device heartbeat is active (every 10s)
3. Review relay server logs with `--verbose`

## License

MIT License - See LICENSE file for details
