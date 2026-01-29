# Multi-Monitor VNC Setup Guide

## Overview

CLT-CLEVER-KVM provides native multi-monitor VNC support with automatic port assignment and centralized audio streaming. Each monitor gets its own VNC server with exact positioning and sizing preserved from your system display settings.

## Port Assignment

The system automatically assigns ports based on monitor index:

| Monitor | VNC Port | WebSockify Port | Description |
|---------|----------|-----------------|-------------|
| Monitor 0 (Primary) | 5900 | 6080 | Primary display |
| Monitor 1 | 5901 | 6081 | Secondary display |
| Monitor 2 | 5902 | 6082 | Third display |
| Monitor N | 5900 + N | 6080 + N | Additional displays |
| **Audio** | **N/A** | **6900** | **Shared WebSocket** |

**Key Points:**
- **VNC ports (5900+)**: For desktop VNC clients (TigerVNC, RealVNC, etc.)
- **WebSockify ports (6080+)**: For NoVNC browser clients
- **Audio port (6900)**: WebSocket audio stream (Opus encoded, shared for all monitors)

## Quick Start

### 1. Install Dependencies

```bash
npm install
```

### 2. Start the Application

```bash
npm run tauri dev
```

### 3. Start VNC Servers

#### Option A: Start All Monitors (Recommended)

Use the `start_vnc_servers_all` command from the UI or API to automatically start VNC servers for all detected monitors.

#### Option B: Start Individual Monitors

Start VNC for specific monitors using the `start_vnc_server` command with the monitor index.

## Connecting to VNC Servers

### Desktop VNC Clients (TigerVNC, RealVNC)

Connect to the **VNC port** (5900+):

```bash
# Monitor 0
vncviewer <hostname>:5900

# Monitor 1
vncviewer <hostname>:5901

# Monitor 2
vncviewer <hostname>:5902
```

### NoVNC (Browser-based)

Connect to the **WebSockify port** (6080+):

```
# Monitor 0 - use WebSockify port, NOT VNC port!
ws://<hostname>:6080/

# Monitor 1
ws://<hostname>:6081/

# Monitor 2
ws://<hostname>:6082/
```

**⚠️ Important:** NoVNC connects to WebSockify (6080+), not directly to VNC (5900+).

## Audio Streaming

Audio is available via WebSocket on port **6900** and is shared across all monitors:

```
ws://<hostname>:6900/audio
```

### Audio Details
- **Protocol**: WebSocket (binary)
- **Encoding**: Opus (48kHz stereo)
- **Latency**: 5-30ms

## MediaMTX Integration

Configure MediaMTX to relay VNC streams using hostname:

```yaml
paths:
  # Monitor 0
  vnc_screen0:
    source: vnc://workstation-1:5900
    sourceProtocol: vnc
    
  # Monitor 1
  vnc_screen1:
    source: vnc://workstation-1:5901
    sourceProtocol: vnc
    
  # Monitor 2
  vnc_screen2:
    source: vnc://workstation-1:5902
    sourceProtocol: vnc
```

