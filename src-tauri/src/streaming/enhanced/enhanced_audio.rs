use anyhow::{Result, Context};
use thiserror::Error;
use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

// Audio encoding dependencies
use opus::{Encoder as OpusEncoder, Channels, Application};
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MediaEngine};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::media::Sample;

/// Enhanced audio encoder errors
#[derive(Error, Debug)]
pub enum AudioEncoderError {
    #[error("Opus encoder initialization failed: {0}")]
    OpusInit(String),
    #[error("WebRTC setup failed: {0}")]
    WebRTC(String),
    #[error("Audio capture failed: {0}")]
    Capture(String),
    #[error("Encoding failed: {0}")]
    Encode(String),
    #[error("Invalid configuration: {0}")]
    Config(String),
}

/// Enhanced audio configuration with Opus and WebRTC support
#[derive(Clone, Debug)]
pub struct EnhancedAudioConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,              // Opus bitrate in bps
    pub frame_duration_ms: u32,    // Frame duration (2.5, 5, 10, 20, 40, 60ms)
    pub application: OpusApplication,
    pub complexity: i32,           // Opus complexity 0-10
    pub use_vbr: bool,            // Variable bitrate
    pub use_fec: bool,            // Forward error correction
    pub use_dtx: bool,            // Discontinuous transmission
    pub enable_webrtc: bool,      // Use WebRTC for streaming
}

#[derive(Clone, Debug)]
pub enum OpusApplication {
    VoIP,      // Optimize for voice
    Audio,     // Optimize for general audio
    LowDelay,  // Optimize for low latency
}

impl From<OpusApplication> for Application {
    fn from(app: OpusApplication) -> Self {
        match app {
            OpusApplication::VoIP => Application::Voip,
            OpusApplication::Audio => Application::Audio,
            OpusApplication::LowDelay => Application::LowDelay,
        }
    }
}

impl Default for EnhancedAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 128000,        // 128 kbps
            frame_duration_ms: 20,  // 20ms frames for good balance
            application: OpusApplication::Audio,
            complexity: 5,          // Medium complexity
            use_vbr: true,         // Variable bitrate for efficiency
            use_fec: true,         // Forward error correction
            use_dtx: false,        // Disable DTX for streaming
            enable_webrtc: true,   // Use WebRTC by default
        }
    }
}

impl EnhancedAudioConfig {
    /// Configuration optimized for voice chat
    pub fn for_voice() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bitrate: 32000,        // 32 kbps for voice
            frame_duration_ms: 20,
            application: OpusApplication::VoIP,
            complexity: 3,         // Lower complexity for real-time
            use_vbr: true,
            use_fec: true,
            use_dtx: true,         // DTX good for voice
            enable_webrtc: true,
        }
    }
    
    /// Configuration optimized for high-quality audio
    pub fn for_high_quality() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 256000,       // 256 kbps for high quality
            frame_duration_ms: 10, // Shorter frames for low latency
            application: OpusApplication::Audio,
            complexity: 8,         // Higher complexity for quality
            use_vbr: true,
            use_fec: true,
            use_dtx: false,
            enable_webrtc: true,
        }
    }
    
    /// Configuration optimized for low latency
    pub fn for_low_latency() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 96000,        // 96 kbps
            frame_duration_ms: 5,  // Very short frames
            application: OpusApplication::LowDelay,
            complexity: 1,         // Lowest complexity for speed
            use_vbr: false,        // CBR for predictable latency
            use_fec: false,        // No FEC for lower latency
            use_dtx: false,
            enable_webrtc: true,
        }
    }
    
    /// Configuration optimized for high-quality streaming
    pub fn for_high_quality_streaming() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 320000,       // 320 kbps for excellent quality
            frame_duration_ms: 20, // Standard frame size
            application: OpusApplication::Audio,
            complexity: 10,        // Maximum complexity for quality
            use_vbr: true,         // VBR for better quality
            use_fec: true,         // FEC for robustness
            use_dtx: true,         // DTX for efficiency
            enable_webrtc: false,  // Direct container streaming
        }
    }
}

/// Audio frame for Opus encoding
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: Vec<f32>,           // Audio samples as f32
    pub sample_rate: u32,
    pub channels: u8,
    pub timestamp: u64,           // Timestamp in microseconds
    pub frame_number: u64,
}

impl AudioFrame {
    pub fn new(sample_rate: u32, channels: u8, frame_duration_ms: u32) -> Self {
        let samples_per_frame = (sample_rate * frame_duration_ms / 1000) as usize;
        let total_samples = samples_per_frame * channels as usize;
        
        Self {
            data: vec![0.0; total_samples],
            sample_rate,
            channels,
            timestamp: 0,
            frame_number: 0,
        }
    }
    
