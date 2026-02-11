//! Audio Capture and Encoding Service
//!
//! Uses cpal for cross-platform audio capture and Opus for encoding.
//! Follows RustDesk's approach:
//! - System audio capture (loopback on supported platforms)
//! - Opus encoding with LowDelay application mode
//! - 10ms frame size for minimal latency
//! - Broadcast to all connected clients

#![allow(dead_code)]

use anyhow::{Result, Context};
use crossbeam_channel::{Sender, Receiver, bounded};
use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::rdengine::protocol;

/// Encoded audio frame ready for sending
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Complete binary message (protocol header + opus data)
    pub message: Vec<u8>,
    pub timestamp_ms: u64,
}

/// Audio service configuration
#[derive(Debug, Clone)]
pub struct AudioServiceConfig {
    /// Sample rate (48000 recommended for Opus)
    pub sample_rate: u32,
    /// Number of channels (1=mono, 2=stereo)
    pub channels: u16,
    /// Opus bitrate in bps
    pub opus_bitrate: u32,
    /// Frame duration in ms (10ms like RustDesk for lowest latency)
    pub frame_duration_ms: u32,
}

impl Default for AudioServiceConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            opus_bitrate: 128_000,
            frame_duration_ms: 10,
        }
    }
}

impl AudioServiceConfig {
    pub fn low_latency() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1,
            opus_bitrate: 64_000,
            frame_duration_ms: 10,
        }
    }

    pub fn high_quality() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            opus_bitrate: 128_000,
            frame_duration_ms: 10,
        }
    }
}

/// Audio service manages audio capture → encode → broadcast
pub struct AudioService {
    thread: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    audio_rx: Receiver<AudioFrame>,
    config: AudioServiceConfig,
}

impl AudioService {
    /// Start the audio service
    pub fn start(config: AudioServiceConfig) -> Result<Self> {
        info!(
            "Starting audio service: {}Hz, {} channels, {} bps, {}ms frames",
            config.sample_rate, config.channels, config.opus_bitrate, config.frame_duration_ms
        );

        let running = Arc::new(AtomicBool::new(true));
        let (audio_tx, audio_rx) = bounded::<AudioFrame>(8);

        let running_clone = running.clone();
        let config_clone = config.clone();

        let thread = thread::Builder::new()
            .name("audio-service".to_string())
            .spawn(move || {
                if let Err(e) = audio_service_loop(config_clone, running_clone, audio_tx) {
                    error!("Audio service error: {}", e);
                }
                info!("Audio service thread exited");
            })
            .context("Failed to spawn audio service thread")?;

        Ok(Self {
            thread: Some(thread),
            running,
            audio_rx,
            config,
        })
    }

    /// Get the audio frame receiver
    pub fn audio_rx(&self) -> &Receiver<AudioFrame> {
        &self.audio_rx
    }

    /// Check if the service is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the audio service
    pub fn stop(&mut self) {
        info!("Stopping audio service");
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for AudioService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Audio capture and encode loop
fn audio_service_loop(
    config: AudioServiceConfig,
    running: Arc<AtomicBool>,
    audio_tx: Sender<AudioFrame>,
) -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();

    // Try to get output device for loopback capture (system audio)
    // On Linux, this requires PulseAudio monitor device
    let device = match host.default_output_device() {
        Some(dev) => {
            info!("Audio capture device: {}", dev.name().unwrap_or_default());
            dev
        }
        None => {
            // Fall back to input device (microphone)
            match host.default_input_device() {
                Some(dev) => {
                    info!("Using input device for audio: {}", dev.name().unwrap_or_default());
                    dev
                }
                None => {
                    warn!("No audio devices found — audio service disabled");
                    // Keep thread alive but idle
                    while running.load(Ordering::Relaxed) {
                        thread::sleep(std::time::Duration::from_secs(1));
                    }
                    return Ok(());
                }
            }
        }
    };

    // Get supported config
    let supported_config = device.default_input_config()
        .or_else(|_| device.default_output_config())
        .context("No supported audio config")?;

    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();

    info!("Audio config: {}Hz, {} channels, format={:?}",
        sample_rate, channels, supported_config.sample_format());

    // Initialize Opus encoder
    let opus_channels = if config.channels >= 2 {
        opus::Channels::Stereo
    } else {
        opus::Channels::Mono
    };

    let mut opus_encoder = opus::Encoder::new(
        config.sample_rate,
        opus_channels,
        opus::Application::LowDelay, // Like RustDesk — minimum latency
    ).context("Failed to create Opus encoder")?;

    opus_encoder.set_bitrate(opus::Bitrate::Bits(config.opus_bitrate as i32))
        .context("Failed to set Opus bitrate")?;

    // Frame size: samples per channel per frame
    let frame_size = (config.sample_rate * config.frame_duration_ms / 1000) as usize;
    let total_samples_per_frame = frame_size * config.channels as usize;

    // Accumulator for audio samples
    let sample_buffer = Arc::new(parking_lot::Mutex::new(Vec::<f32>::with_capacity(total_samples_per_frame * 4)));
    let sample_buffer_clone = sample_buffer.clone();

    // Build the cpal input stream
    let stream_config = cpal::StreamConfig {
        channels: config.channels,
        sample_rate: cpal::SampleRate(config.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let err_fn = |err: cpal::StreamError| {
        warn!("Audio stream error: {}", err);
    };

    let stream = device.build_input_stream(
        &stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut buf = sample_buffer_clone.lock();
            buf.extend_from_slice(data);
        },
        err_fn,
        None,
    ).context("Failed to build audio input stream")?;

    stream.play().context("Failed to start audio stream")?;
    info!("Audio capture stream started");

    let start_time = Instant::now();
    let mut opus_output = vec![0u8; 4000]; // Opus output buffer

    while running.load(Ordering::Relaxed) {
        // Wait for enough samples
        thread::sleep(std::time::Duration::from_millis(config.frame_duration_ms as u64));

        let samples: Vec<f32> = {
            let mut buf = sample_buffer.lock();
            if buf.len() < total_samples_per_frame {
                continue;
            }
            // Take exactly one frame's worth
            let frame: Vec<f32> = buf.drain(..total_samples_per_frame).collect();
            // Keep buffer from growing unbounded
            if buf.len() > total_samples_per_frame * 10 {
                let excess = buf.len() - total_samples_per_frame * 2;
                buf.drain(..excess);
            }
            frame
        };

        // Convert f32 → i16 for Opus
        let pcm: Vec<i16> = samples.iter()
            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        // Encode with Opus
        let encoded_size = match opus_encoder.encode(&pcm, &mut opus_output) {
            Ok(size) => size,
            Err(e) => {
                warn!("Opus encode error: {}", e);
                continue;
            }
        };

        let timestamp_ms = start_time.elapsed().as_millis() as u64;

        // Package as protocol message
        let message = protocol::encode_audio_message(
            config.sample_rate,
            config.channels as u8,
            timestamp_ms,
            &opus_output[..encoded_size],
        );

        let audio_frame = AudioFrame {
            message,
            timestamp_ms,
        };

        // Send to clients (drop if channel is full)
        match audio_tx.try_send(audio_frame) {
            Ok(_) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                debug!("Audio frame dropped (channel full)");
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                info!("Audio receivers disconnected");
                break;
            }
        }
    }

    info!("Audio service loop ended");
    Ok(())
}