**Note**: Audio streaming uses WebSocket (ws://hostname:6900/audio) instead of RTSP.

## Display Positioning

Each VNC server preserves the exact monitor configuration from your system:

- **Position**: X and Y coordinates match system display settings
- **Size**: Width and height match monitor resolution
- **Primary Flag**: Primary monitor is correctly identified

This ensures that video wall systems can reconstruct the exact multi-monitor layout.

## API Commands

### Start All VNC Servers

```javascript
await invoke('start_vnc_servers_all', { 
  enable_audio: true 
});
```

Response:
```json
{
  "servers": [
    {
      "vnc_url": "vnc://workstation-1:5900",
      "websockify_url": "ws://workstation-1:6080/",
      "audio_url": "ws://workstation-1:6900/audio",
      "port": 5900,
      "websockify_port": 6080,
      "audio_port": 6900,
      "clients_connected": 0,
      "monitor_id": 0,
      "monitor_name": "Monitor 0",
      "width": 1920,
      "height": 1080,
      "position_x": 0,
      "position_y": 0,
      "hostname": "workstation-1"
    },
    {
      "vnc_url": "vnc://workstation-1:5901",
      "websockify_url": "ws://workstation-1:6081/",
      "audio_url": null,
      "port": 5901,
      "websockify_port": 6081,
      "audio_port": null,
      "clients_connected": 0,
      "monitor_id": 1,
      "monitor_name": "Monitor 1",
      "width": 1920,
      "height": 1080,
      "position_x": 1920,
      "position_y": 0,
      "hostname": "workstation-1"
    }
  ],
  "audio_url": "ws://workstation-1:6900/audio"
}
```

### Start Single VNC Server

```javascript
await invoke('start_vnc_server', {
  port: 5900,
  monitor: 0,
  enable_audio: true,
  audio_port: 6900
});
```

### Stop All VNC Servers

```javascript
await invoke('stop_vnc_server');
```

### Get VNC Status

```javascript
await invoke('get_vnc_status');
```

Response:
```json
{
  "running": true,
  "clients": 2,
  "audio_enabled": true,
  "registration_status": null
}
```

## Network Configuration

### Firewall Rules

Allow incoming connections on all required ports:

```bash
# Ubuntu/Debian
sudo ufw allow 5900:5910/tcp   # VNC ports for up to 10 monitors
sudo ufw allow 6080:6090/tcp   # WebSockify ports for NoVNC
sudo ufw allow 6900/tcp        # Audio WebSocket port

# CentOS/RHEL
sudo firewall-cmd --permanent --add-port=5900-5910/tcp
sudo firewall-cmd --permanent --add-port=6080-6090/tcp
sudo firewall-cmd --permanent --add-port=6900/tcp
sudo firewall-cmd --reload
```

### SSH Tunneling (Secure Remote Access)

```bash
# Forward all VNC, WebSockify, and audio ports
ssh -L 5900:localhost:5900 \
    -L 5901:localhost:5901 \
    -L 6080:localhost:6080 \
    -L 6081:localhost:6081 \
    -L 6900:localhost:6900 \
    user@remote-host
```

## Troubleshooting

### VNC Servers Don't Start

1. Check if ports are already in use:
   ```bash
   netstat -tuln | grep "5900\|5901\|6080\|6081\|6900"
   ```

2. Check application logs for errors
3. Ensure monitors are properly detected:
   ```javascript
   await invoke('get_monitors');
   ```

### NoVNC Can't Connect

1. Verify you're connecting to the **WebSockify port (6080+)**, NOT the VNC port (5900+)
2. Use the URL format: `ws://hostname:6080/`
3. Check that WebSockify ports are not blocked by firewall

### Audio Not Working

1. Verify audio port (6900) is not blocked by firewall
2. Check if audio stream is running
3. Connect to audio via WebSocket: `ws://hostname:6900/audio`

### Incorrect Monitor Positioning

The system uses the OS-reported monitor positions. Verify your display settings are correct in your OS.

## Best Practices

1. **Network Isolation**: Use on trusted networks or with VPN/SSH tunnel
2. **Monitor Detection**: Let the system auto-detect all monitors for consistent port assignment
3. **Audio Sharing**: All monitors share a single audio stream on port 6900
4. **Client Management**: Monitor client connections through the status API
5. **Resource Limits**: Each VNC server supports up to 10 concurrent clients
6. **NoVNC Users**: Always use WebSockify port (6080+), not VNC port (5900+)

## Example: 3-Monitor Video Wall Setup

```
Physical Layout:
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  Monitor 0  │ │  Monitor 1  │ │  Monitor 2  │
│ VNC: 5900   │ │ VNC: 5901   │ │ VNC: 5902   │
│ WS:  6080   │ │ WS:  6081   │ │ WS:  6082   │
└─────────────┘ └─────────────┘ └─────────────┘
              Audio: 6900 (shared)

Connection URLs:
Desktop VNC:    vnc://hostname:5900, :5901, :5902
NoVNC Browser:  ws://hostname:6080/, :6081/, :6082/
Audio:          ws://hostname:6900/audio

Position Data:
Monitor 0: (0, 0)       1920x1080
Monitor 1: (1920, 0)    1920x1080  
Monitor 2: (3840, 0)    1920x1080
```

## Support

For issues or questions, please refer to:
- [VNC Integration Guide](docs/VNC_INTEGRATION.md)
- [Main README](README.md)