    /// Convert PCM data to audio frame
    pub fn from_pcm_i16(pcm_data: &[i16], sample_rate: u32, channels: u8, frame_number: u64) -> Self {
        let data: Vec<f32> = pcm_data.iter()
            .map(|&sample| sample as f32 / i16::MAX as f32)
            .collect();
        
        Self {
            data,
            sample_rate,
            channels,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            frame_number,
        }
    }
    
    /// Convert to PCM i16 data
    pub fn to_pcm_i16(&self) -> Vec<i16> {
        self.data.iter()
            .map(|&sample| (sample * i16::MAX as f32) as i16)
            .collect()
    }
}

/// Enhanced audio encoder with Opus and WebRTC support
pub struct EnhancedAudioEncoder {
    config: EnhancedAudioConfig,
    opus_encoder: Option<OpusEncoder>,
    
    // WebRTC components
    peer_connection: Option<RTCPeerConnection>,
    audio_track: Option<Arc<TrackLocalStaticSample>>,
    
    // State management
    frame_count: AtomicU64,
    is_active: AtomicBool,
    
    // Performance tracking
    encoding_stats: Arc<AudioEncodingStats>,
    
    // Frame processing
    frame_buffer: Arc<Mutex<Vec<AudioFrame>>>,
    max_buffer_size: usize,
}

/// Audio encoding performance statistics
#[derive(Debug)]
pub struct AudioEncodingStats {
    pub frames_encoded: AtomicU64,
    pub total_bytes_out: AtomicU64,
    pub avg_encode_time_ms: AtomicU64,
    pub packet_loss_rate: AtomicU64, // In percentage * 100
    pub last_update: Mutex<Instant>,
}

impl AudioEncodingStats {
    pub fn new() -> Self {
        Self {
            frames_encoded: AtomicU64::new(0),
            total_bytes_out: AtomicU64::new(0),
            avg_encode_time_ms: AtomicU64::new(0),
            packet_loss_rate: AtomicU64::new(0),
            last_update: Mutex::new(Instant::now()),
        }
    }
    
