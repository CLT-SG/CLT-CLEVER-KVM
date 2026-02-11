//! VNC Audio Module
//! 
//! Implements dual audio streaming approach:
//! 1. Separate audio stream via RTSP (primary)
//! 2. RFB audio extension (optional)

use anyhow::Result;
use log::{info, warn};
use std::sync::Arc;
use parking_lot::RwLock;

use super::server::VncClient;

/// Separate audio stream implementation
/// Streams audio separately via RTSP for compatibility with MediaMTX
pub struct SeparateAudioStream {
    port: u16,
    running: Arc<RwLock<bool>>,
    stream_url: String,
}

impl SeparateAudioStream {
    /// Create a new separate audio stream
    pub fn new(port: u16) -> Result<Self> {
        info!("🎵 Creating separate audio stream on port {}", port);
        
        // Get local IP for stream URL as fallback
        let local_ip = match local_ip_address::local_ip() {
            Ok(ip) => {
                info!("Using local IP for audio stream: {}", ip);
                ip.to_string()
            },
            Err(e) => {
                warn!("Failed to get local IP address: {}, using 'localhost' as fallback", e);
                "localhost".to_string()
            }
        };

        let stream_url = format!("rtsp://{}:{}/audio", local_ip, port);
        
        Ok(Self {
            port,
            running: Arc::new(RwLock::new(false)),
            stream_url,
        })
    }

    /// Create a new separate audio stream with hostname
    pub fn new_with_hostname(port: u16, hostname: &str) -> Result<Self> {
        info!("🎵 Creating separate audio stream on port {} with hostname {}", port, hostname);
        
        let stream_url = format!("rtsp://{}:{}/audio", hostname, port);
        
        Ok(Self {
            port,
            running: Arc::new(RwLock::new(false)),
            stream_url,
        })
    }

    /// Start streaming audio
    pub async fn start_streaming(&mut self) -> Result<()> {
        // Check if already running
        {
            let running = self.running.read();
            if *running {
                return Err(anyhow::anyhow!("Audio stream is already running"));
            }
        }

        info!("🚀 Starting audio stream on port {}", self.port);

        // Mark as running
        {
            let mut running = self.running.write();
            *running = true;
        }

        // In a full implementation, this would:
        // 1. Initialize audio capture using cpal
        // 2. Encode audio using Opus
        // 3. Stream via RTSP/RTP
        // For now, we'll provide a placeholder that can be extended

        info!("✅ Audio stream started (placeholder - implement full audio capture)");
        info!("📡 Audio stream available at: {}", self.stream_url);
        
        Ok(())
    }

    /// Stop the audio stream
    pub fn stop(&mut self) {
        let mut running = self.running.write();
        if *running {
            info!("🛑 Stopping audio stream");
            *running = false;
        }
    }

    /// Get the RTSP stream URL
    pub fn get_stream_url(&self) -> String {
        self.stream_url.clone()
    }

    /// Check if the stream is running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        let running = self.running.read();
        *running
    }
}

impl Drop for SeparateAudioStream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// RFB Audio Extension implementation (optional)
/// Implements Replit-style audio extension for VNC
#[allow(dead_code)]
pub struct RfbAudioExtension {
    enabled: bool,
    sample_rate: u32,
    channels: u16,
}

#[allow(dead_code)]
impl RfbAudioExtension {
    /// Create a new RFB audio extension
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        info!("🎵 Creating RFB audio extension (sample_rate={}, channels={})", 
              sample_rate, channels);
        
        Ok(Self {
            enabled: false,
            sample_rate,
            channels,
        })
    }

    /// Enable the audio extension
    pub fn enable(&mut self) {
        self.enabled = true;
        info!("✅ RFB audio extension enabled");
    }

    /// Disable the audio extension
    pub fn disable(&mut self) {
        self.enabled = false;
        info!("🛑 RFB audio extension disabled");
    }

    /// Encode audio samples into RFB audio packet
    /// Uses pseudo-encoding: 0x52706C41 (Replit Audio)
    pub fn encode_audio_packet(&self, samples: &[f32]) -> Vec<u8> {
        if !self.enabled {
            return Vec::new();
        }

        // RFB audio packet format:
        // 4 bytes: pseudo-encoding type (0x52706C41)
        // 4 bytes: sample count
        // 4 bytes: sample rate
        // 2 bytes: channels
        // N bytes: audio data (Opus encoded)

        let mut packet = Vec::new();
        
        // Pseudo-encoding type
        packet.extend_from_slice(&0x52706C41u32.to_be_bytes());
        
        // Sample count
        packet.extend_from_slice(&(samples.len() as u32).to_be_bytes());
        
        // Sample rate
        packet.extend_from_slice(&self.sample_rate.to_be_bytes());
        
        // Channels
        packet.extend_from_slice(&self.channels.to_be_bytes());
        
        // Audio data (in a full implementation, this would be Opus encoded)
        // For now, convert f32 samples to i16
        for sample in samples {
            let i16_sample = (*sample * 32767.0) as i16;
            packet.extend_from_slice(&i16_sample.to_le_bytes());
        }
        
        packet
    }

    /// Send audio packet to VNC client
    pub fn send_to_client(&self, _client: &mut VncClient) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // In a full implementation, this would send the audio packet
        // to the VNC client via the established connection
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_separate_audio_stream_creation() {
        let result = SeparateAudioStream::new(5901);
        assert!(result.is_ok());
        
        if let Ok(stream) = result {
            assert!(stream.get_stream_url().contains("5901"));
            assert!(stream.get_stream_url().contains("rtsp://"));
        }
    }

    #[test]
    fn test_rfb_audio_extension_creation() {
        let result = RfbAudioExtension::new(48000, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rfb_audio_packet_encoding() {
        let audio_ext = RfbAudioExtension::new(48000, 2).unwrap();
        let samples = vec![0.5f32, -0.5f32, 0.25f32, -0.25f32];
        
        // Should return empty when disabled
        let packet = audio_ext.encode_audio_packet(&samples);
        assert_eq!(packet.len(), 0);
    }

    #[test]
    fn test_rfb_audio_enabled() {
        let mut audio_ext = RfbAudioExtension::new(48000, 2).unwrap();
        audio_ext.enable();
        
        let samples = vec![0.5f32, -0.5f32];
        let packet = audio_ext.encode_audio_packet(&samples);
        
        // Should have header + audio data
        assert!(packet.len() > 14); // 4+4+4+2 = 14 bytes header
    }
}
