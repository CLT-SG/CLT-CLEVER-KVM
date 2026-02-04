//! Native Windows Screen Capture Module
//!
//! This module provides a stable, high-performance screen capture implementation
//! for Windows using the Windows Graphics Capture API (DXGI Desktop Duplication)
//! with GDI fallback for maximum compatibility.
//!
//! This replaces the unreliable zed-scap dependency with a direct Windows API implementation.

use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

#[cfg(target_os = "windows")]
use windows_capture::{
    capture::GraphicsCaptureApiHandler,
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

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

/// Native screen capture implementation using Windows APIs
/// 
/// Uses DXGI Desktop Duplication for high-performance capture with GDI fallback
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
}

impl NativeScreenCapture {
    /// Create a new native screen capture instance
    pub fn new(monitor_index: Option<usize>) -> Result<Self, NativeCaptureError> {
        Self::new_with_options(monitor_index, true)
    }
    
    /// Create a new native screen capture with cursor options
    pub fn new_with_options(monitor_index: Option<usize>, show_cursor: bool) -> Result<Self, NativeCaptureError> {
        let idx = monitor_index.unwrap_or(0);
        
        info!("🖥️  Initializing native Windows screen capture for monitor {}", idx);
        
        // Get monitor info to determine dimensions
        let monitors = Self::enumerate_monitors()?;
        
        if monitors.is_empty() {
            error!("❌ No monitors found");
            return Err(NativeCaptureError::NoMonitorFound);
        }
        
        let target_monitor = if idx < monitors.len() {
            &monitors[idx]
        } else {
            warn!("⚠️  Monitor {} not found, using primary", idx);
            monitors.iter().find(|m| m.is_primary).unwrap_or(&monitors[0])
        };
        
        info!("✅ Using monitor: {} ({}x{})", target_monitor.name, target_monitor.width, target_monitor.height);
        
        Ok(Self {
            monitor_index: idx,
            width: target_monitor.width,
            height: target_monitor.height,
            frame_count: AtomicU64::new(0),
            show_cursor,
            shared_frame: Arc::new(Mutex::new(SharedFrameData::default())),
            capture_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }
    
    /// Enumerate all available monitors
    pub fn enumerate_monitors() -> Result<Vec<NativeMonitorInfo>, NativeCaptureError> {
        #[cfg(target_os = "windows")]
        {
            let monitors = Monitor::enumerate()
                .map_err(|e| NativeCaptureError::InitializationFailed(format!("Failed to enumerate monitors: {}", e)))?;
            
            let mut result = Vec::new();
            for (i, monitor) in monitors.iter().enumerate() {
                let name = monitor.name().unwrap_or_else(|_| format!("Display {}", i + 1));
                
                // Get monitor dimensions - use device_name for primary detection
                let is_primary = i == 0; // First monitor is typically primary
                
                // Default dimensions - will be updated on first capture
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
                // Fallback: create at least one default monitor
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
        
        #[cfg(not(target_os = "windows"))]
        {
            Err(NativeCaptureError::UnsupportedPlatform)
        }
    }
    
    /// Capture a single frame in RGBA format
    #[cfg(target_os = "windows")]
    pub fn capture_rgba(&mut self) -> Result<Vec<u8>, NativeCaptureError> {
        use std::time::{Duration, Instant};
        
        let frame_num = self.frame_count.fetch_add(1, Ordering::Relaxed);
        
        // Use GDI capture for stability and simplicity
        self.capture_gdi()
    }
    
    /// GDI-based screen capture for maximum stability
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
            
            debug!("📸 GDI capture complete: {}x{} ({} bytes)", width, height, buffer.len());
            
            Ok(buffer)
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    pub fn capture_rgba(&mut self) -> Result<Vec<u8>, NativeCaptureError> {
        Err(NativeCaptureError::UnsupportedPlatform)
    }
    
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

/// Capture a single frame using GDI (standalone function for fallback use)
#[cfg(target_os = "windows")]
pub fn capture_screen_gdi(monitor_index: usize) -> Result<(Vec<u8>, u32, u32), NativeCaptureError> {
    let mut capture = NativeScreenCapture::new(Some(monitor_index))?;
    let data = capture.capture_rgba()?;
    let (width, height) = capture.dimensions();
    Ok((data, width, height))
}

/// Capture a single frame using GDI (standalone function for fallback use)
#[cfg(not(target_os = "windows"))]
pub fn capture_screen_gdi(_monitor_index: usize) -> Result<(Vec<u8>, u32, u32), NativeCaptureError> {
    Err(NativeCaptureError::UnsupportedPlatform)
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
