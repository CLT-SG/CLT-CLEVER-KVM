# CLEVER KVM UDP Relay Server

A high-performance UDP relay server for low-latency video streaming in the CLEVER KVM system.

## Features

- **UDP-based transport** for minimal latency (<5ms overhead)
- **Room-based** sessions for multiple concurrent KVM connections
- **NAT traversal** support with peer discovery
- **Optional LZ4 compression** for bandwidth optimization
- **Session management** with automatic cleanup
- **Scalable architecture** using Tokio async runtime

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

## Usage

```bash
# Start server with default settings (port 9922)
./clever-relay

# Custom port
./clever-relay --port 8888

# Enable compression and verbose logging
./clever-relay --compress --verbose

# Full options
./clever-relay \
    --bind 0.0.0.0 \
    --port 9922 \
    --compress \
    --max-clients 50 \
    --timeout 60 \
    --verbose
```

## Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `-p, --port` | 9922 | UDP port to listen on |
| `-b, --bind` | 0.0.0.0 | Address to bind to |
| `-c, --compress` | false | Enable LZ4 compression |
| `-m, --max-clients` | 100 | Maximum clients per room |
| `-t, --timeout` | 60 | Session timeout in seconds |
| `-v, --verbose` | false | Enable debug logging |

## Protocol

The relay server uses a custom binary protocol optimized for low-latency video streaming.

### Packet Header (12 bytes)

```
+--------+--------+--------+--------+
| Magic (4 bytes: "CKVM")           |
+--------+--------+--------+--------+
| Version| Type   | Flags  | Reserved|
+--------+--------+--------+--------+
| Sequence Number (4 bytes)         |
+--------+--------+--------+--------+
| Payload...                        |
```

### Packet Types

| Type | Code | Description |
|------|------|-------------|
| Register | 0x01 | Client registration |
| RegisterAck | 0x02 | Registration acknowledgment |
| Ping | 0x03 | Heartbeat/keepalive |
| Pong | 0x04 | Heartbeat response |
| VideoFrame | 0x10 | Video frame data |
| AudioFrame | 0x11 | Audio frame data |
| InputEvent | 0x20 | Mouse/keyboard input |
| DiscoverPeers | 0x30 | Peer discovery request |
| PeerList | 0x31 | Peer list response |
| Disconnect | 0x40 | Disconnect notification |

### Client Registration

Send a JSON payload after the packet header:

```json
{
    "client_id": "unique-client-id",
    "role": "Host",  // or "Viewer"
    "room": "room-name",
    "capabilities": {
        "supports_compression": true,
        "supports_h264": true,
        "supports_audio": true,
        "max_width": 1920,
        "max_height": 1080
    }
}
```

## Architecture

```
+----------+     UDP      +--------------+     UDP      +----------+
|   Host   | -----------> | Relay Server | -----------> |  Viewer  |
| (Video)  |              |              |              | (Display)|
+----------+              +--------------+              +----------+
     ^                          |                            |
     |                          |                            |
     +----------- Input Events (relayed) -------------------+
```

## Performance

Target latencies:
- Packet routing: <1ms
- Total relay overhead: <5ms
- Video end-to-end: <20ms (with proper encoder settings)

## Integration with CLEVER KVM

The relay server can be used as an alternative to WebSocket streaming for scenarios requiring lower latency. Configure the KVM client to use UDP transport mode and point it to the relay server address.

## License

MIT
