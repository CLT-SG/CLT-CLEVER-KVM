//! Dedicated Video Service Thread
//!
//! Follows RustDesk's architecture: one dedicated OS thread per display
//! running the capture → dedup → encode → broadcast loop.
//!
//! Key optimizations:
//! - Runs on a dedicated std::thread (not async/tokio)
//! - Frame deduplication via byte comparison (skip encoding unchanged screens)
//! - Pre-allocated YUV buffers (reused across frames)
//! - Frame pacing with sleep-based timing
//! - Broadcast to multiple clients via crossbeam channel

use anyhow::{Result, Context};
use crossbeam_channel::{Sender, Receiver, bounded};
use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::ScreenCapture;
use crate::rdengine::codec::{VpxEncoder, VpxConfig, VpxCodec, EncoderApi, EncodeInput, rgba_to_i420};
use crate::rdengine::protocol::{self, CODEC_VP8, CODEC_VP9};
use crate::rdengine::qos::{QualityControl, QosConfig};

/// Encoded frame ready for sending to clients
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Complete binary message (protocol header + encoded data)
    pub message: Vec<u8>,
    /// Whether this is a keyframe
    pub is_keyframe: bool,
    /// Frame number
    pub frame_number: u64,
    /// Timestamp in ms
    pub timestamp_ms: u64,
}

/// Video service statistics
#[derive(Debug, Default)]
pub struct VideoStats {
    pub frames_captured: AtomicU64,
    pub frames_encoded: AtomicU64,
    pub frames_skipped: AtomicU64, // Duplicate frames skipped
    pub total_bytes: AtomicU64,
    pub avg_capture_us: AtomicU64,
    pub avg_encode_us: AtomicU64,
    pub current_fps: AtomicU32,
}

/// Configuration for the video service
#[derive(Debug, Clone)]
pub struct VideoServiceConfig {
    pub monitor_id: usize,
    pub codec: VpxCodec,
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub bitrate_kbps: u32,
    pub max_subscribers: usize,
}

impl Default for VideoServiceConfig {
    fn default() -> Self {
        Self {
            monitor_id: 0,
            codec: VpxCodec::VP9,
            width: 0, // 0 = auto-detect from monitor
            height: 0,
            framerate: 30,
            bitrate_kbps: 2000,
            max_subscribers: 16,
        }
    }
}

/// The video service manages a dedicated capture → encode thread
pub struct VideoService {
    /// Thread handle
    thread: Option<thread::JoinHandle<()>>,
    /// Stop signal
    running: Arc<AtomicBool>,
    /// Channel to receive encoded frames
    frame_rx: Receiver<VideoFrame>,
    /// Clone of the sender (for stats access)
    frame_tx: Sender<VideoFrame>,
    /// Force keyframe signal
    force_keyframe: Arc<AtomicBool>,
    /// Shared QoS control
    qos: Arc<parking_lot::Mutex<QualityControl>>,
    /// Statistics
    stats: Arc<VideoStats>,
    /// Configuration
    config: VideoServiceConfig,
}

impl VideoService {
    /// Start the video service on a dedicated thread
    pub fn start(config: VideoServiceConfig) -> Result<Self> {
        info!(
            "Starting video service: monitor={}, codec={:?}, {}x{}, {}fps, {}kbps",
            config.monitor_id, config.codec, config.width, config.height,
            config.framerate, config.bitrate_kbps
        );

        let running = Arc::new(AtomicBool::new(true));
        let force_keyframe = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(VideoStats::default());

        // Bounded channel for video frames (like RustDesk's tx_video)
        // Small buffer to prevent buildup — if consumer is slow, frames are dropped
        let (frame_tx, frame_rx) = bounded::<VideoFrame>(4);

        let qos = Arc::new(parking_lot::Mutex::new(QualityControl::new(
            QosConfig::default(),
            config.framerate,
            config.bitrate_kbps,
        )));

        let running_clone = running.clone();
        let force_kf_clone = force_keyframe.clone();
        let stats_clone = stats.clone();
        let qos_clone = qos.clone();
        let config_clone = config.clone();
        let tx = frame_tx.clone();

        // Spawn dedicated OS thread — not async! (Like RustDesk's video_service)
        let thread = thread::Builder::new()
            .name(format!("video-svc-{}", config.monitor_id))
            .spawn(move || {
                if let Err(e) = video_service_loop(
                    config_clone,
                    running_clone,
                    force_kf_clone,
                    stats_clone,
                    qos_clone,
                    tx,
                ) {
                    error!("Video service thread error: {}", e);
                }
                info!("Video service thread exited");
            })
            .context("Failed to spawn video service thread")?;

        Ok(Self {
            thread: Some(thread),
            running,
            frame_rx,
            frame_tx,
            force_keyframe,
            qos,
            stats,
            config,
        })
    }

