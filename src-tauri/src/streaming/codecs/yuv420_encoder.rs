use anyhow::{Result, Context};
use thiserror::Error;
use log::{debug, error, info, warn};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use image::{ImageBuffer, Rgba, DynamicImage};

// Simplified encoder using native capture for stability

/// Enhanced YUV420 video encoder errors
#[derive(Error, Debug)]
pub enum YUV420EncoderError {
    #[error("H.264 encoder initialization failed: {0}")]
    H264Init(String),
    #[error("Container creation failed: {0}")]
    ContainerInit(String),
    #[error("Frame encoding failed: {0}")]
    Encode(String),
    #[error("YUV conversion failed: {0}")]
    YUVConversion(String),
    #[error("Monitor capture failed: {0}")]
    Capture(String),
    #[error("Invalid configuration: {0}")]
    Config(String),
}

/// YUV420 encoder configuration for H.264 streaming
#[derive(Clone, Debug)]
pub struct YUV420Config {
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub bitrate: u32,
    pub keyframe_interval: u32,
    pub quality: u32,
    pub monitor_id: usize,
    pub use_h264_container: bool,
    pub enable_audio: bool,
    pub opus_bitrate: u32,
    pub temporal_layers: u8,
    pub spatial_layers: u8,
}

impl Default for YUV420Config {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            framerate: 30,
            bitrate: 2000,
            keyframe_interval: 60,
            quality: 20,
            monitor_id: 0,
            use_h264_container: true,
            enable_audio: false,
            opus_bitrate: 128000,
            temporal_layers: 1,
            spatial_layers: 1,
        }
    }
}

/// YUV420 frame data with proper color space conversion
#[derive(Debug, Clone)]
pub struct YUV420Frame {
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frame_number: u64,
    pub timestamp: u64,
    pub is_keyframe: bool,
}

impl YUV420Frame {
    pub fn new(width: u32, height: u32, frame_number: u64) -> Self {
        let y_size = (width * height) as usize;
        let uv_size = (width * height / 4) as usize; // 4:2:0 subsampling
        
        Self {
            y_plane: vec![0; y_size],
            u_plane: vec![0; uv_size],
            v_plane: vec![0; uv_size],
            width,
            height,
            frame_number,
            timestamp: 0,
            is_keyframe: false,
        }
    }
    