    pub fn update_frame_stats(&self, bytes_out: usize, encode_time_ms: u64) {
        self.frames_encoded.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_out.fetch_add(bytes_out as u64, Ordering::Relaxed);
        self.avg_encode_time_ms.store(encode_time_ms, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> (u64, u64, u64, f64) {
        (
            self.frames_encoded.load(Ordering::Relaxed),
            self.total_bytes_out.load(Ordering::Relaxed),
            self.avg_encode_time_ms.load(Ordering::Relaxed),
            self.packet_loss_rate.load(Ordering::Relaxed) as f64 / 100.0,
        )
    }
}

impl EnhancedAudioEncoder {
    /// Create a new enhanced audio encoder
    pub fn new(config: EnhancedAudioConfig) -> Result<Self, AudioEncoderError> {
        info!("Initializing enhanced audio encoder: {}Hz, {} channels, {}kbps, {}ms frames",
              config.sample_rate, config.channels, config.bitrate / 1000, config.frame_duration_ms);
        
        // Validate configuration
        if config.sample_rate == 0 || ![8000, 12000, 16000, 24000, 48000].contains(&config.sample_rate) {
            return Err(AudioEncoderError::Config("Invalid sample rate".to_string()));
        }
        if config.channels == 0 || config.channels > 2 {
            return Err(AudioEncoderError::Config("Invalid channel count".to_string()));
        }
        if ![5, 10, 20, 40, 60].contains(&config.frame_duration_ms) {
            return Err(AudioEncoderError::Config("Invalid frame duration".to_string()));
        }
        
        let encoder = Self {
            config: config.clone(),
            opus_encoder: None,
            peer_connection: None,
            audio_track: None,
            frame_count: AtomicU64::new(0),
            is_active: AtomicBool::new(false),
            encoding_stats: Arc::new(AudioEncodingStats::new()),
            frame_buffer: Arc::new(Mutex::new(Vec::new())),
            max_buffer_size: 10, // Buffer up to 10 frames
        };
        
        Ok(encoder)
    }
    
    /// Initialize the Opus encoder
    pub fn initialize_encoder(&mut self) -> Result<(), AudioEncoderError> {
        info!("Initializing Opus encoder...");
        
        let channels = match self.config.channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(AudioEncoderError::Config("Invalid channel count".to_string())),
        };
        
        let mut encoder = OpusEncoder::new(
            self.config.sample_rate,
            channels,
            self.config.application.clone().into(),
        ).map_err(|e| AudioEncoderError::OpusInit(format!("Opus encoder creation failed: {}", e)))?;
        
        // Configure encoder settings
        encoder.set_bitrate(opus::Bitrate::Bits(self.config.bitrate as i32))
            .map_err(|e| AudioEncoderError::OpusInit(format!("Bitrate setting failed: {}", e)))?;
        
        encoder.set_vbr(self.config.use_vbr)
            .map_err(|e| AudioEncoderError::OpusInit(format!("VBR setting failed: {}", e)))?;
        
        // encoder.set_complexity(self.config.complexity)
        //     .map_err(|e| AudioEncoderError::OpusInit(format!("Complexity setting failed: {}", e)))?;
        
        if self.config.use_fec {
            encoder.set_inband_fec(true)
                .map_err(|e| AudioEncoderError::OpusInit(format!("FEC setting failed: {}", e)))?;
        }
        
        if self.config.use_dtx {
            // encoder.set_dtx(true)
            //     .map_err(|e| AudioEncoderError::OpusInit(format!("DTX setting failed: {}", e)))?;
        }
        
        self.opus_encoder = Some(encoder);
        
        // Initialize WebRTC if enabled
        if self.config.enable_webrtc {
            tokio::spawn(async move {
                // This would be async initialization
                // For now, we'll just log it
                info!("WebRTC audio track would be initialized here");
            });
        }
        
        info!("✅ Opus encoder initialized successfully");
        Ok(())
    }
    
    /// Initialize WebRTC peer connection for audio streaming
    pub async fn initialize_webrtc(&mut self) -> Result<(), AudioEncoderError> {
        if !self.config.enable_webrtc {
            return Ok(());
        }
        
        info!("Initializing WebRTC for audio streaming...");
        
        // Create MediaEngine with Opus support
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()
            .map_err(|e| AudioEncoderError::WebRTC(format!("Failed to register codecs: {}", e)))?;
        
        // Create API
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .build();
        
        // Create peer connection
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec!["stun:stun.l.google.com:19302".to_string()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        
        let peer_connection = api.new_peer_connection(config).await
            .map_err(|e| AudioEncoderError::WebRTC(format!("Failed to create peer connection: {}", e)))?;
        
        // Create audio track
        let audio_track = Arc::new(TrackLocalStaticSample::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_string(),
                ..Default::default()
            },
            "audio".to_string(),
            "stream".to_string(),
        ));
        
        // Add track to peer connection
        peer_connection.add_track(audio_track.clone()).await
            .map_err(|e| AudioEncoderError::WebRTC(format!("Failed to add audio track: {}", e)))?;
        
        self.peer_connection = Some(peer_connection);
        self.audio_track = Some(audio_track);
        
        info!("✅ WebRTC audio streaming initialized");
        Ok(())
    }
    
    /// Encode an audio frame
    pub fn encode_frame(&mut self, audio_frame: AudioFrame) -> Result<Option<Vec<u8>>, AudioEncoderError> {
        if self.opus_encoder.is_none() {
            self.initialize_encoder()?;
        }
        
        let start_time = Instant::now();
        let encoder = self.opus_encoder.as_mut().unwrap();
        
        // Calculate frame size in samples
        let frame_size = (self.config.sample_rate * self.config.frame_duration_ms / 1000) as usize;
        let expected_samples = frame_size * self.config.channels as usize;
        
        // Ensure we have the right amount of data
        if audio_frame.data.len() != expected_samples {
            warn!("Audio frame size mismatch: expected {}, got {}", expected_samples, audio_frame.data.len());
            return Ok(None);
        }
        
        // Convert f32 samples to i16 for Opus
        let pcm_data = audio_frame.to_pcm_i16();
        
        // Encode with Opus
        let mut encoded_data = vec![0u8; 4000]; // Max Opus packet size
        let encoded_len = encoder.encode(&pcm_data, &mut encoded_data)
            .map_err(|e| AudioEncoderError::Encode(format!("Opus encoding failed: {}", e)))?;
        
        encoded_data.truncate(encoded_len);
        
        let encode_time = start_time.elapsed();
        
        // Update statistics
        self.encoding_stats.update_frame_stats(encoded_len, encode_time.as_millis() as u64);
        self.frame_count.fetch_add(1, Ordering::Relaxed);
        
        // Send via WebRTC if available
        if let Some(track) = &self.audio_track {
            let sample = Sample {
                data: encoded_data.clone().into(), // Convert Vec<u8> to Bytes
                duration: Duration::from_millis(self.config.frame_duration_ms as u64),
                ..Default::default()
            };
            
            // Send asynchronously
            let track_clone = track.clone();
            tokio::spawn(async move {
                if let Err(e) = track_clone.write_sample(&sample).await {
                    error!("Failed to send audio sample via WebRTC: {}", e);
                }
            });
        }
        
        // Log occasionally
        let frame_num = self.frame_count.load(Ordering::Relaxed);
        if frame_num % 100 == 0 {
            debug!("Audio frame {}: {} bytes, {:.1}ms encode time", 
                   frame_num, encoded_len, encode_time.as_millis());
        }
        
        Ok(Some(encoded_data))
    }
    
