use anyhow::Result;
use thiserror::Error;
use log::{debug, error, info};
use std::time::Instant;
use crate::network::models::NetworkStats;

// Custom error type for real-time codec operations
#[derive(Error, Debug)]
pub enum RealtimeCodecError {
    #[error("Screen capture failed: {0}")]
    Capture(String),
    #[error("Encoder initialization failed: {0}")]
    EncoderInit(String),
    #[error("Encoding failed: {0}")]
    Encode(String),
    #[error("Monitor not found: {0}")]
    MonitorNotFound(usize),
    #[error("Configuration error: {0}")]
    Config(String),
}

// Real-time encoder configuration
#[derive(Clone)]
pub struct RealtimeConfig {
    pub monitor_id: usize,
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub framerate: u32,
    pub keyframe_interval: u32,
    pub target_latency_ms: u32,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            monitor_id: 0,
            width: 1920,
            height: 1080,
            bitrate: 4000,
            framerate: 60,
            keyframe_interval: 120,
            target_latency_ms: 50,
        }
    }
}

// Real-time screen streaming encoder using native capture
pub struct RealtimeStreamEncoder {
    config: RealtimeConfig,
    frame_count: u64,
    last_keyframe: u64,
    last_capture_time: Instant,
    capture_duration_ms: f64,
    encode_duration_ms: f64,
    previous_frame_data: Option<Vec<u8>>,
}

impl RealtimeStreamEncoder {
    pub fn new(config: RealtimeConfig) -> Result<Self, RealtimeCodecError> {
        info!("Initializing real-time stream encoder with native capture for monitor {} ({}x{} @ {}fps, {}kbps)", 
              config.monitor_id, config.width, config.height, config.framerate, config.bitrate);

        Ok(Self {
            config,
            frame_count: 0,
            last_keyframe: 0,
            last_capture_time: Instant::now(),
            capture_duration_ms: 0.0,
            encode_duration_ms: 0.0,
            previous_frame_data: None,
        })
    }

    pub fn capture_and_encode(&mut self) -> Result<Vec<u8>, RealtimeCodecError> {
        use crate::core::ScreenCapture;
        
        let capture_start = Instant::now();
        
        // Use native screen capture
        let mut screen_capture = ScreenCapture::new(Some(self.config.monitor_id))
            .map_err(|e| RealtimeCodecError::Capture(format!("Failed to init capture: {}", e)))?;
        
        let rgba_data = screen_capture.capture_rgba()
            .map_err(|e| RealtimeCodecError::Capture(format!("Capture failed: {}", e)))?;
        
        let (width, height) = screen_capture.dimensions();
        
        self.capture_duration_ms = capture_start.elapsed().as_secs_f64() * 1000.0;
        
        // Check if we need a keyframe
        let force_keyframe = (self.frame_count - self.last_keyframe) >= self.config.keyframe_interval as u64;
        
        // Encode the frame
        let encode_start = Instant::now();
        let encoded = self.encode_frame_data(&rgba_data, width as u32, height as u32, force_keyframe)?;
        self.encode_duration_ms = encode_start.elapsed().as_secs_f64() * 1000.0;
        
        self.frame_count += 1;
        self.last_capture_time = Instant::now();
        
        // Log performance occasionally
        if self.frame_count % 60 == 0 {
            debug!(
                "📹 Frame #{}: capture={:.1}ms, encode={:.1}ms, size={:.1}KB",
                self.frame_count,
                self.capture_duration_ms,
                self.encode_duration_ms,
                encoded.len() as f64 / 1024.0
            );
        }
        
        Ok(encoded)
    }

