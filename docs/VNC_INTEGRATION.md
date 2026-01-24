# VNC Server Integration Guide

## Overview

CLT-CLEVER-KVM includes a native VNC (Virtual Network Computing) server implementation that enables seamless integration with the CLEVER video wall ecosystem. The VNC server supports standard RFB 3.8 protocol and includes audio streaming capabilities.

## Features

- **Standard VNC Protocol**: Implements RFB 3.8 protocol for compatibility with all standard VNC clients
- **Multi-Client Support**: Handle up to 10 simultaneous VNC client connections
- **Separate Audio Streaming**: Audio streams via RTSP for compatibility with MediaMTX
- **Auto-Registration**: Automatically registers with clever-service API for video wall integration
- **Input Control**: Full keyboard and mouse control with VNC key code translation
- **Monitor Selection**: Choose which monitor to stream via VNC

## Architecture

### VNC Server Components

```
┌─────────────────────────────────────────────────┐
│           CLT-CLEVER-KVM Application            │
├─────────────────────────────────────────────────┤
│  VNC Server (RFB 3.8)     │  Audio Stream       │
│  - Screen Capture         │  - RTSP Server      │
│  - Input Handling         │  - Opus Encoding    │
│  - Client Management      │                     │
├─────────────────────────────────────────────────┤
│         clever-service Registration             │
└─────────────────────────────────────────────────┘
           │                        │
           │ vnc://hostname:5900   │ rtsp://hostname:5901/audio
           ▼                        ▼
    ┌──────────────┐        ┌──────────────┐
    │ VNC Clients  │        │   MediaMTX   │
    │ - TigerVNC   │        │              │
    │ - RealVNC    │        │  Audio Path  │
    │ - VNC Viewer │        └──────────────┘
    └──────────────┘
```

**Note**: As of version 3.0, URLs use hostname instead of IP addresses for improved stability and integration with video wall systems.

## Quick Start

### 1. Starting the VNC Server

From the UI:
1. Open the CLT-CLEVER-KVM application
2. Navigate to the "Server Controls" section
3. Enable "Enable VNC Server" checkbox
4. Optionally enable "Enable Audio Stream"
5. Configure ports (default: VNC=5900, Audio=5901)
6. Click "Start VNC Server"

From code:
```javascript
import { invoke } from '@tauri-apps/api/tauri';

const vncInfo = await invoke('start_vnc_server', {
  port: 5900,
  monitor: 0,
  enableAudio: true,
  audioPort: 5901,
});

console.log('VNC URL:', vncInfo.vnc_url);
console.log('Audio URL:', vncInfo.audio_url);
```

### 2. Connecting with VNC Clients

**Important**: As of version 3.0, VNC and audio URLs use hostname instead of IP addresses. This provides better stability and integration with video wall systems.

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

**Note**: You can also use IP addresses for backward compatibility, but hostname-based URLs are recommended.

### 3. Audio Stream Integration

The audio stream is available via RTSP using hostname:
```
rtsp://<hostname>:5901/audio
```

#### MediaMTX Configuration

Configure MediaMTX to relay the audio stream using hostname:

```yaml
# mediamtx.yml
paths:
  workstation_1_video:
    source: vnc://workstation-1:5900
    sourceProtocol: vnc
    
  workstation_1_audio:
    source: rtsp://workstation-1:5901/audio
    sourceProtocol: rtsp
```

**Legacy IP-based configuration** (still supported):
```yaml
paths:
  workstation_1_video:
    source: vnc://192.168.1.100:5900
    sourceProtocol: vnc
    
  workstation_1_audio:
    source: rtsp://192.168.1.100:5901/audio
    sourceProtocol: rtsp
```

Then access the combined stream:
```
rtsp://mediamtx:8554/workstation_1_video
rtsp://mediamtx:8554/workstation_1_audio
```

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

- **VNC Port** (default: 5900): TCP port for VNC connections
- **Audio Port** (default: 5901): TCP port for RTSP audio stream

Standard VNC ports:
- 5900: Display :0
- 5901: Display :1
- 5902: Display :2
- etc.

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
  "audio_url": "rtsp://workstation-1:5901/audio",
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
  "audio_url": "rtsp://192.168.1.100:5901/audio",
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

## Audio Streaming

### Dual Audio Approach

CLT-CLEVER-KVM implements two audio streaming approaches:

#### 1. Separate Audio Stream (Primary)
- Audio streams separately via RTSP
- Compatible with MediaMTX
- Works with any VNC client
- Better separation of concerns

#### 2. RFB Audio Extension (Optional)
- Audio embedded in VNC connection
- Requires compatible clients (rfbproxy + noVNC)
- Better A/V synchronization
- Not yet fully implemented

### Audio Quality Settings

Audio encoding uses Opus codec with these default settings:
- **Sample Rate**: 48000 Hz
- **Channels**: 2 (stereo)
- **Bitrate**: 96 kbps (low latency mode)

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

**Issue**: RTSP audio URL returns 404
- **Check**: Audio is enabled in VNC server settings
- **Check**: Audio port is not blocked by firewall
- **Solution**: Verify audio_port is set and audio stream is started

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
  audioPort?: number,      // Audio port (default: 5901)
}): Promise<VncServerInfo>
```

Returns:
```typescript
interface VncServerInfo {
  vnc_url: string;
  audio_url?: string;
  port: number;
  audio_port?: number;
  clients_connected: number;
}
```

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
   # Play audio stream with VLC
   vlc rtsp://localhost:5901/audio
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
3. **Audio Sync**: Separate audio stream may have slight sync issues
4. **Encodings**: Currently implements Raw encoding only (inefficient)
5. **Screen Rotation**: Does not handle screen rotation dynamically

## Future Enhancements

### Planned Features

- [ ] Password authentication (VNC Auth)
- [ ] TLS/SSL encryption
- [ ] Efficient encodings (Tight, ZRLE, H.264)
- [ ] RFB audio extension implementation
- [ ] Screen rotation support
- [ ] Multi-monitor streaming
- [ ] Clipboard synchronization
- [ ] File transfer support

### Performance Improvements

- [ ] Hardware-accelerated encoding
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
