//! Native Cross-Platform Screen Capture Module
//!
//! This module provides a stable, high-performance screen capture implementation
//! for Windows, Linux (X11), and macOS using native platform APIs.
//!
//! Platform-specific implementations:
//! - **Windows**: GDI (GetDC, BitBlt, GetDIBits) for maximum compatibility
//! - **Linux X11**: X11 library (XGetImage) for X Window System
//! - **macOS**: Core Graphics (CGDisplayCreateImage) for Quartz display
//!
//! This replaces external dependencies with direct platform API implementations
//! for better stability and reliability across all platforms.

// Allow dead code for platform-specific capture utilities that may not be used
// on all platforms (conditional compilation means functions compiled on one
// platform may not be called on others)
#![allow(dead_code)]

use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

// Platform-specific imports
#[cfg(target_os = "windows")]
use windows_capture::monitor::Monitor;

#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt, ImageFormat, Screen},
    rust_connection::RustConnection,
};

#[cfg(target_os = "macos")]
use core_graphics::display::CGDisplay;

/// Native monitor information structure
#[derive(Debug, Clone)]
pub struct NativeMonitorInfo {
    pub id: String,
    pub name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub position_x: i32,
    pub position_y: i32,
}

/// Native screen capture error types
#[derive(Debug, Clone)]
pub enum NativeCaptureError {
    InitializationFailed(String),
    CaptureError(String),
    NoMonitorFound,
    PermissionDenied,
    UnsupportedPlatform,
}

impl std::fmt::Display for NativeCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeCaptureError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            NativeCaptureError::CaptureError(msg) => write!(f, "Capture error: {}", msg),
            NativeCaptureError::NoMonitorFound => write!(f, "No monitor found"),
            NativeCaptureError::PermissionDenied => write!(f, "Permission denied"),
            NativeCaptureError::UnsupportedPlatform => write!(f, "Unsupported platform"),
        }
    }
}

impl std::error::Error for NativeCaptureError {}

/// Shared frame data between capture handler and main thread
#[derive(Debug, Default)]
pub struct SharedFrameData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frame_number: u64,
    pub is_valid: bool,
}

/// Native screen capture implementation using platform-specific APIs
///
/// - Windows: GDI (GetDC, BitBlt) for stable screen capture
/// - Linux: X11 (XGetImage) for X Window System capture  
/// - macOS: Core Graphics (CGDisplayCreateImage) for Quartz capture
pub struct NativeScreenCapture {
    monitor_index: usize,
    width: u32,
    height: u32,
    frame_count: AtomicU64,
    show_cursor: bool,
    
    // Shared frame data for callback-based capture
    shared_frame: Arc<Mutex<SharedFrameData>>,
    
    // Capture state
    capture_active: Arc<std::sync::atomic::AtomicBool>,
    
    // Platform-specific state
    #[cfg(target_os = "linux")]
    x11_connection: Option<Arc<RustConnection>>,
    
    #[cfg(target_os = "linux")]
    x11_screen_num: usize,
}

impl NativeScreenCapture {
    /// Create a new native screen capture instance
    pub fn new(monitor_index: Option<usize>) -> Result<Self, NativeCaptureError> {
        Self::new_with_options(monitor_index, true)
    }
    