    fn encode_frame_data(&mut self, rgba_data: &[u8], width: u32, height: u32, force_keyframe: bool) -> Result<Vec<u8>, RealtimeCodecError> {
        // Check if we should force a keyframe
        let should_keyframe = force_keyframe || 
            (self.frame_count - self.last_keyframe) >= self.config.keyframe_interval as u64;

        // Create a simple encoded format for real-time streaming
        // This is a basic implementation - H.264 hardware encoding is used in production
        let mut encoded_data = Vec::new();
        
        // Header
        if should_keyframe {
            encoded_data.extend_from_slice(&[0xAA, 0xBB, 0x01]); // Keyframe marker
            self.last_keyframe = self.frame_count;
            debug!("Encoding keyframe {}", self.frame_count);
        } else {
            encoded_data.extend_from_slice(&[0xAA, 0xBB, 0x02]); // Inter-frame marker
        }
        
        // Frame metadata
        encoded_data.extend_from_slice(&(width as u32).to_le_bytes());
        encoded_data.extend_from_slice(&(height as u32).to_le_bytes());
        encoded_data.extend_from_slice(&(self.frame_count as u64).to_le_bytes());
        
        // For real-time streaming, we'll compress the frame data
        let compressed = if should_keyframe || self.previous_frame_data.is_none() {
            // Full frame compression
            self.compress_rgba_data(rgba_data)?
        } else {
            // Delta compression against previous frame
            let previous = self.previous_frame_data.as_ref().unwrap();
            self.compress_delta_frame(rgba_data, previous)?
        };
        
        // Add compressed data length and data
        encoded_data.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        encoded_data.extend_from_slice(&compressed);
        
        if should_keyframe {
            debug!("Encoded keyframe {} ({} -> {} bytes)", self.frame_count, rgba_data.len(), encoded_data.len());
        }

        Ok(encoded_data)
    }
    
    fn compress_rgba_data(&self, rgba_data: &[u8]) -> Result<Vec<u8>, RealtimeCodecError> {
        // Optimized RLE compression for real-time processing
        let mut compressed = Vec::with_capacity(rgba_data.len() / 4); // Pre-allocate with reasonable size
        let mut i = 0;
        
        // Process data in 4-byte pixel chunks for better cache locality
        while i + 4 <= rgba_data.len() {
            let current_pixel = [rgba_data[i], rgba_data[i+1], rgba_data[i+2], rgba_data[i+3]];
            
            // Count consecutive identical pixels (limited to 255 for single byte count)
            let mut count = 1u8;
            let mut j = i + 4;
            
            while j + 4 <= rgba_data.len() && count < 255 {
                let next_pixel = [rgba_data[j], rgba_data[j+1], rgba_data[j+2], rgba_data[j+3]];
                if current_pixel == next_pixel {
                    count += 1;
                    j += 4;
                } else {
                    break;
                }
            }
            
            // Store count and pixel data efficiently
            compressed.push(count);
            compressed.extend_from_slice(&current_pixel);
            
            i = j;
        }
        
        Ok(compressed)
    }
    
    fn compress_delta_frame(&self, current_rgba: &[u8], previous_rgba: &[u8]) -> Result<Vec<u8>, RealtimeCodecError> {
        // Optimized delta compression with early allocation and chunked processing
        let pixel_count = current_rgba.len() / 4;
        let mut delta_data = Vec::with_capacity(pixel_count * 8); // Reasonable pre-allocation
        let mut changes = Vec::new();
        
        // Process in 4-byte chunks (pixels) for better performance
        for (pixel_idx, (curr_chunk, prev_chunk)) in current_rgba.chunks_exact(4).zip(previous_rgba.chunks_exact(4)).enumerate() {
            // Compare pixels efficiently using array comparison
            if curr_chunk != prev_chunk {
                changes.push((pixel_idx as u32, [curr_chunk[0], curr_chunk[1], curr_chunk[2], curr_chunk[3]]));
                
                // Limit number of changes to prevent excessive data
                if changes.len() > pixel_count / 4 {
                    // Too many changes, fall back to full frame compression
                    debug!("Too many delta changes ({}), falling back to full frame", changes.len());
                    return self.compress_rgba_data(current_rgba);
                }
            }
        }
        
        // Store number of changes efficiently
        delta_data.extend_from_slice(&(changes.len() as u32).to_le_bytes());
        
        // Store changes in compact format
        for (index, pixel) in changes {
            delta_data.extend_from_slice(&index.to_le_bytes());
            delta_data.extend_from_slice(&pixel);
        }
        
        debug!("Delta frame: {} pixel changes out of {} total pixels", 
               delta_data.len() / 8, pixel_count);
        
        Ok(delta_data)
    }

