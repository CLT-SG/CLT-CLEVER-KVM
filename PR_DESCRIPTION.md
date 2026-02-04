# Fix Windows Screen Capture Compatibility Issues

## Problem

The application failed to compile and run on Windows due to breaking API changes between the `scap`/`zed-scap` screen capture library and `windows-capture 1.5.0`. The original dependencies had incompatible version requirements causing build failures and runtime crashes when attempting screen capture.

## Solution

Replaced the problematic `scap`/`zed-scap` dependency with a native Windows GDI screen capture implementation. This provides:

- Stable screen capture using direct Windows API calls (GDI)
- Maximum compatibility across all Windows versions
- No external dependency conflicts
- Reliable RGBA frame output for streaming

## Changes Made

### Core Changes
- **src-tauri/Cargo.toml**: Replaced `scap` with `zed-scap 0.0.8-zed` and pinned `windows-capture` to version `1.4.4` to avoid breaking changes
- **src-tauri/src/core/native_capture.rs**: New native Windows GDI screen capture module with direct Windows API implementation
- **src-tauri/src/core/capture.rs**: Refactored to use the new native capture backend instead of scap
- **src-tauri/src/core/mod.rs**: Added native_capture module export

### Streaming Updates
- **src-tauri/src/streaming/codecs/realtime_codec.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/codecs/yuv420_encoder.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/enhanced/ultra_low_latency.rs**: Updated to use native ScreenCapture
- **src-tauri/src/streaming/handlers/integrated_handler.rs**: Updated monitor enumeration to use native capture
- **src-tauri/src/streaming/handlers/ultra_stream.rs**: Simplified fallback capture to RGBA format

### Configuration
- **src-tauri/tauri.conf.json**: Simplified Debian package dependencies

## Testing

- Verified screen capture works correctly on Windows
- Confirmed RGBA frame output is correctly formatted
- Tested streaming functionality with the web client