    /// Create a new native screen capture with cursor options
    pub fn new_with_options(monitor_index: Option<usize>, show_cursor: bool) -> Result<Self, NativeCaptureError> {
        let idx = monitor_index.unwrap_or(0);
        
        #[cfg(target_os = "windows")]
        info!("[INFO] Initializing native Windows screen capture for monitor {}", idx);
        
        #[cfg(target_os = "linux")]
        info!("[INFO] Initializing native Linux X11 screen capture for monitor {}", idx);
        
        #[cfg(target_os = "macos")]
        info!("[INFO] Initializing native macOS screen capture for monitor {}", idx);
        
        // Get monitor info to determine dimensions
        let monitors = Self::enumerate_monitors()?;
        
        if monitors.is_empty() {
            error!("[ERROR] No monitors found");
            return Err(NativeCaptureError::NoMonitorFound);
        }
        
        let target_monitor = if idx < monitors.len() {
            &monitors[idx]
        } else {
            warn!("[WARNING] Monitor {} not found, using primary", idx);
            monitors.iter().find(|m| m.is_primary).unwrap_or(&monitors[0])
        };
        
        info!("[OK] Using monitor: {} ({}x{})", target_monitor.name, target_monitor.width, target_monitor.height);
        
        // Platform-specific initialization
        #[cfg(target_os = "linux")]
        let (x11_connection, x11_screen_num) = {
            let (conn, screen_num) = RustConnection::connect(None)
                .map_err(|e| NativeCaptureError::InitializationFailed(format!("X11 connection failed: {}", e)))?;
            (Some(Arc::new(conn)), screen_num)
        };
        
        Ok(Self {
            monitor_index: idx,
            width: target_monitor.width,
            height: target_monitor.height,
            frame_count: AtomicU64::new(0),
            show_cursor,
            shared_frame: Arc::new(Mutex::new(SharedFrameData::default())),
            capture_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(target_os = "linux")]
            x11_connection,
            #[cfg(target_os = "linux")]
            x11_screen_num,
        })
    }
    
    /// Enumerate all available monitors
    pub fn enumerate_monitors() -> Result<Vec<NativeMonitorInfo>, NativeCaptureError> {
        #[cfg(target_os = "windows")]
        {
            Self::enumerate_monitors_windows()
        }
        
        #[cfg(target_os = "linux")]
        {
            Self::enumerate_monitors_linux()
        }
        
        #[cfg(target_os = "macos")]
        {
            Self::enumerate_monitors_macos()
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(NativeCaptureError::UnsupportedPlatform)
        }
    }
    
    // =========================================================================
    // Windows Monitor Enumeration
    // =========================================================================
    
    #[cfg(target_os = "windows")]
    fn enumerate_monitors_windows() -> Result<Vec<NativeMonitorInfo>, NativeCaptureError> {
        let monitors = Monitor::enumerate()
            .map_err(|e| NativeCaptureError::InitializationFailed(format!("Failed to enumerate monitors: {}", e)))?;
        
        let mut result = Vec::new();
        for (i, monitor) in monitors.iter().enumerate() {
            let name = monitor.name().unwrap_or_else(|_| format!("Display {}", i + 1));
            let is_primary = i == 0;
            let (width, height) = (1920, 1080);
            
            result.push(NativeMonitorInfo {
                id: format!("monitor-{}", i),
                name,
                is_primary,
                width,
                height,
                position_x: 0,
                position_y: 0,
            });
        }
        
        if result.is_empty() {
            result.push(NativeMonitorInfo {
                id: "primary".to_string(),
                name: "Primary Display".to_string(),
                is_primary: true,
                width: 1920,
                height: 1080,
                position_x: 0,
                position_y: 0,
            });
        }
        
        Ok(result)
    }
    
    // =========================================================================
    // Linux X11 Monitor Enumeration
    // =========================================================================
    
    #[cfg(target_os = "linux")]
    fn enumerate_monitors_linux() -> Result<Vec<NativeMonitorInfo>, NativeCaptureError> {
        use x11rb::protocol::randr::{self, ConnectionExt as RandrConnectionExt};
        
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| NativeCaptureError::InitializationFailed(format!("X11 connection failed: {}", e)))?;
        
        let setup = conn.setup();
        let screen = &setup.roots[screen_num];
        
        // Try to use RandR extension for multi-monitor support
        let mut result = Vec::new();
        
        if let Ok(resources) = conn.randr_get_screen_resources_current(screen.root) {
            if let Ok(resources) = resources.reply() {
                for (i, output) in resources.outputs.iter().enumerate() {
                    if let Ok(output_info) = conn.randr_get_output_info(*output, resources.config_timestamp) {
                        if let Ok(output_info) = output_info.reply() {
                            if output_info.connection == randr::Connection::CONNECTED {
                                if let Some(crtc) = output_info.crtc.checked_sub(0).filter(|&c| c != 0) {
                                    if let Ok(crtc_info) = conn.randr_get_crtc_info(crtc, resources.config_timestamp) {
                                        if let Ok(crtc_info) = crtc_info.reply() {
                                            let name = String::from_utf8_lossy(&output_info.name).to_string();
                                            result.push(NativeMonitorInfo {
                                                id: format!("monitor-{}", i),
                                                name: if name.is_empty() { format!("Display {}", i + 1) } else { name },
                                                is_primary: i == 0,
                                                width: crtc_info.width as u32,
                                                height: crtc_info.height as u32,
                                                position_x: crtc_info.x as i32,
                                                position_y: crtc_info.y as i32,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback to root window dimensions if RandR fails
        if result.is_empty() {
            result.push(NativeMonitorInfo {
                id: "primary".to_string(),
                name: "Primary Display".to_string(),
                is_primary: true,
                width: screen.width_in_pixels as u32,
                height: screen.height_in_pixels as u32,
                position_x: 0,
                position_y: 0,
            });
        }
        
        info!("[INFO] Found {} monitors on Linux X11", result.len());
        Ok(result)
    }
    
    // =========================================================================
    // macOS Monitor Enumeration
    // =========================================================================
    
    #[cfg(target_os = "macos")]
    fn enumerate_monitors_macos() -> Result<Vec<NativeMonitorInfo>, NativeCaptureError> {
        let mut result = Vec::new();
        
        // Get all active displays
        let display_ids = CGDisplay::active_displays()
            .map_err(|_| NativeCaptureError::InitializationFailed("Failed to get active displays".to_string()))?;
        
        let main_display = CGDisplay::main();
        let main_display_id = main_display.id;
        
        for (i, &display_id) in display_ids.iter().enumerate() {
            let display = CGDisplay::new(display_id);
            let is_primary = display_id == main_display_id;
            
            // Get display dimensions using pixels_wide/pixels_high
            let width = display.pixels_wide() as u32;
            let height = display.pixels_high() as u32;
            
            result.push(NativeMonitorInfo {
                id: format!("display-{}", display_id),
                name: if is_primary { 
                    "Main Display".to_string() 
                } else { 
                    format!("Display {}", i + 1) 
                },
                is_primary,
                width,
                height,
                position_x: 0, // Position can be obtained if needed
                position_y: 0,
            });
        }
        
        if result.is_empty() {
            result.push(NativeMonitorInfo {
                id: "main".to_string(),
                name: "Main Display".to_string(),
                is_primary: true,
                width: main_display.pixels_wide() as u32,
                height: main_display.pixels_high() as u32,
                position_x: 0,
                position_y: 0,
            });
        }
        
        info!("[INFO] Found {} monitors on macOS", result.len());
        Ok(result)
    }
    
    // =========================================================================
    // Screen Capture - Platform Dispatcher
    // =========================================================================
    
    /// Capture a single frame in RGBA format
    pub fn capture_rgba(&mut self) -> Result<Vec<u8>, NativeCaptureError> {
        let frame_num = self.frame_count.fetch_add(1, Ordering::Relaxed);
        
        #[cfg(target_os = "windows")]
        {
            self.capture_gdi()
        }
        
        #[cfg(target_os = "linux")]
        {
            self.capture_x11()
        }
        
        #[cfg(target_os = "macos")]
        {
            self.capture_macos()
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(NativeCaptureError::UnsupportedPlatform)
        }
    }
    
    // =========================================================================
    // Windows GDI Capture
    // =========================================================================
    
    /// GDI-based screen capture for maximum stability (Windows)
    #[cfg(target_os = "windows")]
    fn capture_gdi(&self) -> Result<Vec<u8>, NativeCaptureError> {
        use std::ptr;
        use std::mem;
        
        // Use raw Windows API calls for GDI capture
        #[link(name = "user32")]
        extern "system" {
            fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
            fn GetSystemMetrics(index: i32) -> i32;
        }
        
        #[link(name = "gdi32")]
        extern "system" {
            fn CreateCompatibleDC(hdc: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn CreateCompatibleBitmap(hdc: *mut std::ffi::c_void, width: i32, height: i32) -> *mut std::ffi::c_void;
            fn SelectObject(hdc: *mut std::ffi::c_void, obj: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn BitBlt(dest: *mut std::ffi::c_void, x: i32, y: i32, w: i32, h: i32, 
                      src: *mut std::ffi::c_void, sx: i32, sy: i32, rop: u32) -> i32;
            fn GetDIBits(hdc: *mut std::ffi::c_void, bitmap: *mut std::ffi::c_void, start: u32, 
                         lines: u32, bits: *mut u8, bi: *mut BITMAPINFO, usage: u32) -> i32;
            fn DeleteObject(obj: *mut std::ffi::c_void) -> i32;
            fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
        }
        
        #[repr(C)]
        struct BITMAPINFOHEADER {
            size: u32,
            width: i32,
            height: i32,
            planes: u16,
            bit_count: u16,
            compression: u32,
            size_image: u32,
            x_pels_per_meter: i32,
            y_pels_per_meter: i32,
            clr_used: u32,
            clr_important: u32,
        }
        
        #[repr(C)]
        struct RGBQUAD {
            blue: u8,
            green: u8,
            red: u8,
            reserved: u8,
        }
        
        #[repr(C)]
        struct BITMAPINFO {
            header: BITMAPINFOHEADER,
            colors: [RGBQUAD; 1],
        }
        
        const SM_CXSCREEN: i32 = 0;
        const SM_CYSCREEN: i32 = 1;
        const SRCCOPY: u32 = 0x00CC0020;
        const BI_RGB: u32 = 0;
        const DIB_RGB_COLORS: u32 = 0;
        
        unsafe {
            // Get screen dimensions
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            
            if width <= 0 || height <= 0 {
                return Err(NativeCaptureError::CaptureError("Invalid screen dimensions".to_string()));
            }
            
            // Get screen DC
            let screen_dc = GetDC(ptr::null_mut());
            if screen_dc.is_null() {
                return Err(NativeCaptureError::CaptureError("Failed to get screen DC".to_string()));
            }
            
            // Create compatible DC and bitmap
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(ptr::null_mut(), screen_dc);
                return Err(NativeCaptureError::CaptureError("Failed to create compatible DC".to_string()));
            }
            
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(ptr::null_mut(), screen_dc);
                return Err(NativeCaptureError::CaptureError("Failed to create bitmap".to_string()));
            }
            
            let old_bitmap = SelectObject(mem_dc, bitmap);
            
            // Copy screen content
            let result = BitBlt(mem_dc, 0, 0, width, height, screen_dc, 0, 0, SRCCOPY);
            if result == 0 {
                SelectObject(mem_dc, old_bitmap);
                DeleteObject(bitmap);
                DeleteDC(mem_dc);
                ReleaseDC(ptr::null_mut(), screen_dc);
                return Err(NativeCaptureError::CaptureError("BitBlt failed".to_string()));
            }
            
            // Prepare BITMAPINFO structure
            let mut bi = BITMAPINFO {
                header: BITMAPINFOHEADER {
                    size: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    width,
                    height: -height, // Negative for top-down DIB
                    planes: 1,
                    bit_count: 32,
                    compression: BI_RGB,
                    size_image: 0,
                    x_pels_per_meter: 0,
                    y_pels_per_meter: 0,
                    clr_used: 0,
                    clr_important: 0,
                },
                colors: [RGBQUAD { blue: 0, green: 0, red: 0, reserved: 0 }],
            };
            
            // Allocate buffer for pixel data
            let buffer_size = (width * height * 4) as usize;
            let mut buffer: Vec<u8> = vec![0u8; buffer_size];
            
            // Get the bitmap bits
            let lines = GetDIBits(
                mem_dc,
                bitmap,
                0,
                height as u32,
                buffer.as_mut_ptr(),
                &mut bi,
                DIB_RGB_COLORS,
            );
            
            // Cleanup
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(ptr::null_mut(), screen_dc);
            
            if lines == 0 {
                return Err(NativeCaptureError::CaptureError("GetDIBits failed".to_string()));
            }
            
            // Convert BGRA to RGBA
            for i in (0..buffer_size).step_by(4) {
                buffer.swap(i, i + 2); // Swap B and R
            }
            
            debug!("[DEBUG] GDI capture complete: {}x{} ({} bytes)", width, height, buffer.len());
            
            Ok(buffer)
        }
    }
    
    // =========================================================================
    // Linux X11 Capture
    // =========================================================================
    
    /// X11-based screen capture for Linux
    #[cfg(target_os = "linux")]
    fn capture_x11(&self) -> Result<Vec<u8>, NativeCaptureError> {
        let conn = self.x11_connection.as_ref()
            .ok_or_else(|| NativeCaptureError::CaptureError("X11 connection not initialized".to_string()))?;
        
        let setup = conn.setup();
        let screen = &setup.roots[self.x11_screen_num];
        let root = screen.root;
        
        // Get geometry of root window
        let geometry = conn.get_geometry(root)
            .map_err(|e| NativeCaptureError::CaptureError(format!("Failed to get geometry: {}", e)))?
            .reply()
            .map_err(|e| NativeCaptureError::CaptureError(format!("Failed to get geometry reply: {}", e)))?;
        
        let width = geometry.width as u32;
        let height = geometry.height as u32;
        
        // Capture the screen using XGetImage equivalent
        let image = conn.get_image(
            ImageFormat::Z_PIXMAP,
            root,
            0, 0,
            width as u16,
            height as u16,
            !0, // All planes
        )
        .map_err(|e| NativeCaptureError::CaptureError(format!("Failed to get image: {}", e)))?
        .reply()
        .map_err(|e| NativeCaptureError::CaptureError(format!("Failed to get image reply: {}", e)))?;
        
        let depth = image.depth;
        let data = image.data;
        
        // Convert to RGBA based on depth and visual
        let rgba_buffer = self.convert_x11_to_rgba(&data, width, height, depth, screen)?;
        
        debug!("[DEBUG] X11 capture complete: {}x{} ({} bytes)", width, height, rgba_buffer.len());
        
        Ok(rgba_buffer)
    }
    
    /// Convert X11 image data to RGBA format
    #[cfg(target_os = "linux")]
    #[allow(unused_variables)]
    fn convert_x11_to_rgba(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        depth: u8,
        screen: &Screen, // Reserved for future visual info lookup
    ) -> Result<Vec<u8>, NativeCaptureError> {
        let pixel_count = (width * height) as usize;
        let mut rgba_buffer = Vec::with_capacity(pixel_count * 4);
        
        match depth {
            24 | 32 => {
                // 32-bit BGRA or 24-bit BGR (padded to 32-bit)
                let bytes_per_pixel = 4;
                for i in 0..pixel_count {
                    let offset = i * bytes_per_pixel;
                    if offset + 3 < data.len() {
                        // X11 typically uses BGRA or BGRX format
                        let b = data[offset];
                        let g = data[offset + 1];
                        let r = data[offset + 2];
                        let a = if depth == 32 { data[offset + 3] } else { 255 };
                        
                        rgba_buffer.push(r);
                        rgba_buffer.push(g);
                        rgba_buffer.push(b);
                        rgba_buffer.push(a);
                    } else {
                        rgba_buffer.extend_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }
            16 => {
                // 16-bit RGB565
                for i in 0..pixel_count {
                    let offset = i * 2;
                    if offset + 1 < data.len() {
                        let pixel = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let r = ((pixel >> 11) & 0x1F) as u8 * 8;
                        let g = ((pixel >> 5) & 0x3F) as u8 * 4;
                        let b = (pixel & 0x1F) as u8 * 8;
                        
                        rgba_buffer.push(r);
                        rgba_buffer.push(g);
                        rgba_buffer.push(b);
                        rgba_buffer.push(255);
                    } else {
                        rgba_buffer.extend_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }
            _ => {
                // Unsupported depth, create black image
                warn!("[WARNING] Unsupported X11 depth: {}, creating fallback", depth);
                rgba_buffer.resize(pixel_count * 4, 0);
                for i in (3..rgba_buffer.len()).step_by(4) {
                    rgba_buffer[i] = 255; // Alpha
                }
            }
        }
        
        Ok(rgba_buffer)
    }
    
    // =========================================================================
    // macOS Core Graphics Capture
    // =========================================================================
    
    /// Core Graphics-based screen capture for macOS
    #[cfg(target_os = "macos")]
    fn capture_macos(&self) -> Result<Vec<u8>, NativeCaptureError> {
        use core_graphics::context::CGContext;
        use core_graphics::color_space::CGColorSpace;
        use core_graphics::geometry::{CGRect, CGPoint, CGSize};
        
        // Get all displays and find the target one
        let display_ids = CGDisplay::active_displays()
            .map_err(|_| NativeCaptureError::CaptureError("Failed to get active displays".to_string()))?;
        
        let display_id = if self.monitor_index < display_ids.len() {
            display_ids[self.monitor_index]
        } else {
            CGDisplay::main().id
        };
        
        let display = CGDisplay::new(display_id);
        
        // Capture the display image
        let image = display.image().ok_or_else(|| {
            NativeCaptureError::CaptureError("Failed to capture display image".to_string())
        })?;
        
        // Get image dimensions
        let width = image.width();
        let height = image.height();
        
        // Create a bitmap context with RGBA format (8 bits per component, 4 bytes per pixel)
        let bytes_per_row = width * 4;
        let mut buffer = vec![0u8; height * bytes_per_row];
        
        let color_space = CGColorSpace::create_device_rgb();
        
        // Create bitmap context with RGBA format
        // kCGImageAlphaPremultipliedLast (1) = RGBA with premultiplied alpha
        // kCGBitmapByteOrder32Big (4 << 12 = 16384) = big-endian byte order
        const BITMAP_INFO: u32 = 1 | (4 << 12); // kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big
        
        let context = CGContext::create_bitmap_context(
            Some(buffer.as_mut_ptr() as *mut _),
            width,
            height,
            8,                  // bits per component
            bytes_per_row,      // bytes per row
            &color_space,
            BITMAP_INFO,
        );
        
        let context = context.ok_or_else(|| {
            NativeCaptureError::CaptureError("Failed to create bitmap context".to_string())
        })?;
        
        // Draw the captured image into our RGBA bitmap context
        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(width as f64, height as f64));
        context.draw_image(rect, &image);
        
        debug!("[DEBUG] macOS capture complete: {}x{} ({} bytes)", width, height, buffer.len());
        
        Ok(buffer)
    }
    
    // =========================================================================
    // Common Methods
    // =========================================================================
    
    /// Get current dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    
    /// Get the monitor index
    pub fn monitor_index(&self) -> usize {
        self.monitor_index
    }
    
    /// Check if cursor capture is enabled
    pub fn cursor_enabled(&self) -> bool {
        self.show_cursor
    }
    
    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }
}

// =============================================================================
// Standalone Capture Functions
// =============================================================================

/// Capture a single frame using native platform APIs (standalone function)
pub fn capture_screen_native(monitor_index: usize) -> Result<(Vec<u8>, u32, u32), NativeCaptureError> {
    let mut capture = NativeScreenCapture::new(Some(monitor_index))?;
    let data = capture.capture_rgba()?;
    let (width, height) = capture.dimensions();
    Ok((data, width, height))
}

/// Capture a single frame using GDI (Windows only, for backward compatibility)
#[cfg(target_os = "windows")]
pub fn capture_screen_gdi(monitor_index: usize) -> Result<(Vec<u8>, u32, u32), NativeCaptureError> {
    capture_screen_native(monitor_index)
}

/// Capture a single frame (non-Windows platforms)
#[cfg(not(target_os = "windows"))]
pub fn capture_screen_gdi(monitor_index: usize) -> Result<(Vec<u8>, u32, u32), NativeCaptureError> {
    capture_screen_native(monitor_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_enumeration() {
        let monitors = NativeScreenCapture::enumerate_monitors();
        assert!(monitors.is_ok());
        let monitors = monitors.unwrap();
        assert!(!monitors.is_empty());
        println!("Found {} monitors", monitors.len());
    }
}