    fn resize_image(&self, rgba_data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32) -> Result<Vec<u8>, RealtimeCodecError> {
        // Simple nearest neighbor resizing for speed
        let mut resized = vec![0u8; (dst_width * dst_height * 4) as usize];
        
        let x_ratio = src_width as f32 / dst_width as f32;
        let y_ratio = src_height as f32 / dst_height as f32;
        
        for y in 0..dst_height {
            for x in 0..dst_width {
                let src_x = (x as f32 * x_ratio) as u32;
                let src_y = (y as f32 * y_ratio) as u32;
                
                let src_idx = ((src_y * src_width + src_x) * 4) as usize;
                let dst_idx = ((y * dst_width + x) * 4) as usize;
                
                if src_idx + 3 < rgba_data.len() && dst_idx + 3 < resized.len() {
                    resized[dst_idx..dst_idx + 4].copy_from_slice(&rgba_data[src_idx..src_idx + 4]);
                }
            }
        }
        
        Ok(resized)
    }

    pub fn update_bitrate(&mut self, new_bitrate: u32) -> Result<(), RealtimeCodecError> {
        info!("Updating bitrate from {} to {} kbps", self.config.bitrate, new_bitrate);
        self.config.bitrate = new_bitrate;
        Ok(())
    }

    pub fn adapt_to_network_conditions(&mut self, stats: &NetworkStats) -> Result<(), RealtimeCodecError> {
        // Adaptive bitrate based on network conditions
        let target_bitrate = if stats.packet_loss > 5.0 {
            // High packet loss - reduce bitrate significantly
            (self.config.bitrate as f32 * 0.5) as u32
        } else if stats.packet_loss > 2.0 {
            // Medium packet loss - reduce bitrate moderately
            (self.config.bitrate as f32 * 0.7) as u32
        } else if stats.latency > 200 {
            // High latency - reduce bitrate to help
            (self.config.bitrate as f32 * 0.8) as u32
        } else if stats.latency < 50 && stats.packet_loss < 0.5 {
            // Good conditions - can increase bitrate
            std::cmp::min((self.config.bitrate as f32 * 1.2) as u32, 8000) // Cap at 8Mbps
        } else {
            self.config.bitrate // Keep current bitrate
        };

        if target_bitrate != self.config.bitrate {
            self.update_bitrate(target_bitrate)?;
        }

        Ok(())
    }

    pub fn get_performance_stats(&self) -> (f64, f64, u64) {
        (self.capture_duration_ms, self.encode_duration_ms, self.frame_count)
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn force_keyframe(&mut self) {
        self.last_keyframe = 0; // Force next frame to be keyframe
    }
}

// Compatibility aliases for existing code
pub type VideoEncoder = RealtimeStreamEncoder;
pub type EncoderConfig = RealtimeConfig;
pub type CodecError = RealtimeCodecError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodecType {
    H264,
}

impl CodecType {
    pub fn from_string(codec: &str) -> Self {
        match codec.to_lowercase().as_str() {
            "h264" | "avc" | "avc1" => Self::H264,
            _ => Self::H264, // H.264 is the only supported codec
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::H264 => "h264",
        }
    }
}

// WebRTC-specific encoder configuration
#[derive(Clone)]
pub struct WebRTCEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub framerate: u32,
    pub use_hardware: bool,
    pub keyframe_interval: u32,
    pub quality_preset: String,
    pub monitor_id: usize,
}

impl Default for WebRTCEncoderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            bitrate: 2000, // 2 Mbps in kbps
            framerate: 30,
            use_hardware: false,
            keyframe_interval: 60,
            quality_preset: "medium".to_string(),
            monitor_id: 0,
        }
    }
}

impl WebRTCEncoderConfig {
    pub fn for_webrtc_streaming(width: u32, height: u32, bitrate: u32, monitor_id: usize) -> Self {
        Self {
            width,
            height,
            bitrate,
            framerate: 30,
            use_hardware: false,
            keyframe_interval: 60,
            quality_preset: "medium".to_string(),
            monitor_id,
        }
    }

    pub fn to_encoder_config(&self) -> RealtimeConfig {
        RealtimeConfig {
            monitor_id: self.monitor_id,
            width: self.width,
            height: self.height,
            bitrate: self.bitrate,
            framerate: self.framerate,
            keyframe_interval: self.keyframe_interval,
            target_latency_ms: 100, // Default target latency
        }
    }
}


