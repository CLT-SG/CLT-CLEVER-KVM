# Replace H.264/Relay Architecture with RDEngine VP9 Streaming and HTTPS

## [5.0.6] Persistent TLS Certificates and Infinite WebSocket Reconnection

### Problem

1. The self-signed TLS certificate was regenerated on every server restart, causing browsers to silently reject the new wss:// certificate and fail WebSocket connections with close code 1006.
2. WebSocket reconnection stopped permanently after 5 failed attempts, leaving users with a dead screen requiring a manual page reload.
3. The `reconnectAttempts` counter was never incremented in the `ws.onclose` handler, so the retry count was meaningless.
4. No guard against overlapping `setTimeout` reconnect timers -- multiple reconnect attempts could race and create duplicate WebSocket connections.
5. Old WebSocket event handlers were not nulled before closing, allowing stale `onmessage`/`onerror` callbacks to fire on the old socket after a new connection was created.
6. VPX decoder was left in a stale state after reconnection (closed decoder instance, wrong `needsKeyframe` flag), causing decode errors on the new stream.
7. Pending `VideoFrame` references from the previous connection were never closed, leaking GPU memory.
8. The canvas displayed a frozen last frame during reconnection instead of clearing to black, making it unclear whether the stream was live or disconnected.
9. The status display was inside the OSD overlay, so reconnection messages were only visible when hovering over the video area.

### Solution

1. Added `load_or_generate_cert()` that persists the TLS certificate and private key to `~/.local/share/clever-kvm/server.crt` and `server.key`, reusing them across restarts and only regenerating if the files are missing or corrupt.
2. Changed `maxReconnectAttempts` from 5 to `Infinity` with exponential backoff (1-10 second cap). After 30 consecutive failures, falls back to a full page reload.
3. Added `cleanupConnection()` method that nulls all WebSocket event handlers before closing, cancels pending animation frames, closes pending VPX frames, and clears host cursor state.
4. Added `_cancelPendingReconnect()` to clear any existing reconnect timer before scheduling a new one, preventing overlapping timers.
5. Added `resetDecoderState()` that destroys and reinitializes both VPX and H264 decoders on each reconnection, ensuring a clean decoder state.
6. Added `clearCanvasToBlack()` that fills the canvas with black and stops the video element on disconnect.
7. Moved `.status-display` from inside `.osd-overlay` to directly inside `#screen` with `z-index: 200`, so reconnection status is always visible regardless of hover state.
8. Reordered status display layout: loading spinner above title text, followed by description and troubleshooting tips.
9. Added troubleshooting tips that appear during reconnection states with common fixes (check server, refresh page, check network, check firewall).

### Changes Made

#### Modified Files - Backend
- **src-tauri/src/network/server/server.rs**: Added `get_cert_dir()` returning `~/.local/share/clever-kvm/`, added `load_or_generate_cert()` that checks for existing cert/key files before generating new ones, persists generated cert and key to disk, `WebSocketServer::new()` now calls `load_or_generate_cert()` instead of `generate_self_signed_cert()`

#### Modified Files - Frontend
- **src-tauri/web-client/kvm-client.js**: `maxReconnectAttempts` changed from 5 to `Infinity`, added `_reconnectTimer`, `_isReconnecting`, `_consecutiveFailures`, `_maxConsecutiveFailuresBeforeReload` (30) state, added `cleanupConnection()` with proper WebSocket teardown, added `resetDecoderState()` for decoder destroy/reinit, added `_cancelPendingReconnect()` and `_scheduleReconnect()` with exponential backoff, added `clearCanvasToBlack()`, updated `connect()` with duplicate-call guard and cleanup before new connection, updated `updateStatus()` to populate troubleshooting tips, removed attempt counts from all status messages
- **src-tauri/web-client/kvm-template.html**: Moved `.status-display` div from inside `.osd-overlay` to directly inside `#screen`, reordered children to spinner then h2 then p then `.status-tips` div
- **src-tauri/web-client/kvm-client.css**: Updated `.status-display` with `z-index: 200` and `pointer-events: none`, added flexbox column layout with CSS `order` properties for element ordering, added `.status-tips` styles with bullet list formatting and `:empty { display: none }` for auto-hide, adjusted spinner size to 36px

### Testing

- Verified cargo check passes with 0 errors
- Verified JavaScript syntax check passes
- TLS certificate persists across server restarts in ~/.local/share/clever-kvm/
- WebSocket reconnects indefinitely with exponential backoff (1-10s)
- Canvas clears to black immediately on disconnect
- Status display is always visible during reconnection (not hidden behind OSD hover)
- Troubleshooting tips appear during reconnection states
- Page reloads automatically after 30 consecutive failures as last resort