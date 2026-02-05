use anyhow::{Result, Context};
use thiserror::Error;
use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::core::ScreenCapture;
use crate::streaming::{
    YUV420Encoder, YUV420Config, YUV420EncoderError,
    SystemAudioCapture,
};

/// Integrated streaming handler errors
#[derive(Error, Debug)]
pub enum IntegratedStreamError {
    #[error("Video encoder error: {0}")]
    Video(#[from] YUV420EncoderError),
    // #[error("Enhanced video encoder error: {0}")]
    // EnhancedVideo(#[from] VideoEncoderError),
    // #[error("Audio encoder error: {0}")]
    // Audio(#[from] AudioEncoderError),
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Streaming configuration combining video and audio
#[derive(Clone, Debug)]
pub struct IntegratedStreamConfig {
    pub video: YUV420Config,
    // pub audio: EnhancedAudioConfig,
    pub enable_audio: bool,
    pub monitor_id: usize,
    pub adaptive_quality: bool,
    pub max_bandwidth_kbps: u32, // Maximum total bandwidth
    pub video_bitrate: u32, // Convenience field for video bitrate
}

impl Default for IntegratedStreamConfig {
    fn default() -> Self {
        Self {
            video: YUV420Config::default(),
            // audio: EnhancedAudioConfig::default(),
            enable_audio: false, // Temporarily disabled
            monitor_id: 0,
            adaptive_quality: true,
            max_bandwidth_kbps: 5000, // 5 Mbps total
            video_bitrate: 2000, // 2 Mbps for video
        }
    }
}

impl IntegratedStreamConfig {
    /// Configuration for high quality streaming
    pub fn high_quality(monitor_id: usize) -> Self {
        Self {
            video: YUV420Config {
                width: 1920,
                height: 1080,
                framerate: 30,
                bitrate: 4000, // 4 Mbps for video
                quality: 15,   // High quality
                keyframe_interval: 60,
                monitor_id,
                use_h264_container: true,
                enable_audio: true,
                opus_bitrate: 256000, // 256 kbps for audio
                temporal_layers: 1,
                spatial_layers: 1,
            },
            // audio: EnhancedAudioConfig::for_high_quality(),
            enable_audio: false, // Temporarily disabled
            monitor_id,
            adaptive_quality: true,
            max_bandwidth_kbps: 5000,
            video_bitrate: 4000,
        }
    }
    
    /// Configuration for balanced streaming
    pub fn balanced(monitor_id: usize) -> Self {
        Self {
            video: YUV420Config {
                width: 1920,
                height: 1080,
                framerate: 24,
                bitrate: 2000, // 2 Mbps for video
                quality: 25,   // Balanced quality
                keyframe_interval: 72, // 3 seconds at 24fps
                monitor_id,
                use_h264_container: true,
                enable_audio: true,
                opus_bitrate: 128000, // 128 kbps for audio
                temporal_layers: 1,
                spatial_layers: 1,
            },
            // audio: EnhancedAudioConfig::default(),
            enable_audio: false, // Temporarily disabled
            monitor_id,
            adaptive_quality: true,
            max_bandwidth_kbps: 3000,
            video_bitrate: 2000,
        }
    }
    
    /// Configuration for low latency streaming
    pub fn low_latency(monitor_id: usize) -> Self {
        Self {
            video: YUV420Config {
                width: 1280,
                height: 720,
                framerate: 60,
                bitrate: 1500, // 1.5 Mbps for video
                quality: 35,   // Lower quality for speed
                keyframe_interval: 60, // 1 second at 60fps
                monitor_id,
                use_h264_container: true, // H.264 for low latency
                enable_audio: true,
                opus_bitrate: 96000, // 96 kbps for audio
                temporal_layers: 1,
                spatial_layers: 1,
            },
            // audio: EnhancedAudioConfig::for_low_latency(),
            enable_audio: false, // Disable audio for lowest latency
            monitor_id,
            adaptive_quality: true,
            max_bandwidth_kbps: 2000,
            video_bitrate: 1500,
        }
    }

    /// Configuration for H.264 streaming with audio
    pub fn h264_with_audio(monitor_id: usize) -> Self {
        Self {
            video: YUV420Config {
                width: 1920,
                height: 1080,
                framerate: 30,
                bitrate: 6000, // 6 Mbps for high quality H.264 video
                quality: 12,   // Very high quality
                keyframe_interval: 90, // 3 seconds at 30fps
                monitor_id,
                use_h264_container: true,
                enable_audio: true,
                opus_bitrate: 320000, // 320 kbps for high-quality audio
                temporal_layers: 2,
                spatial_layers: 1,
            },
            enable_audio: false, // Temporarily disabled
            monitor_id,
            adaptive_quality: true,
            max_bandwidth_kbps: 7000,
            video_bitrate: 6000,
        }
    }

    /// Configuration for H.264 video-only streaming
    pub fn h264_video_only(monitor_id: usize) -> Self {
        Self {
            video: YUV420Config {
                width: 1920,
                height: 1080,
                framerate: 60,
                bitrate: 8000, // 8 Mbps for very high quality H.264 video
                quality: 10,   // Excellent quality
                keyframe_interval: 120, // 2 seconds at 60fps
                monitor_id,
                use_h264_container: true,
                enable_audio: false,
                opus_bitrate: 0, // No audio
                temporal_layers: 3, // More temporal layers for smooth playback
                spatial_layers: 1,
            },
            enable_audio: false,
            monitor_id,
            adaptive_quality: true,
            max_bandwidth_kbps: 9000,
            video_bitrate: 8000,
        }
    }
}

/// Stream packet types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamPacket {
    #[serde(rename = "video_frame")]
    VideoFrame {
        data: Vec<u8>,
        width: u32,
        height: u32,
        frame_number: u64,
        is_keyframe: bool,
        timestamp: u64,
        format: String, // "h264"
    },
    
    #[serde(rename = "audio_frame")]
    AudioFrame {
        data: Vec<u8>,
        sample_rate: u32,
        channels: u8,
        frame_number: u64,
        timestamp: u64,
        format: String, // "opus"
    },
    
    #[serde(rename = "stream_info")]
    StreamInfo {
        video_config: VideoStreamInfo,
        audio_config: Option<AudioStreamInfo>,
        server_info: ServerInfo,
    },
    
    #[serde(rename = "quality_update")]
    QualityUpdate {
        video_bitrate: u32,
        audio_bitrate: u32,
        framerate: u32,
    },
    
    #[serde(rename = "ping")]
    Ping { timestamp: u64 },
    
    #[serde(rename = "pong")]
    Pong { timestamp: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStreamInfo {
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub bitrate: u32,
    pub codec: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStreamInfo {
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub codec: String,
    pub frame_duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub hostname: String,
    pub monitor_count: usize,
    pub current_monitor: usize,
    pub capabilities: Vec<String>,
}

/// Integrated streaming handler with YUV420 video and Opus audio
pub struct IntegratedStreamHandler {
    config: IntegratedStreamConfig,
    video_encoder: Arc<Mutex<YUV420Encoder>>,
    // audio_encoder: Option<Arc<Mutex<EnhancedAudioEncoder>>>,
    audio_capture: Option<Arc<SystemAudioCapture>>,
    
    // State management
    is_active: Arc<AtomicBool>,
    frame_count: Arc<AtomicU64>,
    
    // Performance monitoring
    stream_stats: Arc<StreamStats>,
    
    // Adaptive quality control
    quality_controller: Arc<Mutex<QualityController>>,
}

/// Streaming performance statistics
#[derive(Debug)]
pub struct StreamStats {
    pub video_frames_sent: AtomicU64,
    pub audio_frames_sent: AtomicU64,
    pub total_bytes_sent: AtomicU64,
    pub current_video_bitrate: AtomicU64,
    pub current_audio_bitrate: AtomicU64,
    pub avg_fps: AtomicU64,
    pub client_latency_ms: AtomicU64,
    pub last_update: Mutex<Instant>,
}

impl StreamStats {
    pub fn new() -> Self {
        Self {
            video_frames_sent: AtomicU64::new(0),
            audio_frames_sent: AtomicU64::new(0),
            total_bytes_sent: AtomicU64::new(0),
            current_video_bitrate: AtomicU64::new(0),
            current_audio_bitrate: AtomicU64::new(0),
            avg_fps: AtomicU64::new(0),
            client_latency_ms: AtomicU64::new(0),
            last_update: Mutex::new(Instant::now()),
        }
    }
    
    pub fn update_video_stats(&self, bytes_sent: usize) {
        self.video_frames_sent.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_sent.fetch_add(bytes_sent as u64, Ordering::Relaxed);
    }
    
    pub fn update_audio_stats(&self, bytes_sent: usize) {
        self.audio_frames_sent.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_sent.fetch_add(bytes_sent as u64, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64, u64, u64) {
        (
            self.video_frames_sent.load(Ordering::Relaxed),
            self.audio_frames_sent.load(Ordering::Relaxed),
            self.total_bytes_sent.load(Ordering::Relaxed),
            self.current_video_bitrate.load(Ordering::Relaxed),
            self.current_audio_bitrate.load(Ordering::Relaxed),
            self.avg_fps.load(Ordering::Relaxed),
            self.client_latency_ms.load(Ordering::Relaxed),
        )
    }
}

/// Adaptive quality controller
#[derive(Debug)]
pub struct QualityController {
    pub target_bandwidth: u32,
    pub current_bandwidth: u32,
    pub adaptation_history: Vec<QualityAdjustment>,
    pub last_adjustment: Instant,
    pub adjustment_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct QualityAdjustment {
    pub timestamp: Instant,
    pub video_bitrate: u32,
    pub audio_bitrate: u32,
    pub reason: String,
}

impl QualityController {
    pub fn new(target_bandwidth: u32) -> Self {
        Self {
            target_bandwidth,
            current_bandwidth: 0,
            adaptation_history: Vec::new(),
            last_adjustment: Instant::now(),
            adjustment_interval: Duration::from_secs(5), // Adjust at most every 5 seconds
        }
    }
    
    pub fn should_adjust(&self, current_latency: u64, packet_loss: f64) -> bool {
        let time_since_last = self.last_adjustment.elapsed();
        time_since_last >= self.adjustment_interval && 
        (current_latency > 200 || packet_loss > 2.0 || self.current_bandwidth > self.target_bandwidth)
    }
    
    pub fn calculate_adjustment(&mut self, current_latency: u64, packet_loss: f64, current_video_bitrate: u32, current_audio_bitrate: u32) -> Option<QualityAdjustment> {
        if !self.should_adjust(current_latency, packet_loss) {
            return None;
        }
        
        let (new_video_bitrate, new_audio_bitrate, reason) = if current_latency > 500 || packet_loss > 10.0 {
            // Severe network issues - aggressive reduction
            (
                (current_video_bitrate as f32 * 0.5) as u32,
                (current_audio_bitrate as f32 * 0.7) as u32,
                "Severe network conditions".to_string()
            )
        } else if current_latency > 200 || packet_loss > 5.0 {
            // Moderate network issues
            (
                (current_video_bitrate as f32 * 0.7) as u32,
                (current_audio_bitrate as f32 * 0.8) as u32,
                "Moderate network conditions".to_string()
            )
        } else if current_latency < 50 && packet_loss < 1.0 && self.current_bandwidth < self.target_bandwidth * 80 / 100 {
            // Good network conditions - can increase quality
            (
                std::cmp::min((current_video_bitrate as f32 * 1.2) as u32, self.target_bandwidth * 80 / 100),
                std::cmp::min((current_audio_bitrate as f32 * 1.1) as u32, 256000),
                "Good network conditions".to_string()
            )
        } else {
            return None; // No adjustment needed
        };
        
        let adjustment = QualityAdjustment {
            timestamp: Instant::now(),
            video_bitrate: new_video_bitrate,
            audio_bitrate: new_audio_bitrate,
            reason,
        };
        
        self.adaptation_history.push(adjustment.clone());
        self.last_adjustment = Instant::now();
        
        // Keep only last 10 adjustments
        if self.adaptation_history.len() > 10 {
            self.adaptation_history.remove(0);
        }
        
        Some(adjustment)
    }
}

impl IntegratedStreamHandler {
    /// Create a new integrated streaming handler
    pub fn new(config: IntegratedStreamConfig) -> Result<Self, IntegratedStreamError> {
        info!("Initializing integrated stream handler for monitor {}", config.monitor_id);
        info!("Video: {}x{} @ {}fps, {}kbps", 
              config.video.width, config.video.height, config.video.framerate, config.video.bitrate);
        if config.enable_audio {
            // info!("Audio: {}Hz, {} channels, {}kbps", 
            //       config.audio.sample_rate, config.audio.channels, config.audio.bitrate / 1000);
            info!("Audio: temporarily disabled");
        }
        
        // Create video encoder
        let video_encoder = YUV420Encoder::new(config.video.clone())?;
        
        // Create audio encoder if enabled
        // let audio_encoder = if config.enable_audio {
        //     let encoder = EnhancedAudioEncoder::new(config.audio.clone())?;
        //     Some(Arc::new(Mutex::new(encoder)))
        // } else {
        //     None
        // };
        
        // Create audio capture if enabled
        let audio_capture = if config.enable_audio {
            // let capture = SystemAudioCapture::new(
            //     config.audio.sample_rate,
            //     config.audio.channels,
            //     config.audio.frame_duration_ms,
            // );
            // Some(Arc::new(capture))
            None // Temporarily disabled
        } else {
            None
        };
        
        let handler = Self {
            config: config.clone(),
            video_encoder: Arc::new(Mutex::new(video_encoder)),
            // audio_encoder,
            audio_capture,
            is_active: Arc::new(AtomicBool::new(false)),
            frame_count: Arc::new(AtomicU64::new(0)),
            stream_stats: Arc::new(StreamStats::new()),
            quality_controller: Arc::new(Mutex::new(QualityController::new(config.max_bandwidth_kbps))),
        };
        
        Ok(handler)
    }
    
    /// Handle WebSocket connection with integrated streaming
    pub async fn handle_connection(
        mut self,
        mut websocket: WebSocket,
        mut stop_rx: Option<tokio::sync::broadcast::Receiver<()>>,
    ) {
        info!("🎬 Starting integrated YUV420 + Opus streaming session");
        
        // Initialize encoders
        if let Err(e) = self.initialize_encoders().await {
            error!("Failed to initialize encoders: {}", e);
            let _ = websocket.close().await;
            return;
        }
        
        // Send initial stream info
        if let Err(e) = self.send_stream_info(&mut websocket).await {
            error!("Failed to send stream info: {}", e);
            let _ = websocket.close().await;
            return;
        }
        
        self.is_active.store(true, Ordering::Relaxed);
        
        // Create channels for frame processing
        let (video_tx, mut video_rx) = mpsc::unbounded_channel();
        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        
        // Start video capture loop
        let video_handler = self.start_video_capture_loop(video_tx);
        
        // Start audio capture loop if enabled - temporarily disabled
        let audio_handler: Option<tokio::task::JoinHandle<()>> = if self.config.enable_audio {
            // Some(self.start_audio_capture_loop(audio_tx))
            None // Temporarily disabled
        } else {
            None
        };
        
        // Main streaming loop
        loop {
            tokio::select! {
                // Handle stop signal
                _ = async {
                    if let Some(ref mut stop_rx_ref) = stop_rx {
                        let _ = stop_rx_ref.recv().await;
                    } else {
                        // If no stop receiver, wait indefinitely
                        futures_util::future::pending::<()>().await;
                    }
                } => {
                    info!("Stop signal received, ending stream");
                    break;
                }
                
                // Handle video frames
                frame_data = video_rx.recv() => {
                    if let Some(frame_data) = frame_data {
                        if let Err(e) = self.send_video_frame(&mut websocket, frame_data).await {
                            error!("Failed to send video frame: {}", e);
                            break;
                        }
                    }
                }
                
                // Handle audio frames - temporarily disabled
                // frame_data = async {
                //     if let Some(ref mut rx) = audio_rx.as_mut() {
                //         rx.recv().await
                //     } else {
                //         futures_util::future::pending().await
                //     }
                // } => {
                //     if let Some(frame_data) = frame_data {
                //         if let Err(e) = self.send_audio_frame(&mut websocket, frame_data).await {
                //             error!("Failed to send audio frame: {}", e);
                //             // Don't break on audio errors, continue with video
                //         }
                //     }
                // }
                
                // Handle incoming WebSocket messages
                msg = websocket.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_client_message(&text).await {
                                warn!("Error handling client message: {}", e);
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            info!("Client disconnected");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Cleanup
        self.is_active.store(false, Ordering::Relaxed);
        
        // Wait for handlers to finish
        let _ = video_handler.await;
        if let Some(handler) = audio_handler {
            let _ = handler.await;
        }
        
        let _ = websocket.close().await;
        info!("🎬 Integrated streaming session ended");
    }
    
    /// Initialize video and audio encoders
    async fn initialize_encoders(&mut self) -> Result<(), IntegratedStreamError> {
        // Initialize video encoder
        {
            let mut video_encoder = self.video_encoder.lock();
            video_encoder.initialize_encoder()?;
        }
        
        // Initialize audio encoder if enabled - temporarily disabled
        // if let Some(ref audio_encoder) = self.audio_encoder {
        //     let mut encoder = audio_encoder.lock();
        //     encoder.initialize_encoder()?;
        //     
        //     // Initialize WebRTC for audio if enabled
        //     if self.config.audio.enable_webrtc {
        //         encoder.initialize_webrtc().await?;
        //     }
        //     
        //     encoder.start()?;
        // }
        
        // Start audio capture if enabled - temporarily disabled
        // if let Some(ref audio_capture) = self.audio_capture {
        //     audio_capture.start_capture()?;
        // }
        
        info!("✅ All encoders initialized successfully");
        Ok(())
    }
    
    /// Send initial stream information
    async fn send_stream_info(&self, websocket: &mut WebSocket) -> Result<(), IntegratedStreamError> {
        let video_config = VideoStreamInfo {
            width: self.config.video.width,
            height: self.config.video.height,
            framerate: self.config.video.framerate,
            bitrate: self.config.video.bitrate,
            codec: "H264".to_string(),
            format: "h264".to_string(),
        };
        
        let audio_config = if self.config.enable_audio {
            // Some(AudioStreamInfo {
            //     sample_rate: self.config.audio.sample_rate,
            //     channels: self.config.audio.channels,
            //     bitrate: self.config.audio.bitrate,
            //     codec: "Opus".to_string(),
            //     frame_duration_ms: self.config.audio.frame_duration_ms,
            // })
            None // Temporarily disabled
        } else {
            None
        };
        
        let server_info = ServerInfo {
            hostname: gethostname::gethostname().to_string_lossy().to_string(),
            monitor_count: ScreenCapture::get_all_monitors().map(|m| m.len()).unwrap_or(1),
            current_monitor: self.config.monitor_id,
            capabilities: vec![
                "h264".to_string(),
                "hardware_acceleration".to_string(),
                if self.config.enable_audio { "opus_audio" } else { "no_audio" }.to_string(),
                "adaptive_quality".to_string(),
                "native_gdi_capture".to_string(),
                "yuv420_output".to_string(),
            ],
        };
        
        let stream_info = StreamPacket::StreamInfo {
            video_config,
            audio_config,
            server_info,
        };
        
        let json = serde_json::to_string(&stream_info)
            .map_err(|e| IntegratedStreamError::WebSocket(format!("JSON serialization failed: {}", e)))?;
        
        websocket.send(Message::Text(json)).await
            .map_err(|e| IntegratedStreamError::WebSocket(format!("Failed to send stream info: {}", e)))?;
        
        info!("📡 Stream info sent to client");
        Ok(())
    }
    
    /// Start video capture loop
    fn start_video_capture_loop(&self, tx: mpsc::UnboundedSender<Vec<u8>>) -> tokio::task::JoinHandle<()> {
        let video_encoder = Arc::clone(&self.video_encoder);
        let is_active = Arc::clone(&self.is_active);
        let stream_stats = Arc::clone(&self.stream_stats);
        let framerate = self.config.video.framerate;
        
        tokio::spawn(async move {
            let frame_duration = Duration::from_millis(1000 / framerate as u64);
            let mut last_keyframe = Instant::now();
            let keyframe_interval = Duration::from_secs(2); // Force keyframe every 2 seconds
            
            info!("🎥 Video capture loop started at {}fps", framerate);
            
            while is_active.load(Ordering::Relaxed) {
                let start_time = Instant::now();
                
                // Force keyframe if needed
                let force_keyframe = last_keyframe.elapsed() >= keyframe_interval;
                if force_keyframe {
                    last_keyframe = Instant::now();
                }
                
                // Capture and encode frame
                match video_encoder.lock().capture_and_encode(force_keyframe) {
                    Ok(Some(encoded_data)) => {
                        stream_stats.update_video_stats(encoded_data.len());
                        
                        if let Err(e) = tx.send(encoded_data) {
                            error!("Failed to send video frame to channel: {}", e);
                            break;
                        }
                    }
                    Ok(None) => {
                        debug!("No video frame generated");
                    }
                    Err(e) => {
                        error!("Video encoding error: {}", e);
                        // Continue on encoding errors
                    }
                }
                
                // Frame rate limiting
                let elapsed = start_time.elapsed();
                if elapsed < frame_duration {
                    tokio::time::sleep(frame_duration - elapsed).await;
                }
            }
            
            info!("🎥 Video capture loop ended");
        })
    }
    
    /// Start audio capture loop - temporarily disabled
    // fn start_audio_capture_loop(&self, tx: mpsc::UnboundedSender<Vec<u8>>) -> tokio::task::JoinHandle<()> {
    //     let audio_encoder = self.audio_encoder.as_ref().unwrap().clone();
    //     let audio_capture = self.audio_capture.as_ref().unwrap().clone();
    //     let is_active = Arc::clone(&self.is_active);
    //     let stream_stats = Arc::clone(&self.stream_stats);
    //     let frame_duration_ms = self.config.audio.frame_duration_ms;
    //     
    //     tokio::spawn(async move {
    //         let frame_duration = Duration::from_millis(frame_duration_ms as u64);
    //         let mut frame_number = 0u64;
    //         
    //         info!("🎵 Audio capture loop started with {}ms frames", frame_duration_ms);
    //         
    //         while is_active.load(Ordering::Relaxed) {
    //             let start_time = Instant::now();
    //             
    //             // Generate test audio frame (in real implementation, capture system audio)
    //             let audio_frame = audio_capture.generate_test_frame(frame_number);
    //             frame_number += 1;
    //             
    //             // Encode audio frame
    //             match audio_encoder.lock().encode_frame(audio_frame) {
    //                 Ok(Some(encoded_data)) => {
    //                     stream_stats.update_audio_stats(encoded_data.len());
    //                     
    //                     if let Err(e) = tx.send(encoded_data) {
    //                         error!("Failed to send audio frame to channel: {}", e);
    //                         break;
    //                     }
    //                 }
    //                 Ok(None) => {
    //                     debug!("No audio frame generated");
    //                 }
    //                 Err(e) => {
    //                     error!("Audio encoding error: {}", e);
    //                     // Continue on encoding errors
    //                 }
    //             }
    //             
    //             // Frame rate limiting
    //             let elapsed = start_time.elapsed();
    //             if elapsed < frame_duration {
    //                 tokio::time::sleep(frame_duration - elapsed).await;
    //             }
    //         }
    //         
    //         info!("🎵 Audio capture loop ended");
    //     })
    // }
    
    /// Send video frame to client
    async fn send_video_frame(&self, websocket: &mut WebSocket, frame_data: Vec<u8>) -> Result<(), IntegratedStreamError> {
        let frame_number = self.frame_count.fetch_add(1, Ordering::Relaxed);
        
        let packet = StreamPacket::VideoFrame {
            data: frame_data,
            width: self.config.video.width,
            height: self.config.video.height,
            frame_number,
            is_keyframe: frame_number % self.config.video.keyframe_interval as u64 == 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            format: "h264".to_string(),
        };
        
        let json = serde_json::to_string(&packet)
            .map_err(|e| IntegratedStreamError::WebSocket(format!("JSON serialization failed: {}", e)))?;
        
        websocket.send(Message::Text(json)).await
            .map_err(|e| IntegratedStreamError::WebSocket(format!("Failed to send video frame: {}", e)))?;
        
        Ok(())
    }
    
    /// Send audio frame to client - temporarily disabled
    // async fn send_audio_frame(&self, websocket: &mut WebSocket, frame_data: Vec<u8>) -> Result<(), IntegratedStreamError> {
    //     let packet = StreamPacket::AudioFrame {
    //         data: frame_data,
    //         sample_rate: self.config.audio.sample_rate,
    //         channels: self.config.audio.channels,
    //         frame_number: 0, // Audio frames don't need frame numbers
    //         timestamp: std::time::SystemTime::now()
    //             .duration_since(std::time::UNIX_EPOCH)
    //             .unwrap_or_default()
    //             .as_micros() as u64,
    //         format: "opus".to_string(),
    //     };
    //     
    //     let json = serde_json::to_string(&packet)
    //         .map_err(|e| IntegratedStreamError::WebSocket(format!("JSON serialization failed: {}", e)))?;
    //     
    //     websocket.send(Message::Text(json)).await
    //         .map_err(|e| IntegratedStreamError::WebSocket(format!("Failed to send audio frame: {}", e)))?;
    //     
    //     Ok(())
    // }
    
    /// Handle client messages
    async fn handle_client_message(&self, message: &str) -> Result<(), IntegratedStreamError> {
        // Parse client message (ping, quality requests, etc.)
        // This is a placeholder for actual message handling
        debug!("Received client message: {}", message);
        Ok(())
    }
    
    /// Get streaming statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64, u64, u64) {
        self.stream_stats.get_stats()
    }
}
