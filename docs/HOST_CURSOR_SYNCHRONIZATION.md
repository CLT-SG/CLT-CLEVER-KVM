# Host Cursor Synchronization

This document describes the host cursor synchronization system that tracks the remote host's cursor position and renders it as a visual overlay on the web client, replacing the static crosshair with a live cursor indicator.

---

## Overview

The KVM web client previously displayed a generic crosshair cursor over the remote screen, providing no visibility into where the host's cursor was actually located. The cursor synchronization feature adds:

1. **Real-time cursor position tracking** — polls the host OS cursor at ~30–60 Hz
2. **Visual cursor overlay** — renders an SVG arrow with a "Host" label on the web client
3. **Host control priority** — suppresses client input while the host is actively moving the cursor
4. **Client activity awareness** — hides host cursor overlay when the web client user is moving their own mouse; overlay only appears when the remote host moves the cursor
5. **Cursor shape mapping** — 23 shape types (pointer, text, resize, wait, etc.) mapped to CSS equivalents
6. **Delta compression** — only transmits updates when position or shape changes

---

## Architecture

```
┌──────────────────────────────────┐
│         Host (Rust/Tauri)        │
│                                  │
│  CursorService thread            │
│  ┌──────────────────────┐        │
│  │ X11 QueryPointer     │ ~33ms  │
│  │ poll loop             │───────│
│  └──────┬───────────────┘        │
│         │ crossbeam channel      │
│         ▼                        │
│  Connection handler (tokio)      │
│  ┌──────────────────────┐        │
│  │ bridge task:          │        │
│  │ crossbeam → tokio mpsc│        │
│  └──────┬───────────────┘        │
│         │ tokio::select!         │
│         ▼                        │
│  WebSocket binary send           │
│  MSG_CURSOR (0x03)               │
└──────────┬───────────────────────┘
           │ binary WebSocket
           ▼
┌──────────────────────────────────┐
│       Web Client (JS)            │
│                                  │
│  handleCursorMessage()           │
│  ┌──────────────────────┐        │
│  │ Parse binary payload  │        │
│  │ Update hostCursor     │        │
│  │ Detect movement       │        │
│  └──────┬───────────────┘        │
│         │                        │
│    ┌────┴────┐                   │
│    ▼         ▼                   │
│  setHost   renderHost            │
│  Controlling() Cursor()          │
│  (input     (SVG overlay         │
│   priority)  positioning)        │
└──────────────────────────────────┘
```

---

## Binary Protocol

### MSG_CURSOR (0x03) — 15 bytes total

| Offset | Size  | Field         | Encoding       | Description                        |
|--------|-------|---------------|----------------|------------------------------------|
| 0      | 1 B   | `type`        | `0x03`         | Message type identifier            |
| 1      | 4 B   | `payload_len` | u32 LE         | Always `10` (4+4+1+1)             |
| 5      | 4 B   | `x`           | i32 LE         | Cursor X in screen coordinates     |
| 9      | 4 B   | `y`           | i32 LE         | Cursor Y in screen coordinates     |
| 13     | 1 B   | `shape`       | u8             | Cursor shape ID (see table below)  |
| 14     | 1 B   | `visible`     | u8 (0 or 1)    | Whether the cursor is visible      |

### Cursor Shape IDs

| ID  | Shape        | CSS Cursor     | ID  | Shape         | CSS Cursor     |
|-----|--------------|----------------|-----|---------------|----------------|
| 0   | Default      | `default`      | 12  | ResizeNE      | `ne-resize`    |
| 1   | Pointer      | `pointer`      | 13  | ResizeNW      | `nw-resize`    |
| 2   | Text         | `text`         | 14  | ResizeSE      | `se-resize`    |
| 3   | Wait         | `wait`         | 15  | ResizeSW      | `sw-resize`    |
| 4   | Crosshair    | `crosshair`    | 16  | ResizeEW      | `ew-resize`    |
| 5   | Move         | `move`         | 17  | ResizeNS      | `ns-resize`    |
| 6   | NotAllowed   | `not-allowed`  | 18  | ResizeNESW    | `nesw-resize`  |
| 7   | Help         | `help`         | 19  | ResizeNWSE    | `nwse-resize`  |
| 8   | ResizeN      | `n-resize`     | 20  | Grab          | `grab`         |
| 9   | ResizeS      | `s-resize`     | 21  | Grabbing      | `grabbing`     |
| 10  | ResizeE      | `e-resize`     | 22  | Progress      | `progress`     |
| 11  | ResizeW      | `w-resize`     | 255 | Hidden        | `none`         |

---

## Rust Implementation

### Files

