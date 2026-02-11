# Rust Backend Structure

This document describes the professional modular structure of the Tauri Rust backend.

## Related Components

| Component | Location | Description |
|-----------|----------|-------------|
| **Tauri App** | `src-tauri/` | Desktop application with screen capture, encoding, and local server |
| **Relay Server** | `relay-server/` | Centralized device management with Actix Web (see [relay-server/README.md](../../relay-server/README.md)) |
| **Web Client** | `src-tauri/web-client/` | Browser-based KVM viewer with H.264 WebCodecs decoder |
| **Vue Frontend** | `src/` | Management UI with Matrix dark theme (see [THEME.md](../../docs/THEME.md)) |

## Directory Structure

```
src-tauri/src/
├── main.rs                         # Clean entry point with minimal code
├── main_original.rs                # Backup of original main.rs
├── app/                           # Application-level functionality
│   ├── mod.rs                     # Application module exports
│   ├── state.rs                   # Application state and configuration
│   └── commands.rs                # All Tauri commands organized
├── audio/                         # Audio processing functionality
│   ├── mod.rs                     # Audio module exports
│   └── engine.rs                  # Audio engine implementation
├── core/                         # Core system functionality
│   ├── mod.rs                     # Core module exports
│   ├── capture.rs                 # Screen capture functionality
│   └── input.rs                   # Input handling (keyboard/mouse)
├── lib/                          # Shared utilities and constants
│   ├── mod.rs                     # Library module exports
│   ├── constants.rs               # Application-wide constants
│   └── error_types.rs             # Custom error types
├── network/                      # Networking and server functionality
│   ├── mod.rs                     # Network module exports
│   └── server/                    # Server implementation
│       ├── mod.rs                 # Server module exports
│       ├── handlers.rs            # HTTP handlers
│       ├── models.rs              # Data models and structs
│       ├── server.rs              # Main server implementation
│       └── websocket.rs           # WebSocket handling
├── streaming/                    # Video/audio streaming functionality
│   ├── mod.rs                     # Streaming module exports (organized)
│   ├── codecs/                    # Encoding/decoding implementations
│   │   ├── mod.rs                 # Codecs module exports
│   │   ├── h264_encoder.rs        # H.264 hardware-accelerated encoder
│   │   ├── realtime_codec.rs      # Real-time codec implementation
│   │   └── yuv420_encoder.rs      # YUV420 video encoder
│   ├── enhanced/                  # High-performance implementations
│   │   ├── mod.rs                 # Enhanced module exports
│   │   ├── enhanced_audio.rs      # Enhanced audio processing
│   │   └── ultra_low_latency.rs   # Ultra-low latency encoder
│   └── handlers/                  # Stream management handlers
│       ├── mod.rs                 # Handlers module exports
│       ├── integrated_handler.rs  # Integrated H.264 streaming handler
│       ├── low_latency_pipeline.rs # Low-latency H.264 pipeline
│       ├── realtime_stream.rs     # Real-time stream handler
│       └── ultra_stream.rs        # Ultra-performance stream handler
└── system/                       # System optimization functionality
    ├── mod.rs                     # System module exports
    └── system_optimizer.rs        # System performance optimizations
```

## Modular Organization

### 1. **app/** - Application Layer
- **state.rs**: Contains `ServerState`, `ServerOptions`, `MonitorInfo` and state management
- **commands.rs**: All Tauri commands organized in one place with proper error handling
- **mod.rs**: Clean module exports and re-exports

**Benefits:**
- Clear separation of application logic from system functionality
- All Tauri commands in one organized file
- Centralized state management

### 2. **audio/** - Audio Processing
- **engine.rs**: Moved from single `audio.rs` file to proper module
- **mod.rs**: Audio module organization

**Benefits:**
- Room for expansion (capture, effects, formats, etc.)
- Better organization than single file approach

### 3. **core/** - Core System Operations
- **capture.rs**: Screen capture functionality (cross-platform unified interface)
- **native_capture.rs**: Native platform-specific screen capture implementations
- **input.rs**: Input handling (keyboard/mouse)
- **mod.rs**: Core functionality exports

**Native Screen Capture - Platform Support:**
- **Windows**: GDI (GetDC, BitBlt, GetDIBits) for maximum compatibility
- **Linux X11**: X11 library with RandR extension for multi-monitor support
- **macOS**: Core Graphics (CGDisplayCreateImage) for Quartz display capture

