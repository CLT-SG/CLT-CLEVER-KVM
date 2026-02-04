# Changelog

All notable changes to this project will be documented in this file.

## [4.1.0] - 2026-02-04

### Fixed
- Fixed Windows screen capture compatibility issues caused by breaking API changes in windows-capture 1.5.0
- Resolved build failures due to scap/zed-scap dependency conflicts
- Fixed KVM web client black screen issue by correcting default screen dimensions
- Fixed mouse cursor alignment between client and actual screen coordinates
- Fixed mouse and keyboard input events not being processed by the server
- Fixed RGBA frame data copying to prevent ArrayBuffer reuse issues
- Fixed cursor visibility in CSS for remote control interface

### Changed
- Replaced scap screen capture library with native Windows GDI implementation for maximum stability
- Updated Cargo.toml to use zed-scap 0.0.8-zed with pinned windows-capture 1.4.4
- Refactored capture.rs to use native capture backend
- Updated all streaming codecs and handlers to use the new native capture module
- Simplified fallback capture to use RGBA format for better client compatibility
- Reduced Debian package dependencies to only require ffmpeg

### Added
- New native_capture.rs module with direct Windows GDI API implementation
- Native monitor enumeration support
- Improved error handling for screen capture operations
- Cross-platform native screen capture support for Linux X11 and macOS
- Linux X11 capture using x11rb library with RandR extension for multi-monitor support
- macOS capture using Core Graphics (CGDisplayCreateImage) API
- Platform-specific dependencies: x11rb for Linux, core-graphics/core-foundation for macOS
- Connection health monitoring with automatic stream freeze detection
- Automatic reconnection when connection becomes stale
- Input event parsing for mouse and keyboard events in streaming handlers
- Thread-safe input handler using Arc<parking_lot::Mutex>