| File | Role |
|------|------|
| `src-tauri/src/rdengine/cursor_service.rs` | Cursor tracking service (new) |
| `src-tauri/src/rdengine/mod.rs` | Module registration |
| `src-tauri/src/rdengine/connection.rs` | Integration into WebSocket pipeline |

### CursorService

The `CursorService` struct manages a dedicated OS thread that polls cursor state:

```rust
pub struct CursorService {
    thread: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    cursor_rx: Receiver<CursorUpdate>,
}
```

- **Thread model**: Dedicated thread (not async) — avoids blocking the tokio runtime during X11 syscalls
- **Channel**: `crossbeam_channel::bounded(8)` — backpressure with non-blocking `try_send`, drops updates if the client is slow
- **Polling**: Configurable interval via `CursorServiceConfig.poll_interval_ms` (default 16ms / ~60 Hz)
- **Delta compression**: Only sends a message when `(x, y, shape)` differs from the previous poll

### CursorServiceConfig

```rust
pub struct CursorServiceConfig {
    pub poll_interval_ms: u32,   // Default: 16 (~60 Hz)
    pub screen_width: u32,       // For bounds context
    pub screen_height: u32,      // For bounds context
    pub monitor_id: usize,       // X11 screen index
}
```

### Platform Cursor Reader

| Platform | Implementation | Details |
|----------|---------------|---------|
| Linux    | `LinuxCursorReader` | X11 `QueryPointer` via `x11rb` crate |
| Other    | `FallbackCursorReader` | Stub returning `(0, 0, Default, true)` |

The Linux reader connects to the X11 display, queries the root window pointer position, and returns screen-absolute coordinates. Cursor shape detection currently defaults to `Default` — full XFixes-based shape reading can be added as a future enhancement.

### Connection Integration

In `connection.rs`, the cursor service is integrated into the existing `tokio::select!` loop alongside video, audio, and control streams:

```
tokio::select! {
    // Priority order in select!:
    video  => send video frame
    audio  => send audio frame
    cursor => send cursor update    // ← new arm
    ctrl   => handle control message
}
```

A bridge task converts crossbeam channel receives into tokio mpsc sends (same pattern used for video/audio):

```rust
// Bridge: crossbeam → tokio mpsc
let cursor_rx = cursor_service.cursor_rx().clone();
tokio::task::spawn_blocking(move || {
    while let Ok(update) = cursor_rx.recv() {
        let _ = cursor_bridge_tx.blocking_send(update);
    }
});
```

---

## Web Client Implementation

### Files

| File | Role |
|------|------|
| `src-tauri/web-client/kvm-client.js` | Binary parsing, state management, DOM rendering |
| `src-tauri/web-client/kvm-client.css` | Cursor overlay styles, shape animations |

### State Management

```javascript
this.hostCursor = {
    x: 0, y: 0,
    shape: 'default',
    visible: true,
    lastUpdate: 0,
    isHostControlling: false,
};
this.hostControlTimeoutMs = 500; // ms of host inactivity before client regains control

// Client activity tracking
this.clientActive = false;
this.clientActiveTimer = null;
this.clientActiveTimeoutMs = 300; // ms of client inactivity before host cursor can reappear
```

### Binary Message Parsing

`handleCursorMessage(binaryData)` parses the MSG_CURSOR binary format:

```javascript
const view = new DataView(binaryData);
const x       = view.getInt32(5, true);   // offset 5, little-endian
const y       = view.getInt32(9, true);   // offset 9, little-endian
const shapeId = view.getUint8(13);        // offset 13
const visible = view.getUint8(14) !== 0;  // offset 14
```

Shape IDs are mapped using a static array (`KVMClient.CURSOR_SHAPE_MAP`) indexed by the shape byte, with `255` mapping to `'none'`.

### Message Dispatch

In `handleBinaryVideoFrame()`, incoming binary messages are dispatched by their first byte:

| First Byte | Handler |
|------------|---------|
| `0x01`     | `handleRdEngineVideoFrame()` |
| `0x03`     | `handleCursorMessage()` |

### Host Control Priority

When the host cursor moves, `setHostControlling(true)` is called:

1. **Hides the client cursor** — `cursor: none` on `#screen`, `#real-canvas`, `#video-screen`
2. **Suppresses client mouse input** — `handleMouseEvent()` returns early for non-scroll events while `isHostControlling` is true (unless the client is actively moving)
3. **Auto-expires after 500ms** — a timeout restores `cursor: default` and re-enables client input

This prevents the two cursors from "fighting" when both users move their mouse simultaneously, giving the host priority.

### Client Activity Awareness

When the web client user moves their mouse, `setClientActive(true)` is called:

