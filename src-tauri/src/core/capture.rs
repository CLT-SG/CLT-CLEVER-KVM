//! Screen Capture Module
//!
//! Provides stable screen capture functionality using native platform APIs for maximum
//! stability and reliability across all operating systems.
//!
//! Platform Support:
//! - **Windows**: GDI (GetDC, BitBlt, GetDIBits) for universal Windows compatibility
//! - **Linux**: X11 (XGetImage) for X Window System support
//! - **macOS**: Core Graphics (CGDisplayCreateImage) for Quartz display capture
//!
//! This module provides a unified interface for screen capture that works consistently
//! across Windows, Linux (X11), and macOS platforms.

use log::{debug, error, info, warn};
use std::sync::Mutex;

use super::native_capture::{NativeScreenCapture, NativeCaptureError, NativeMonitorInfo};

/// Screen tile for delta encoding optimization
#[derive(Clone)]
pub struct ScreenTile {
    pub data: Vec<u8>,
    pub hash: u64,
    pub changed: bool,
}

/// Monitor information structure
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub is_primary: bool,
    pub width: usize,
    pub height: usize,
    pub position_x: i32,
    pub position_y: i32,
    pub scale_factor: f64,
    pub rotation: i32,
    pub supports_cursor: bool,
    pub supports_highlight: bool,
}

/// Output format for captured frames
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    RGBA,
    BGRA,
    YUV420,
}

/// YUV converter for format conversion
pub struct YuvConverter {
    width: usize,
    height: usize,
}

impl YuvConverter {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    /// Convert RGBA to YUV420 planar format
    pub fn rgba_to_yuv420(&self, rgba_data: &[u8]) -> Vec<u8> {
        let pixel_count = self.width * self.height;
        let uv_size = pixel_count / 4;
        let mut yuv_data = Vec::with_capacity(pixel_count + uv_size * 2);

        // Y plane (full resolution)
        for chunk in rgba_data.chunks_exact(4) {
            let r = chunk[0] as f32;
            let g = chunk[1] as f32;
            let b = chunk[2] as f32;

            // ITU-R BT.709 conversion for better screen content
            let y = (0.2126 * r + 0.7152 * g + 0.0722 * b).clamp(0.0, 255.0) as u8;
            yuv_data.push(y);
        }

        // U and V planes (subsampled 4:2:0)
        for y in (0..self.height).step_by(2) {
            for x in (0..self.width).step_by(2) {
                let idx = (y * self.width + x) * 4;
                if idx + 4 <= rgba_data.len() {
                    let r = rgba_data[idx] as f32;
                    let g = rgba_data[idx + 1] as f32;
                    let b = rgba_data[idx + 2] as f32;

                    let u = (-0.1146 * r - 0.3854 * g + 0.5 * b + 128.0).clamp(0.0, 255.0) as u8;
                    let v = (0.5 * r - 0.4542 * g - 0.0458 * b + 128.0).clamp(0.0, 255.0) as u8;

                    yuv_data.push(u);
                    yuv_data.push(v);
                }
            }
        }

        yuv_data
    }
}

/// Main screen capture structure using native Windows APIs
pub struct ScreenCapture {
    /// Native capture instance (primary capture method)
    native_capture: NativeScreenCapture,

    /// Frame dimensions
    width: usize,
    height: usize,

    /// Tile-based encoding support
    tile_size: usize,
    tiles: Vec<ScreenTile>,

    /// Previous frame for delta encoding
    previous_frame: Option<Vec<u8>>,

    /// Adaptive quality level (1-100)
    adaptive_quality: Mutex<u8>,

    /// Monitor identification
    monitor_id: String,
    is_primary: bool,

    /// Capture options
    show_cursor: bool,
    output_format: OutputFormat,
    capture_fps: u32,

    /// YUV conversion support
    yuv_converter: Option<YuvConverter>,

    /// Frame counter for statistics
    frame_count: u64,
}