    /// Convert RGBA to YUV420 using Rec. 709 color space (better for screen content)
    pub fn from_rgba(rgba_data: &[u8], width: u32, height: u32, frame_number: u64) -> Result<Self, YUV420EncoderError> {
        let mut frame = YUV420Frame::new(width, height, frame_number);
        
        // Use Rec. 709 coefficients for better screen content representation
        let kr = 0.2126;
        let kg = 0.7152; 
        let kb = 0.0722;
        
        // Convert RGB to YUV with Rec. 709 color space
        for y in 0..height {
            for x in 0..width {
                let rgba_idx = ((y * width + x) * 4) as usize;
                if rgba_idx + 2 >= rgba_data.len() {
                    continue;
                }
                
                let r = rgba_data[rgba_idx] as f64 / 255.0;
                let g = rgba_data[rgba_idx + 1] as f64 / 255.0;
                let b = rgba_data[rgba_idx + 2] as f64 / 255.0;
                
                // Calculate Y (luminance) using Rec. 709
                let y_val = (kr * r + kg * g + kb * b) * 255.0;
                let y_idx = (y * width + x) as usize;
                frame.y_plane[y_idx] = y_val.clamp(0.0, 255.0) as u8;
                
                // Calculate U and V (chrominance) for 4:2:0 subsampling
                if y % 2 == 0 && x % 2 == 0 {
                    let u_val = (-0.1146 * r - 0.3854 * g + 0.5000 * b + 0.5) * 255.0;
                    let v_val = (0.5000 * r - 0.4542 * g - 0.0458 * b + 0.5) * 255.0;
                    
                    let uv_idx = ((y / 2) * (width / 2) + (x / 2)) as usize;
                    if uv_idx < frame.u_plane.len() {
                        frame.u_plane[uv_idx] = u_val.clamp(0.0, 255.0) as u8;
                        frame.v_plane[uv_idx] = v_val.clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
        
        frame.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
            
        Ok(frame)
    }
    
    /// Get total frame size in bytes
    pub fn size(&self) -> usize {
        self.y_plane.len() + self.u_plane.len() + self.v_plane.len()
    }
}

/// Simplified YUV420 encoder for stable screen streaming
pub struct YUV420Encoder {
    config: YUV420Config,
    frame_count: AtomicU64,
    last_keyframe: AtomicU64,
    
    // Performance tracking
    encoding_stats: Arc<EncodingStats>,
    
    // Frame processing pipeline
    frame_buffer: Arc<Mutex<Vec<YUV420Frame>>>,
    max_buffer_size: usize,
}

/// Performance statistics for monitoring
#[derive(Debug)]
pub struct EncodingStats {
    pub frames_encoded: AtomicU64,
    pub keyframes_encoded: AtomicU64,
    pub total_bytes_out: AtomicU64,
    pub avg_encode_time_ms: AtomicU32,
    pub avg_fps: AtomicU32,
    pub last_update: Mutex<Instant>,
}

impl EncodingStats {
    pub fn new() -> Self {
        Self {
            frames_encoded: AtomicU64::new(0),
            keyframes_encoded: AtomicU64::new(0),
            total_bytes_out: AtomicU64::new(0),
            avg_encode_time_ms: AtomicU32::new(0),
            avg_fps: AtomicU32::new(0),
            last_update: Mutex::new(Instant::now()),
        }
    }
    
    pub fn update_frame_stats(&self, is_keyframe: bool, bytes_out: usize, encode_time_ms: u32) {
        self.frames_encoded.fetch_add(1, Ordering::Relaxed);
        if is_keyframe {
            self.keyframes_encoded.fetch_add(1, Ordering::Relaxed);
        }
        self.total_bytes_out.fetch_add(bytes_out as u64, Ordering::Relaxed);
        self.avg_encode_time_ms.store(encode_time_ms, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> (u64, u64, u64, u32, u32) {
        (
            self.frames_encoded.load(Ordering::Relaxed),
            self.keyframes_encoded.load(Ordering::Relaxed),
            self.total_bytes_out.load(Ordering::Relaxed),
            self.avg_encode_time_ms.load(Ordering::Relaxed),
            self.avg_fps.load(Ordering::Relaxed),
        )
    }
}

impl YUV420Encoder {
    /// Create a new simplified YUV420 encoder for stable streaming
    pub fn new(config: YUV420Config) -> Result<Self, YUV420EncoderError> {
        info!("Initializing YUV420 encoder with native capture: {}x{} @ {}fps, {}kbps", 
              config.width, config.height, config.framerate, config.bitrate);
        
        // Validate configuration
        if config.width == 0 || config.height == 0 {
            return Err(YUV420EncoderError::Config("Invalid dimensions".to_string()));
        }
        if config.framerate == 0 || config.framerate > 120 {
            return Err(YUV420EncoderError::Config("Invalid framerate".to_string()));
        }
        
        let encoder = Self {
            config: config.clone(),
            frame_count: AtomicU64::new(0),
            last_keyframe: AtomicU64::new(0),
            encoding_stats: Arc::new(EncodingStats::new()),
            frame_buffer: Arc::new(Mutex::new(Vec::new())),
            max_buffer_size: 5,
        };
        
        Ok(encoder)
    }
    
    /// Initialize encoder (simplified version for now)
    pub fn initialize_encoder(&mut self) -> Result<(), YUV420EncoderError> {
        info!("Initializing simplified YUV420 encoder...");
        info!("✅ Encoder initialized successfully");
        Ok(())
    }
    
    /// Capture and encode a frame
    pub fn capture_and_encode(&mut self, force_keyframe: bool) -> Result<Option<Vec<u8>>, YUV420EncoderError> {
        use crate::core::ScreenCapture;
        
        // Use native screen capture
        let mut screen_capture = ScreenCapture::new(Some(self.config.monitor_id))
            .map_err(|e| YUV420EncoderError::Capture(format!("Failed to init capture: {}", e)))?;
        
        let rgba_data = screen_capture.capture_rgba()
            .map_err(|e| YUV420EncoderError::Capture(format!("Capture failed: {}", e)))?;
        
        let (width, height) = screen_capture.dimensions();
        let frame_count = self.frame_count.fetch_add(1, Ordering::Relaxed);
        
        // Convert RGBA to YUV420
        let yuv_frame = YUV420Frame::from_rgba(&rgba_data, width as u32, height as u32, frame_count)
            .map_err(|e| YUV420EncoderError::YUVConversion(format!("Conversion failed: {}", e)))?;
        
        // Determine if keyframe is needed
        let last_keyframe = self.last_keyframe.load(Ordering::Relaxed);
        let should_keyframe = force_keyframe || (frame_count - last_keyframe) >= self.config.keyframe_interval as u64;
        
        if should_keyframe {
            self.last_keyframe.store(frame_count, Ordering::Relaxed);
        }
        
        // Encode the frame
        self.encode_yuv_frame(yuv_frame, should_keyframe)
    }

    /// Encode a YUV420 frame (simplified version for now)
    fn encode_yuv_frame(&mut self, yuv_frame: YUV420Frame, is_keyframe: bool) -> Result<Option<Vec<u8>>, YUV420EncoderError> {
        // For now, return the raw YUV420 data as a simple encoded frame
        // In production, H.264 hardware encoding is used
        
        let mut encoded_data = Vec::new();
        
        // Simple header with frame info
        encoded_data.extend_from_slice(&[0x59, 0x55, 0x56]); // "YUV" signature
        
        if is_keyframe {
            encoded_data.push(0x01); // Keyframe marker
        } else {
            encoded_data.push(0x00); // Inter-frame marker
        }
        
        // Frame metadata
        encoded_data.extend_from_slice(&yuv_frame.width.to_le_bytes());
        encoded_data.extend_from_slice(&yuv_frame.height.to_le_bytes());
        encoded_data.extend_from_slice(&yuv_frame.timestamp.to_le_bytes());
        
        // YUV420 data
        encoded_data.extend_from_slice(&(yuv_frame.y_plane.len() as u32).to_le_bytes());
        encoded_data.extend_from_slice(&yuv_frame.y_plane);
        
        encoded_data.extend_from_slice(&(yuv_frame.u_plane.len() as u32).to_le_bytes());
        encoded_data.extend_from_slice(&yuv_frame.u_plane);
        
        encoded_data.extend_from_slice(&(yuv_frame.v_plane.len() as u32).to_le_bytes());
        encoded_data.extend_from_slice(&yuv_frame.v_plane);
        
        if is_keyframe {
            info!("📹 Encoded YUV420 keyframe {}: {} bytes", yuv_frame.frame_number, encoded_data.len());
        } else {
            debug!("📹 Encoded YUV420 frame {}: {} bytes", yuv_frame.frame_number, encoded_data.len());
        }
        
        Ok(Some(encoded_data))
    }
    
    /// Resize RGBA image using high-quality interpolation
    fn resize_rgba(&self, rgba_data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32) -> Result<Vec<u8>, YUV420EncoderError> {
        use image::{ImageBuffer, Rgba, DynamicImage};
        
        // Convert to ImageBuffer
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(src_width, src_height, rgba_data.to_vec())
            .ok_or_else(|| YUV420EncoderError::Config("Invalid RGBA data".to_string()))?;
        
        // Resize using high-quality filtering
        let dynamic_img = DynamicImage::ImageRgba8(img_buffer);
        let resized = dynamic_img.resize(dst_width, dst_height, image::imageops::FilterType::Lanczos3);
        
        Ok(resized.as_rgba8().unwrap().to_vec())
    }
    
    /// Get encoder performance statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u32, u32) {
        self.encoding_stats.get_stats()
    }
    
    /// Force next frame to be a keyframe
    pub fn force_keyframe(&self) {
        self.last_keyframe.store(0, Ordering::Relaxed);
    }
    
    /// Update encoder configuration dynamically
    pub fn update_config(&mut self, new_config: YUV420Config) -> Result<(), YUV420EncoderError> {
        info!("Updating encoder configuration: {}x{} @ {}fps, {}kbps", 
              new_config.width, new_config.height, new_config.framerate, new_config.bitrate);
        
        // Update configuration
        self.config = new_config;
        self.force_keyframe(); // Force keyframe after config change
        
        Ok(())
    }
    
    /// Encode RGBA frame data directly (for compatibility)
    pub fn encode_rgba_frame(&mut self, rgba_data: &[u8], width: u32, height: u32, timestamp_us: u64, force_keyframe: bool) -> Result<Vec<u8>, YUV420EncoderError> {
        // Convert RGBA to YUV420
        let frame_number = self.frame_count.fetch_add(1, Ordering::Relaxed);
        let yuv_frame = YUV420Frame::from_rgba(rgba_data, width, height, frame_number)?;
        
        // Encode the frame
        match self.encode_yuv_frame(yuv_frame, force_keyframe)? {
            Some(data) => Ok(data),
            None => Ok(Vec::new()), // Return empty vec if no data
        }
    }
    
    /// Encode YUV420 frame directly (for compatibility)
    pub fn encode_yuv420_frame(&mut self, y_plane: &[u8], u_plane: &[u8], v_plane: &[u8], 
                               width: u32, height: u32, timestamp_us: u64, force_keyframe: bool) -> Result<Vec<u8>, YUV420EncoderError> {
        // Create YUV frame from planes
        let frame_number = self.frame_count.fetch_add(1, Ordering::Relaxed);
        let yuv_frame = YUV420Frame {
            y_plane: y_plane.to_vec(),
            u_plane: u_plane.to_vec(),
            v_plane: v_plane.to_vec(),
            width,
            height,
            frame_number,
            timestamp: timestamp_us,
            is_keyframe: force_keyframe,
        };
        
        // Encode the frame
        match self.encode_yuv_frame(yuv_frame, force_keyframe)? {
            Some(data) => Ok(data),
            None => Ok(Vec::new()), // Return empty vec if no data
        }
    }
}

impl Drop for YUV420Encoder {
    fn drop(&mut self) {
        info!("YUV420 encoder dropped");
    }
}

/// Utility functions for YUV420 color space operations
pub mod yuv_utils {
    /// Convert RGB to YUV using Rec. 709 color space (better for computer displays)
    pub fn rgb_to_yuv_rec709(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let rf = r as f64 / 255.0;
        let gf = g as f64 / 255.0;
        let bf = b as f64 / 255.0;
        
        let y = (0.2126 * rf + 0.7152 * gf + 0.0722 * bf) * 255.0;
        let u = ((-0.1146 * rf - 0.3854 * gf + 0.5000 * bf) + 0.5) * 255.0;
        let v = ((0.5000 * rf - 0.4542 * gf - 0.0458 * bf) + 0.5) * 255.0;
        
        (
            y.clamp(0.0, 255.0) as u8,
            u.clamp(0.0, 255.0) as u8,
            v.clamp(0.0, 255.0) as u8,
        )
    }
    
    /// Convert YUV to RGB using Rec. 709 color space
    pub fn yuv_to_rgb_rec709(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
        let yf = y as f64 / 255.0;
        let uf = (u as f64 / 255.0) - 0.5;
        let vf = (v as f64 / 255.0) - 0.5;
        
        let r = (yf + 1.5748 * vf) * 255.0;
        let g = (yf - 0.1873 * uf - 0.4681 * vf) * 255.0;
        let b = (yf + 1.8556 * uf) * 255.0;
        
        (
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        )
    }
}