1. **Hides the host cursor overlay** — the overlay fades out via CSS `opacity` transition
2. **Restores the client's native cursor** — overrides any host-control cursor hiding
3. **Client input is never blocked** — when `clientActive` is true, mouse events are always processed regardless of host-control state
4. **Auto-expires after 300ms** — when the client stops moving, `clientActive` resets to false, and if the host is still sending cursor updates, the overlay reappears

This ensures the host cursor overlay only appears when the **remote host** is moving the cursor, not when the client user is interacting with the web page.

### Cursor Overlay Rendering

`renderHostCursor()` creates and updates a `#host-cursor-overlay` div:

```html
<div id="host-cursor-overlay" data-shape="default">
    <svg width="20" height="20" viewBox="0 0 24 24" class="host-cursor-svg">
        <path d="M5 3l14 8-6.5 1.5L11 19z" fill="rgba(0,0,0,0.85)"
              stroke="white" stroke-width="1.5" stroke-linejoin="round"/>
    </svg>
    <span class="host-cursor-label">Host</span>
</div>
```

**Coordinate mapping** converts host screen coordinates to CSS pixel positions:

```javascript
const content = this.getContentRect(targetElement);
const cssX = content.left + (hostCursor.x / screenWidth) * content.width;
const cssY = content.top  + (hostCursor.y / screenHeight) * content.height;
```

This correctly handles letterboxing/pillarboxing when the remote screen aspect ratio differs from the client viewport.

---

## CSS Styles

### Base Overlay

```css
#host-cursor-overlay {
    position: absolute;
    z-index: 50;
    pointer-events: none;         /* clicks pass through */
    transform: translate(-2px, -2px); /* align arrow tip */
    transition: left 30ms linear, top 30ms linear;
    will-change: left, top;
}
```

### Shape-Specific Variants

| Shape | Visual |
|-------|--------|
| Default | Black SVG arrow with white stroke + "Host" badge |
| Text (`data-shape="text"`) | SVG hidden, "I" text beam shown via `::before` pseudo-element |
| Pointer (`data-shape="pointer"`) | SVG path fill changes to `rgba(0,0,0,0.7)` |
| Wait / Progress | SVG spins via `@keyframes cursor-spin` animation |

---

## Configuration

### Tunable Parameters

| Parameter | Location | Default | Description |
|-----------|----------|---------|-------------|
| `poll_interval_ms` | `CursorServiceConfig` (Rust) | 16 ms | Cursor polling frequency (~60 Hz) |
| `hostControlTimeoutMs` | `KVMClient` constructor (JS) | 500 ms | Host inactivity timeout before client regains control |
| `clientActiveTimeoutMs` | `KVMClient` constructor (JS) | 300 ms | Client inactivity timeout before host cursor can reappear |
| Channel buffer size | `CursorService::start()` | 8 | Bounded crossbeam channel capacity |

### Adjusting Poll Rate

Higher poll rates (lower interval) provide smoother cursor tracking but increase CPU and network usage. For most KVM use cases, 16–33ms (30–60 Hz) is sufficient:

| Interval | Rate | Network (approx.) | Use Case |
|----------|------|--------------------|----------|
| 16 ms    | 60 Hz | ~900 msg/s max    | Low-latency interactive |
| 33 ms    | 30 Hz | ~450 msg/s max    | Standard KVM |
| 50 ms    | 20 Hz | ~300 msg/s max    | Bandwidth-constrained |

Note: delta compression means actual message rate is typically much lower — messages are only sent when the cursor position changes.

---

## Platform Support

| Platform | Position Tracking | Shape Detection | Notes |
|----------|------------------|-----------------|-------|
| Linux (X11) | ✅ Full | ⚠️ Default only | Full shape detection requires XFixes extension (future enhancement) |
| Linux (Wayland) | ❌ | ❌ | X11 fallback may work via XWayland |
| macOS | ⚠️ Stub | ⚠️ Stub | FallbackCursorReader — needs CGEvent-based implementation |
| Windows | ⚠️ Stub | ⚠️ Stub | FallbackCursorReader — needs GetCursorPos/GetCursorInfo implementation |

---

## Future Enhancements

1. **XFixes cursor shape detection** — use `XFixesGetCursorImage` to read the actual cursor pixmap and identify the shape more accurately
2. **Custom cursor bitmaps** — send the raw cursor image data for pixel-perfect reproduction on the client
3. **Wayland support** — implement cursor tracking via `wlr-export-dmabuf` or portal APIs
4. **macOS / Windows implementations** — platform-specific cursor readers using native APIs
5. **Configurable host control timeout** — expose `hostControlTimeoutMs` as a user-facing setting