impl ScreenCapture {
    /// Get the current width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the current height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Create a new screen capture instance for the specified monitor
    pub fn new(monitor_index: Option<usize>) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_options(monitor_index, true, OutputFormat::RGBA)
    }

    /// Create a new screen capture with custom options
    pub fn new_with_options(
        monitor_index: Option<usize>,
        show_cursor: bool,
        output_format: OutputFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let idx = monitor_index.unwrap_or(0);
        
        info!("🖥️  Initializing native screen capture for monitor {}", idx);

        // Create native capture instance
        let native_capture = NativeScreenCapture::new_with_options(Some(idx), show_cursor)
            .map_err(|e| format!("Failed to initialize native capture: {}", e))?;

        let (width, height) = native_capture.dimensions();
        let width = width as usize;
        let height = height as usize;

        info!(
            "✅ Native screen capture initialized: {}x{}, cursor: {}, format: {:?}",
            width, height, show_cursor, output_format
        );

        // Initialize tile structure (64x64 tiles for delta encoding)
        let tile_size = 64;
        let tiles_x = (width + tile_size - 1) / tile_size;
        let tiles_y = (height + tile_size - 1) / tile_size;
        let total_tiles = tiles_x * tiles_y;

        let tiles = vec![
            ScreenTile {
                data: Vec::new(),
                hash: 0,
                changed: false,
            };
            total_tiles
        ];

        // Initialize YUV converter if needed
        let yuv_converter = match output_format {
            OutputFormat::YUV420 => Some(YuvConverter::new(width, height)),
            _ => None,
        };

        Ok(Self {
            native_capture,
            width,
            height,
            tile_size,
            tiles,
            previous_frame: None,
            adaptive_quality: Mutex::new(80),
            monitor_id: format!("native-monitor-{}", idx),
            is_primary: idx == 0,
            show_cursor,
            output_format,
            capture_fps: 30,
            yuv_converter,
            frame_count: 0,
        })
    }

    /// Get all available monitors
    pub fn get_all_monitors() -> Result<Vec<MonitorInfo>, Box<dyn std::error::Error>> {
        let native_monitors = NativeScreenCapture::enumerate_monitors()
            .map_err(|e| format!("Failed to enumerate monitors: {}", e))?;

        let monitors: Vec<MonitorInfo> = native_monitors
            .into_iter()
            .map(|m| MonitorInfo {
                id: m.id,
                name: m.name,
                is_primary: m.is_primary,
                width: m.width as usize,
                height: m.height as usize,
                position_x: m.position_x,
                position_y: m.position_y,
                scale_factor: 1.0,
                rotation: 0,
                supports_cursor: true,
                supports_highlight: false,
            })
            .collect();

        if monitors.is_empty() {
            // Fallback to default monitor
            Ok(vec![MonitorInfo {
                id: "primary".to_string(),
                name: "Primary Display".to_string(),
                is_primary: true,
                width: 1920,
                height: 1080,
                position_x: 0,
                position_y: 0,
                scale_factor: 1.0,
                rotation: 0,
                supports_cursor: true,
                supports_highlight: false,
            }])
        } else {
            Ok(monitors)
        }
    }

    /// Capture a raw frame in the current output format
    pub fn capture_raw(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.frame_count += 1;

        // Capture RGBA frame using native capture
        let rgba_buffer = self.native_capture.capture_rgba()
            .map_err(|e| format!("Native capture failed: {}", e))?;

        // Update dimensions based on captured frame
        let (new_width, new_height) = self.native_capture.dimensions();
        if new_width as usize != self.width || new_height as usize != self.height {
            self.width = new_width as usize;
            self.height = new_height as usize;
            
            // Update YUV converter if format requires it
            if matches!(self.output_format, OutputFormat::YUV420) {
                self.yuv_converter = Some(YuvConverter::new(self.width, self.height));
            }
            
            debug!("Screen dimensions updated: {}x{}", self.width, self.height);
        }

        // Apply format conversion if needed
        let output_buffer = match self.output_format {
            OutputFormat::RGBA => rgba_buffer,
            OutputFormat::BGRA => {
                // Convert RGBA to BGRA
                let mut bgra_buffer = Vec::with_capacity(rgba_buffer.len());
                for chunk in rgba_buffer.chunks_exact(4) {
                    bgra_buffer.push(chunk[2]); // B
                    bgra_buffer.push(chunk[1]); // G
                    bgra_buffer.push(chunk[0]); // R
                    bgra_buffer.push(chunk[3]); // A
                }
                bgra_buffer
            }
            OutputFormat::YUV420 => {
                if let Some(ref converter) = self.yuv_converter {
                    converter.rgba_to_yuv420(&rgba_buffer)
                } else {
                    return Err("YUV converter not initialized".into());
                }
            }
        };

        // Store for delta encoding
        self.previous_frame = Some(output_buffer.clone());

        // Log occasionally for debugging
        if self.frame_count % 60 == 0 {
            debug!(
                "📸 Frame #{}: {}x{} ({:.1} KB)",
                self.frame_count,
                self.width,
                self.height,
                output_buffer.len() as f64 / 1024.0
            );
        }

        Ok(output_buffer)
    }

    /// Capture a frame in RGBA format
    pub fn capture_rgba(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if matches!(self.output_format, OutputFormat::RGBA) {
            return self.capture_raw();
        }

        // Temporarily switch to RGBA format
        let original_format = self.output_format.clone();
        self.output_format = OutputFormat::RGBA;
        let yuv_converter = self.yuv_converter.take();

        let result = self.capture_raw();

        // Restore original format
        self.output_format = original_format;
        self.yuv_converter = yuv_converter;

        result
    }

    /// Get current dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get tile dimensions for delta encoding
    pub fn tile_dimensions(&self) -> (usize, usize, usize) {
        let tiles_x = (self.width + self.tile_size - 1) / self.tile_size;
        let tiles_y = (self.height + self.tile_size - 1) / self.tile_size;
        (tiles_x, tiles_y, self.tile_size)
    }

    /// Update adaptive quality level
    pub fn update_quality(&self, quality: u8) {
        if let Ok(mut current_quality) = self.adaptive_quality.lock() {
            *current_quality = quality.clamp(1, 100);
        }
    }

    /// Get the monitor ID
    pub fn get_monitor_id(&self) -> &str {
        &self.monitor_id
    }

    /// Check if this is the primary monitor
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// Enable or disable cursor capture
    pub fn set_cursor_capture(&mut self, show_cursor: bool) -> Result<(), Box<dyn std::error::Error>> {
        self.show_cursor = show_cursor;
        info!("Cursor capture set to: {}", show_cursor);
        Ok(())
    }

    /// Set capture FPS preference
    pub fn set_fps(&mut self, fps: u32) -> Result<(), Box<dyn std::error::Error>> {
        self.capture_fps = fps;
        info!("Capture FPS set to: {}", fps);
        Ok(())
    }

    /// Set output format
    pub fn set_output_format(
        &mut self,
        format: OutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.output_format = format.clone();

        // Update YUV converter based on format
        match format {
            OutputFormat::YUV420 => {
                self.yuv_converter = Some(YuvConverter::new(self.width, self.height));
            }
            _ => {
                self.yuv_converter = None;
            }
        }

        info!("Output format set to: {:?}", format);
        Ok(())
    }

    /// Check if cursor capture is enabled
    pub fn cursor_enabled(&self) -> bool {
        self.show_cursor
    }

    /// Get current FPS setting
    pub fn get_fps(&self) -> u32 {
        self.capture_fps
    }

    /// Get current output format
    pub fn get_output_format(&self) -> &OutputFormat {
        &self.output_format
    }

    /// Capture YUV frame directly
    pub fn capture_yuv(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if matches!(self.output_format, OutputFormat::YUV420) {
            return self.capture_raw();
        }

        // Temporarily switch to YUV format
        let original_format = self.output_format.clone();
        self.output_format = OutputFormat::YUV420;
        self.yuv_converter = Some(YuvConverter::new(self.width, self.height));

        let result = self.capture_raw();

        // Restore original format
        self.output_format = original_format;
        if !matches!(self.output_format, OutputFormat::YUV420) {
            self.yuv_converter = None;
        }

        result
    }

    /// Get supported capabilities
    pub fn get_capabilities(&self) -> Vec<String> {
        vec![
            "native_gdi_capture".to_string(),
            "rgba_output".to_string(),
            "bgra_output".to_string(),
            "yuv420_output".to_string(),
            "format_conversion".to_string(),
            "tile_based_capture".to_string(),
            "adaptive_quality".to_string(),
            "cursor_capture".to_string(),
        ]
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        info!("ScreenCapture stopped and cleaned up");
    }
}