    /// Get the frame receiver (clone for each client connection)
    pub fn frame_rx(&self) -> &Receiver<VideoFrame> {
        &self.frame_rx
    }

    /// Request a keyframe (called when new client connects or client requests refresh)
    pub fn request_keyframe(&self) {
        self.force_keyframe.store(true, Ordering::Relaxed);
    }

    /// Get QoS control (for updating FPS/bitrate)
    pub fn qos(&self) -> &Arc<parking_lot::Mutex<QualityControl>> {
        &self.qos
    }

    /// Get statistics
    pub fn stats(&self) -> &Arc<VideoStats> {
        &self.stats
    }

    /// Check if the service is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the video service
    pub fn stop(&mut self) {
        info!("Stopping video service for monitor {}", self.config.monitor_id);
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        info!("Video service stopped");
    }
}

impl Drop for VideoService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The main video service loop — runs on a dedicated OS thread
///
/// This is the RustDesk-inspired capture → dedup → convert → encode → send loop.
fn video_service_loop(
    config: VideoServiceConfig,
    running: Arc<AtomicBool>,
    force_keyframe: Arc<AtomicBool>,
    stats: Arc<VideoStats>,
    qos: Arc<parking_lot::Mutex<QualityControl>>,
    frame_tx: Sender<VideoFrame>,
) -> Result<()> {
    // Initialize screen capture
    let mut capture = ScreenCapture::new(Some(config.monitor_id))
        .map_err(|e| anyhow::anyhow!("Failed to initialize screen capture: {}", e))?;

    // Get actual dimensions
    let (width, height) = capture.dimensions();
    let width = if config.width > 0 { config.width } else { width as u32 };
    let height = if config.height > 0 { config.height } else { height as u32 };

    info!("Video service capture initialized: {}x{}", width, height);

    // Initialize VPX encoder
    let vpx_config = VpxConfig {
        codec: config.codec,
        width,
        height,
        bitrate_kbps: config.bitrate_kbps,
        framerate: config.framerate,
        ..VpxConfig::default()
    };
    let mut encoder = VpxEncoder::new(vpx_config)
        .context("Failed to initialize VPX encoder")?;

    // Pre-allocate YUV buffers (REUSED across frames — key optimization!)
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let mut y_buf = vec![0u8; y_size];
    let mut u_buf = vec![0u8; uv_size];
    let mut v_buf = vec![0u8; uv_size];

    // Previous frame buffer for deduplication
    let frame_size = (width * height * 4) as usize;
    let mut prev_frame: Vec<u8> = Vec::with_capacity(frame_size);

    // Codec identifier for protocol
    let codec_id = match config.codec {
        VpxCodec::VP8 => CODEC_VP8,
        VpxCodec::VP9 => CODEC_VP9,
    };

    let mut frame_count: u64 = 0;
    let start_time = Instant::now();
    let mut last_fps_report = Instant::now();
    let mut fps_frame_count: u64 = 0;

    info!("Video service main loop starting");

    while running.load(Ordering::Relaxed) {
        let frame_start = Instant::now();

        // Get target frame interval from QoS
        let spf = qos.lock().spf();

        // 1. CAPTURE
        let capture_start = Instant::now();
        let rgba_data: Vec<u8> = match capture.capture_raw() {
            Ok(data) => data,
            Err(e) => {
                // Don't spam logs on capture failures
                if frame_count % 100 == 0 {
                    warn!("Screen capture failed (frame {}): {}", frame_count, e);
                }
                thread::sleep(Duration::from_millis(16));
                continue;
            }
        };
        let capture_time = capture_start.elapsed();

        stats.frames_captured.fetch_add(1, Ordering::Relaxed);
        stats.avg_capture_us.store(capture_time.as_micros() as u64, Ordering::Relaxed);

        // 2. FRAME DEDUPLICATION — compare with previous frame (RustDesk's would_block_if_equal)
        let frame_changed = if prev_frame.len() == rgba_data.len() {
            // Byte-by-byte comparison (fast — short-circuits on first difference)
            rgba_data != prev_frame
        } else {
            true // Different size = definitely changed
        };

        if !frame_changed && !force_keyframe.load(Ordering::Relaxed) {
            // Frame unchanged — skip encoding entirely
            stats.frames_skipped.fetch_add(1, Ordering::Relaxed);

            // Sleep for frame interval
            let elapsed = frame_start.elapsed();
            if elapsed < spf {
                thread::sleep(spf - elapsed);
            }
            continue;
        }

        // Store current frame for next comparison
        if prev_frame.len() != rgba_data.len() {
            prev_frame.resize(rgba_data.len(), 0);
        }
        prev_frame.copy_from_slice(&rgba_data);

        // 3. COLOR CONVERT — RGBA → I420 (reusing pre-allocated buffers!)
        rgba_to_i420(&rgba_data, width, height, &mut y_buf, &mut u_buf, &mut v_buf);

        // 4. ENCODE — VPX encoding
        let encode_start = Instant::now();
        let timestamp_ms = start_time.elapsed().as_millis() as i64;

        // Check for forced keyframe
        let want_keyframe = force_keyframe.swap(false, Ordering::Relaxed);
        if want_keyframe {
            encoder.force_keyframe();
        }

        let stride_y = width as usize;
        let stride_uv = (width / 2) as usize;

        let packets = match encoder.encode(
            EncodeInput::I420 {
                y: &y_buf,
                u: &u_buf,
                v: &v_buf,
                stride_y,
                stride_u: stride_uv,
                stride_v: stride_uv,
            },
            timestamp_ms,
        ) {
            Ok(packets) => packets,
            Err(e) => {
                warn!("Encode error: {}", e);
                continue;
            }
        };
        let encode_time = encode_start.elapsed();
        stats.avg_encode_us.store(encode_time.as_micros() as u64, Ordering::Relaxed);

        // 5. SEND — Package and broadcast to all clients
        for pkt in packets {
            let message = protocol::encode_video_message(
                codec_id,
                pkt.is_keyframe,
                width,
                height,
                timestamp_ms as u64,
                &pkt.data,
            );

            let msg_size = message.len();
            stats.total_bytes.fetch_add(msg_size as u64, Ordering::Relaxed);
            stats.frames_encoded.fetch_add(1, Ordering::Relaxed);

            let video_frame = VideoFrame {
                message,
                is_keyframe: pkt.is_keyframe,
                frame_number: frame_count,
                timestamp_ms: timestamp_ms as u64,
            };

            // Try to send — if channel is full, drop the frame (backpressure)
            match frame_tx.try_send(video_frame) {
                Ok(_) => {}
                Err(crossbeam_channel::TrySendError::Full(_)) => {
                    debug!("Frame dropped (channel full) — client too slow");
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    // No more receivers — stop the service
                    info!("All frame receivers disconnected, stopping video service");
                    return Ok(());
                }
            }
        }

        frame_count += 1;
        fps_frame_count += 1;

        // FPS reporting (every 5 seconds)
        if last_fps_report.elapsed() >= Duration::from_secs(5) {
            let fps = fps_frame_count as f64 / last_fps_report.elapsed().as_secs_f64();
            stats.current_fps.store(fps as u32, Ordering::Relaxed);

            let skipped = stats.frames_skipped.load(Ordering::Relaxed);
            let captured = stats.frames_captured.load(Ordering::Relaxed);
            let skip_pct = if captured > 0 { 
                (skipped as f64 / captured as f64 * 100.0) as u32 
            } else { 
                0 
            };

            info!(
                "Video service stats: {:.1} fps, capture={}us, encode={}us, skipped={}%, bytes={}KB",
                fps,
                stats.avg_capture_us.load(Ordering::Relaxed),
                stats.avg_encode_us.load(Ordering::Relaxed),
                skip_pct,
                stats.total_bytes.load(Ordering::Relaxed) / 1024,
            );

            fps_frame_count = 0;
            last_fps_report = Instant::now();
        }

        // 6. FRAME PACING — sleep for remainder of frame interval (like RustDesk)
        let elapsed = frame_start.elapsed();
        if elapsed < spf {
            thread::sleep(spf - elapsed);
        }
    }

    info!("Video service loop ended after {} frames", frame_count);
    Ok(())
}