    /// Encode PCM audio data directly
    pub fn encode_pcm(&mut self, pcm_data: &[i16], sample_rate: u32, channels: u8) -> Result<Option<Vec<u8>>, AudioEncoderError> {
        let frame_number = self.frame_count.load(Ordering::Relaxed);
        let audio_frame = AudioFrame::from_pcm_i16(pcm_data, sample_rate, channels, frame_number);
        self.encode_frame(audio_frame)
    }
    
    /// Start audio encoding
    pub fn start(&mut self) -> Result<(), AudioEncoderError> {
        self.is_active.store(true, Ordering::Relaxed);
        info!("Audio encoder started");
        Ok(())
    }
    
    /// Stop audio encoding
    pub fn stop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
        info!("Audio encoder stopped");
    }
    
    /// Check if encoder is active
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }
    
    /// Get encoding statistics
    pub fn get_stats(&self) -> (u64, u64, u64, f64) {
        self.encoding_stats.get_stats()
    }
    
    /// Update encoder configuration dynamically
    pub fn update_config(&mut self, new_config: EnhancedAudioConfig) -> Result<(), AudioEncoderError> {
        info!("Updating audio encoder configuration");
        
        // If sample rate or channels changed, reinitialize encoder
        if new_config.sample_rate != self.config.sample_rate ||
           new_config.channels != self.config.channels ||
           new_config.frame_duration_ms != self.config.frame_duration_ms {
            self.config = new_config;
            self.opus_encoder = None; // Will be re-initialized on next encode
        } else {
            // Update encoder settings that can be changed on the fly
            if let Some(encoder) = &mut self.opus_encoder {
                if new_config.bitrate != self.config.bitrate {
                    let _ = encoder.set_bitrate(opus::Bitrate::Bits(new_config.bitrate as i32));
                }
                if new_config.complexity != self.config.complexity {
                    // let _ = encoder.set_complexity(new_config.complexity);
                }
            }
            self.config = new_config;
        }
        
        Ok(())
    }
}

impl Drop for EnhancedAudioEncoder {
    fn drop(&mut self) {
        self.stop();
        info!("Enhanced audio encoder dropped");
    }
}

/// Audio capture utility for system audio
pub struct SystemAudioCapture {
    sample_rate: u32,
    channels: u8,
    frame_duration_ms: u32,
    is_capturing: AtomicBool,
}

impl SystemAudioCapture {
    pub fn new(sample_rate: u32, channels: u8, frame_duration_ms: u32) -> Self {
        Self {
            sample_rate,
            channels,
            frame_duration_ms,
            is_capturing: AtomicBool::new(false),
        }
    }
    
    /// Start capturing system audio (placeholder implementation)
    pub fn start_capture(&self) -> Result<(), AudioEncoderError> {
        info!("Starting system audio capture: {}Hz, {} channels", 
              self.sample_rate, self.channels);
        
        self.is_capturing.store(true, Ordering::Relaxed);
        
        // In a real implementation, this would:
        // 1. Initialize system audio capture (WASAPI on Windows, ALSA/PulseAudio on Linux, CoreAudio on macOS)
        // 2. Set up audio capture callbacks
        // 3. Start capturing audio in a separate thread
        
        info!("✅ System audio capture started (placeholder)");
        Ok(())
    }
    
    /// Stop capturing system audio
    pub fn stop_capture(&self) {
        self.is_capturing.store(false, Ordering::Relaxed);
        info!("System audio capture stopped");
    }
    
    /// Check if currently capturing
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::Relaxed)
    }
    
    /// Generate test audio frame (for testing)
    pub fn generate_test_frame(&self, frame_number: u64) -> AudioFrame {
        let frame_size = (self.sample_rate * self.frame_duration_ms / 1000) as usize;
        let total_samples = frame_size * self.channels as usize;
        
        // Generate a simple sine wave for testing
        let frequency = 440.0; // A4 note
        let mut data = Vec::with_capacity(total_samples);
        
        for i in 0..frame_size {
            let t = i as f32 / self.sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.1; // Low volume
            
            for _ in 0..self.channels {
                data.push(sample);
            }
        }
        
        AudioFrame {
            data,
            sample_rate: self.sample_rate,
            channels: self.channels,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            frame_number,
        }
    }
}
