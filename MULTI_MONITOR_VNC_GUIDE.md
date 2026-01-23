# Multi-Monitor VNC Setup Guide

## Overview

CLT-CLEVER-KVM now provides native multi-monitor VNC support with automatic port assignment and centralized audio streaming. Each monitor gets its own VNC server with exact positioning and sizing preserved from your system display settings.

## Port Assignment

The system automatically assigns VNC ports based on monitor index:

| Monitor | VNC Port | Display |
|---------|----------|---------|
| Monitor 1 (Primary) | 5900 | Screen 1 |
| Monitor 2 | 5901 | Screen 2 |
| Monitor 3 | 5902 | Screen 3 |
| Monitor N | 5900 + (N-1) | Screen N |
| **Audio** | **6900** | **Shared** |

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

### Using TigerVNC

```bash
# Monitor 1
vncviewer <ip-address>:5900

# Monitor 2
vncviewer <ip-address>:5901

# Monitor 3
vncviewer <ip-address>:5902
```

### Using RealVNC

```bash
# Monitor 1
vnc://<ip-address>:5900

# Monitor 2
vnc://<ip-address>:5901
```

## Audio Streaming

Audio is available on port **6900** and is shared across all monitors:

```
rtsp://<ip-address>:6900/audio
```

### Example with VLC

```bash
vlc rtsp://<ip-address>:6900/audio
```

## MediaMTX Integration

Configure MediaMTX to relay VNC streams and audio:

```yaml
paths:
  # Monitor 1
  vnc_screen1:
    source: vnc://192.168.1.100:5900
    sourceProtocol: vnc
    
  # Monitor 2
  vnc_screen2:
    source: vnc://192.168.1.100:5901
    sourceProtocol: vnc
    
  # Monitor 3
  vnc_screen3:
    source: vnc://192.168.1.100:5902
    sourceProtocol: vnc
    
  # Shared audio
  audio_stream:
    source: rtsp://192.168.1.100:6900/audio
    sourceProtocol: rtsp
```

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
      "vnc_url": "vnc://192.168.1.100:5900",
      "audio_url": "rtsp://192.168.1.100:6900/audio",
      "port": 5900,
      "audio_port": 6900,
      "clients_connected": 0,
      "monitor_id": 0,
      "monitor_name": "Monitor 1",
      "width": 1920,
      "height": 1080,
      "position_x": 0,
      "position_y": 0
    },
    {
      "vnc_url": "vnc://192.168.1.100:5901",
      "audio_url": null,
      "port": 5901,
      "audio_port": null,
      "clients_connected": 0,
      "monitor_id": 1,
      "monitor_name": "Monitor 2",
      "width": 1920,
      "height": 1080,
      "position_x": 1920,
      "position_y": 0
    }
  ],
  "audio_url": "rtsp://192.168.1.100:6900/audio"
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

Allow incoming connections on VNC and audio ports:

```bash
# Ubuntu/Debian
sudo ufw allow 5900:5910/tcp  # VNC ports for up to 10 monitors
sudo ufw allow 6900/tcp       # Audio port

# CentOS/RHEL
sudo firewall-cmd --permanent --add-port=5900-5910/tcp
sudo firewall-cmd --permanent --add-port=6900/tcp
sudo firewall-cmd --reload
```

### SSH Tunneling (Secure Remote Access)

```bash
# Forward all VNC ports and audio
ssh -L 5900:localhost:5900 \
    -L 5901:localhost:5901 \
    -L 5902:localhost:5902 \
    -L 6900:localhost:6900 \
    user@remote-host
```

## Troubleshooting

### VNC Servers Don't Start

1. Check if ports are already in use:
   ```bash
   netstat -tuln | grep "5900\|5901\|5902\|6900"
   ```

2. Check application logs for errors
3. Ensure monitors are properly detected:
   ```javascript
   await invoke('get_monitors');
   ```

### Audio Not Working

1. Verify audio port (6900) is not blocked by firewall
2. Check if audio stream is running
3. Test with VLC or other RTSP client

### Incorrect Monitor Positioning

The system uses the OS-reported monitor positions. Verify your display settings are correct in your OS.

## Best Practices

1. **Network Isolation**: Use on trusted networks or with VPN/SSH tunnel
2. **Monitor Detection**: Let the system auto-detect all monitors for consistent port assignment
3. **Audio Sharing**: Only the first monitor's VNC server handles audio to avoid conflicts
4. **Client Management**: Monitor client connections through the status API
5. **Resource Limits**: Each VNC server supports up to 10 concurrent clients

## Example: 3-Monitor Video Wall Setup

```
Physical Layout:
┌─────────┐ ┌─────────┐ ┌─────────┐
│Monitor 1│ │Monitor 2│ │Monitor 3│
│  :5900  │ │  :5901  │ │  :5902  │
└─────────┘ └─────────┘ └─────────┘
         Audio: :6900 (shared)

Position Data:
Monitor 1: (0, 0)       1920x1080
Monitor 2: (1920, 0)    1920x1080  
Monitor 3: (3840, 0)    1920x1080
```

Connect each display to the corresponding VNC port, and all will have synchronized audio from port 6900.

## Support

For issues or questions, please refer to:
- [VNC Integration Guide](docs/VNC_INTEGRATION.md)
- [Implementation Summary](IMPLEMENTATION_SUMMARY.md)
- [Security Summary](SECURITY_SUMMARY.md)
