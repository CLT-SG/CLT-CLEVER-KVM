# Video Quality, Latency & Input Fixes

This document details the fixes applied to the RDEngine streaming pipeline to resolve low video quality, high latency, and broken keyboard/mouse input on the web client.

---

## 1. Video Quality & Latency Fixes

### Encoder Configuration (`codec.rs`)

| Setting | Before | After | Impact |
|---|---|---|---|
| Default bitrate | 2000 kbps | 4000 kbps | 2x sharper image for 1080p screen content |
| `rc_max_quantizer` | 56 | 40 | Less aggressive compression — sharper text and edges |
| `rc_min_quantizer` | 4 | 2 | Allows higher quality when bandwidth is available |
| `rc_buf_sz` | 600 ms | 150 ms | ~4x lower encoder buffering latency |
| `rc_buf_initial_sz` | 400 ms | 100 ms | Faster first-frame delivery |
| `rc_buf_optimal_sz` | 500 ms | 120 ms | Tighter rate control response |
| `rc_dropframe_thresh` | _(not set)_ | 0 | Never drop frames — prefer lower quality over stuttering |
| `cpu_speed` | 7 | 6 | Better encode quality with minimal CPU cost increase |
| Base bitrate for resolution calc | 2073 kbps | 4000 kbps | Higher quality baseline for all resolutions |
| Minimum bitrate floor | 400 kbps | 800 kbps | Prevents quality from degrading too far |

### QoS Defaults (`qos.rs`)

| Setting | Before | After | Impact |
|---|---|---|---|
| `min_bitrate_kbps` | 400 | 800 | Floor quality stays usable even under congestion |

### Default Configs (`video_service.rs`, `connection.rs`)

| Setting | Before | After | Impact |
|---|---|---|---|
| `VideoServiceConfig.bitrate_kbps` | 2000 | 4000 | New connections start at a good quality level |
| `ConnectionConfig.bitrate_kbps` | 2000 | 4000 | Consistent with encoder defaults |

### Critical Bug Fix: `set_bitrate()` (`codec.rs`)

The `set_bitrate()` function had a **critical bug** that was silently corrupting the encoder on every QoS bitrate adjustment:

**Before (broken):**
```rust
fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
    // BUG: Gets a fresh default config, which resets dimensions, threading,
    // CBR mode, buffer sizes — everything — to libvpx defaults.
    // Only the bitrate was set, so the encoder was effectively destroyed
    // and rebuilt with wrong settings on every QoS adjustment.
    let mut enc_cfg = vpx_codec_enc_config_default(...);
    enc_cfg.rc_target_bitrate = bitrate_kbps;
    vpx_codec_enc_config_set(&mut self.ctx, &enc_cfg); // Corrupts encoder
}
```

**After (fixed):**
```rust
fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
    // Get default config, then re-apply ALL custom settings on top,
    // only changing the bitrate. This preserves dimensions, threading,
    // CBR mode, buffer sizes, keyframe config, error resilience, etc.
    let mut enc_cfg = vpx_codec_enc_config_default(...);
    enc_cfg.g_w = self.config.width;
    enc_cfg.g_h = self.config.height;
    enc_cfg.g_threads = self.config.threads;
    enc_cfg.rc_target_bitrate = bitrate_kbps;
    // ... all other settings preserved ...
    vpx_codec_enc_config_set(&mut self.ctx, &enc_cfg);
}
```

### QoS-to-Encoder Pipeline Fix (`video_service.rs`)

QoS bitrate adjustments were **never applied to the encoder**. The QoS controller would calculate a new bitrate, but the encoder continued using its initial value.

**Added** in the video service main loop:
```rust
let current_bitrate = qos.lock().bitrate_kbps();
if current_bitrate != last_applied_bitrate {
    encoder.set_bitrate(current_bitrate);
    last_applied_bitrate = current_bitrate;
}
```

---

## 2. Keyboard & Mouse Input Fixes

### Root Cause

The web client and the Rust server had a **complete type mismatch** on every input message field. All input events were failing silent JSON deserialization on the server — no mouse clicks, no key presses, no scroll events were ever processed.

### Field Mismatches Fixed (`protocol.rs`)

| Field | Server Expected | Client Sent | Fix |
|---|---|---|---|
| `button` (mouse) | `u8` (0, 1, 2) | `"left"`, `"right"`, `"middle"` | Accept `serde_json::Value` — parse both string and number via `parse_button_value()` |
| `monitor_id` | `Option<usize>` | `"primary"` (string) | Accept `Option<serde_json::Value>` — handles both string and number |
| `code` (keyboard) | `String` (required) | Not sent | Made `Option<String>` |
| `key_code` (keyboard) | Not accepted | `keyCode: 65` | Added `#[serde(alias = "keyCode")]` field |
| Modifier keys | `Option<KeyModifiers>` struct | `ctrlKey: true`, `altKey: false` | Added individual boolean fields with `#[serde(alias = "ctrlKey")]` etc. |
| `delta_x`/`delta_y` (wheel) | Required | Sometimes missing | Added `#[serde(default)]` |
| `x`/`y` (wheel) | Not accepted | Sent by client | Added as `Option<f64>` |

### Client-Side Fixes (`kvm-client.js`)

| Change | Before | After |
|---|---|---|
| Mouse button format | `button: "left"` (string) | `button: e.button` (numeric 0/1/2) |
| Keyboard `code` field | Not sent | `code: e.code` (e.g., `"KeyA"`, `"ArrowUp"`) |
| Ping interval | 5000 ms | 2000 ms (faster QoS feedback) |

### Error Logging (`connection.rs`)

**Before:** Input parsing failures were silently ignored — if `serde_json::from_str::<InputMsg>(text)` returned `Err`, nothing was logged.

**After:** Failed parses now log a warning with the error message and the first 200 characters of the raw message:
```rust
Err(e) => {
    warn!("Failed to parse message as control or input: {} | raw: {}", e, &text[..text.len().min(200)]);
}
```

---

## Files Changed

| File | Changes |
|---|---|
| `src-tauri/src/rdengine/codec.rs` | Encoder defaults, buffer sizes, `set_bitrate()` bug fix, preset configs |
| `src-tauri/src/rdengine/video_service.rs` | Default bitrate, QoS-to-encoder pipeline |
| `src-tauri/src/rdengine/connection.rs` | Default bitrate, input parsing rewrite, error logging, quality presets |
| `src-tauri/src/rdengine/qos.rs` | Minimum bitrate floor |
| `src-tauri/src/rdengine/protocol.rs` | `InputMsg` enum rewrite for flexible field types |
| `src-tauri/web-client/kvm-client.js` | Mouse button format, keyboard `code` field, ping interval |
