//! Adaptive Quality of Service Control
//!
//! Inspired by RustDesk's video_qos.rs:
//! - Smoothed RTT estimation using min-window approach
//! - Adaptive FPS based on network delay
//! - Adaptive bitrate based on congestion
//! - Frame ACK-based flow control

use log::{debug, info};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Quality control configuration
#[derive(Debug, Clone)]
pub struct QosConfig {
    /// Minimum FPS to never go below
    pub min_fps: u32,
    /// Maximum FPS to never exceed
    pub max_fps: u32,
    /// Minimum bitrate in kbps
    pub min_bitrate_kbps: u32,
    /// Maximum bitrate in kbps
    pub max_bitrate_kbps: u32,
    /// How often to adjust quality (in seconds)
    pub adjustment_interval_secs: f64,
    /// RTT threshold (ms) above which we reduce quality
    pub high_latency_threshold_ms: u32,
    /// RTT threshold (ms) below which we increase quality
    pub low_latency_threshold_ms: u32,
}

impl Default for QosConfig {
    fn default() -> Self {
        Self {
            min_fps: 5,
            max_fps: 60,
            min_bitrate_kbps: 400,
            max_bitrate_kbps: 12000,
            adjustment_interval_secs: 3.0,
            high_latency_threshold_ms: 100,
            low_latency_threshold_ms: 30,
        }
    }
}

/// Quality control state — manages adaptive FPS and bitrate
pub struct QualityControl {
    config: QosConfig,
    /// Current target FPS
    current_fps: AtomicU32,
    /// Current target bitrate
    current_bitrate_kbps: AtomicU32,
    /// RTT samples (sliding window)
    rtt_samples: VecDeque<u32>,
    /// Smoothed RTT estimate (microseconds stored as u64 for atomic)
    smoothed_rtt_ms: AtomicU32,
    /// Last quality adjustment time
    last_adjustment: Instant,
    /// Frame send count (for flow control)
    frames_sent: AtomicU64,
    /// Frame ACK count
    frames_acked: AtomicU64,
    /// Quality level description
    quality_level: String,
}

impl QualityControl {
    pub fn new(config: QosConfig, initial_fps: u32, initial_bitrate: u32) -> Self {
        Self {
            current_fps: AtomicU32::new(initial_fps.clamp(config.min_fps, config.max_fps)),
            current_bitrate_kbps: AtomicU32::new(initial_bitrate.clamp(config.min_bitrate_kbps, config.max_bitrate_kbps)),
            rtt_samples: VecDeque::with_capacity(60),
            smoothed_rtt_ms: AtomicU32::new(0),
            last_adjustment: Instant::now(),
            frames_sent: AtomicU64::new(0),
            frames_acked: AtomicU64::new(0),
            quality_level: "balanced".to_string(),
            config,
        }
    }

    /// Record an RTT sample (called when pong is received)
    pub fn record_rtt(&mut self, rtt_ms: u32) {
        self.rtt_samples.push_back(rtt_ms);
        if self.rtt_samples.len() > 60 {
            self.rtt_samples.pop_front();
        }

        // Smoothed RTT: 50% min + 50% window min (RustDesk approach)
        let min_rtt = self.rtt_samples.iter().copied().min().unwrap_or(rtt_ms);
        let recent_min = self.rtt_samples.iter().rev().take(10).copied().min().unwrap_or(rtt_ms);
        let smoothed = (min_rtt + recent_min) / 2;
        self.smoothed_rtt_ms.store(smoothed, Ordering::Relaxed);
    }

    /// Get current smoothed RTT in ms
    pub fn rtt_ms(&self) -> u32 {
        self.smoothed_rtt_ms.load(Ordering::Relaxed)
    }

    /// Get current target FPS
    pub fn fps(&self) -> u32 {
        self.current_fps.load(Ordering::Relaxed)
    }

    /// Get current target bitrate
    pub fn bitrate_kbps(&self) -> u32 {
        self.current_bitrate_kbps.load(Ordering::Relaxed)
    }

    /// Get the frame interval (seconds per frame)
    pub fn spf(&self) -> Duration {
        let fps = self.fps().max(1);
        Duration::from_micros(1_000_000 / fps as u64)
    }

    /// Record that a frame was sent
    pub fn frame_sent(&self) {
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a frame ACK was received
    pub fn frame_acked(&self) {
        self.frames_acked.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if we should adjust quality (called periodically)
    /// Returns (new_fps, new_bitrate, changed)
    pub fn maybe_adjust(&mut self) -> (u32, u32, bool) {
        let now = Instant::now();
        if now.duration_since(self.last_adjustment).as_secs_f64() < self.config.adjustment_interval_secs {
            return (self.fps(), self.bitrate_kbps(), false);
        }
        self.last_adjustment = now;

        let rtt = self.rtt_ms();
        let old_fps = self.fps();
        let old_bitrate = self.bitrate_kbps();

        let (new_fps, new_bitrate) = if rtt > self.config.high_latency_threshold_ms {
            // High latency — reduce quality
            let fps = (old_fps * 8 / 10).max(self.config.min_fps); // Reduce by 20%
            let bitrate = (old_bitrate * 8 / 10).max(self.config.min_bitrate_kbps);
            self.quality_level = "low".to_string();
            (fps, bitrate)
        } else if rtt < self.config.low_latency_threshold_ms && rtt > 0 {
            // Low latency (LAN) — increase quality
            let fps = (old_fps * 11 / 10).min(self.config.max_fps); // Increase by 10%
            let bitrate = (old_bitrate * 11 / 10).min(self.config.max_bitrate_kbps);
            self.quality_level = "high".to_string();
            (fps, bitrate)
        } else {
            self.quality_level = "balanced".to_string();
            (old_fps, old_bitrate)
        };

        let changed = new_fps != old_fps || new_bitrate != old_bitrate;
        if changed {
            self.current_fps.store(new_fps, Ordering::Relaxed);
            self.current_bitrate_kbps.store(new_bitrate, Ordering::Relaxed);
            info!(
                "QoS adjusted: fps {}→{}, bitrate {}→{} kbps (RTT={}ms, level={})",
                old_fps, new_fps, old_bitrate, new_bitrate, rtt, self.quality_level
            );
        }

        (new_fps, new_bitrate, changed)
    }

    /// Set FPS directly (from client request)
    pub fn set_fps(&self, fps: u32) {
        let clamped = fps.clamp(self.config.min_fps, self.config.max_fps);
        self.current_fps.store(clamped, Ordering::Relaxed);
    }

    /// Set bitrate directly (from client request)
    pub fn set_bitrate(&self, bitrate_kbps: u32) {
        let clamped = bitrate_kbps.clamp(self.config.min_bitrate_kbps, self.config.max_bitrate_kbps);
        self.current_bitrate_kbps.store(clamped, Ordering::Relaxed);
    }

    /// Get quality level string
    pub fn quality_level(&self) -> &str {
        &self.quality_level
    }
}