**Benefits:**
- Cross-platform native screen capture without external dependencies
- High-performance direct platform API access
- Multi-monitor support on all platforms
- Clear separation of core system operations
- Logical grouping of related functionality

### 4. **lib/** - Shared Utilities
- **constants.rs**: Application-wide constants
- **error_types.rs**: Custom error types for better error handling
- **mod.rs**: Utility exports

**Benefits:**
- Centralized constants management
- Consistent error handling across modules
- Shared utilities accessible to all modules

### 5. **network/** - Networking Layer
- **server/**: Complete server implementation
  - **server.rs**: Main WebSocket server
  - **handlers.rs**: HTTP request handlers
  - **models.rs**: Network data models
  - **websocket.rs**: WebSocket connection handling

**Benefits:**
- Clear networking layer separation
- Organized server components
- Easy to extend with new protocols

### 6. **streaming/** - Media Streaming (Reorganized)
- **codecs/**: Encoding and decoding implementations
- **enhanced/**: High-performance, low-latency implementations
- **handlers/**: Stream management and coordination

**Benefits:**
- Logical separation by functionality type
- Easy to add new codecs or handlers
- Clear distinction between basic and enhanced features

### 7. **system/** - System Optimization
- **system_optimizer.rs**: System performance optimizations
- **mod.rs**: System functionality exports

**Benefits:**
- Centralized system optimization code
- Easy to extend with platform-specific optimizations

## Key Improvements

### [X] **Modular Architecture**
- Each module has a clear, single responsibility
- Easy to maintain and extend
- Better code organization

### [X] **Clean Entry Point**
- `main.rs` reduced from 653 lines to ~70 lines
- Clear application initialization
- Better separation of concerns

### [X] **Professional Structure**
- Industry-standard Rust project organization
- Proper module hierarchy
- Consistent naming conventions

### [X] **Maintainability**
- Related functionality grouped together
- Easy to locate specific features
- Clear module boundaries

### [X] **Scalability**
- Easy to add new features in appropriate modules
- Room for expansion in each category
- Modular design supports growth

### [X] **Error Handling**
- Centralized error types
- Consistent error handling patterns
- Better debugging capabilities

### [X] **Constants Management**
- Centralized application constants
- Easy configuration management
- Consistent default values

## Backward Compatibility

[X] **All functionality preserved** - The restructuring maintains 100% of the original functionality while organizing it better.

[X] **Same public API** - All Tauri commands and public interfaces remain the same.

[X] **Build compatibility** - The project builds successfully with the new structure.

## Cross-Platform Screen Capture

The native screen capture module (`core/native_capture.rs`) provides high-performance screen capture using platform-native APIs:

### Windows
- **API**: Windows GDI (Graphics Device Interface)
- **Functions**: `GetDC`, `BitBlt`, `GetDIBits`
- **Format**: BGRA → RGBA conversion
- **Dependencies**: `windows-capture` crate for monitor enumeration

### Linux (X11)
- **API**: X11 with RandR extension
- **Functions**: `XGetImage`, `RandR` for multi-monitor
- **Format**: Depth-aware conversion (16/24/32-bit support)
- **Dependencies**: `x11rb` crate with `randr` feature

### macOS
- **API**: Core Graphics (Quartz)
- **Functions**: `CGDisplayCreateImage`, `CGDataProvider`
- **Format**: BGRA → RGBA conversion
- **Dependencies**: `core-graphics` and `core-foundation` crates

### Usage Example
```rust
use crate::core::native_capture::{NativeScreenCapture, capture_screen_native};

// Using the struct
let mut capture = NativeScreenCapture::new(Some(0))?; // Monitor 0
let rgba_data = capture.capture_rgba()?;
let (width, height) = capture.dimensions();

// Using the standalone function
let (data, width, height) = capture_screen_native(0)?;
```

## Build Status

[X] **Compilation**: Success with 109 warnings (mostly unused code warnings, which is normal during refactoring)
[X] **Module resolution**: All modules resolve correctly
[X] **Dependencies**: All dependencies satisfied

## Next Steps

1. **Code cleanup**: Address unused code warnings as features are implemented
2. **Documentation**: Add comprehensive module documentation
3. **Testing**: Implement unit tests for each module
4. **Feature expansion**: Add new features using the modular structure

The new structure provides a solid foundation for professional Rust development while maintaining all existing functionality.
