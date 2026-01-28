//! WebSocket Audio Streaming Module
//! 
//! Implements low-latency audio streaming over WebSocket with Opus encoding.
//! Replaces the previous RTSP-based audio architecture for reduced latency.

use tokio_tungstenite::{accept_async, tungstenite::Message};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Encoder, Application, Channels};
use std::sync::Arc;
use parking_lot::RwLock;
use anyhow::{Result, Context};
use log::{info, warn, error, debug};
use tokio::net::TcpListener;

/// WebSocket audio streamer for low-latency audio transmission
pub struct WebSocketAudioStreamer {
    port: u16,
    encoder: Arc<RwLock<Encoder>>,
    clients: Arc<RwLock<Vec<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>>,
    running: Arc<RwLock<bool>>,
    #[allow(dead_code)]
    stream_url: String,
}

impl WebSocketAudioStreamer {
    /// Create a new WebSocket audio streamer
    pub fn new(port: u16, hostname: Option<String>) -> Result<Self> {
        info!("🎵 Creating WebSocket audio streamer on port {}", port);
        
        // Initialize Opus encoder for low-latency audio
        // 48kHz sample rate, stereo, low delay mode
        let encoder = Encoder::new(48000, Channels::Stereo, Application::LowDelay)
            .context("Failed to create Opus encoder")?;
        
        let host = hostname.unwrap_or_else(|| {
            gethostname::gethostname()
                .into_string()
                .unwrap_or_else(|_| "localhost".into())
        });
        
        let stream_url = format!("ws://{}:{}/audio", host, port);
        
        Ok(Self {
            port,
            encoder: Arc::new(RwLock::new(encoder)),
            clients: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            stream_url,
        })
    }
    
    /// Start the WebSocket audio streamer
    pub async fn start(&mut self) -> Result<()> {
        // Check if already running
        {
            let running = self.running.read();
            if *running {
                return Err(anyhow::anyhow!("Audio streamer is already running"));
            }
        }
        
        info!("🚀 Starting WebSocket audio streamer on port {}", self.port);
        
        // Mark as running
        {
            let mut running = self.running.write();
            *running = true;
        }
        
        // Start WebSocket server for client connections
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await
            .context(format!("Failed to bind to port {}", self.port))?;
        
        info!("✅ WebSocket audio server listening on 0.0.0.0:{}", self.port);
        info!("📡 Audio stream available at: {}", self.stream_url);
        
        // Accept client connections in background
        let clients = self.clients.clone();
        let running = self.running.clone();
        tokio::spawn(async move {
            while *running.read() {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("New WebSocket connection from: {}", addr);
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                info!("✅ WebSocket client connected: {}", addr);
                                clients.write().push(ws_stream);
                            }
                            Err(e) => {
                                warn!("Failed to accept WebSocket connection: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error accepting TCP connection: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        // Start audio capture and streaming
        self.start_audio_capture()?;
        
        Ok(())
    }
    
    /// Start capturing and streaming audio
    fn start_audio_capture(&self) -> Result<()> {
        let host = cpal::default_host();
        
        // Try to get default output device (for loopback/system audio)
        let device = host.default_output_device()
            .or_else(|| host.default_input_device())
            .context("No audio device available")?;
        
        info!("🎤 Using audio device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
        
        // Get default config
        let config = device.default_output_config()
            .or_else(|_| device.default_input_config())
            .context("Failed to get audio config")?;
        
        info!("🎵 Audio config: {:?}", config);
        
        // Build audio stream
        let clients = self.clients.clone();
        let encoder = self.encoder.clone();
        let running = self.running.clone();
        
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let config: cpal::StreamConfig = config.into();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !*running.read() {
                            return;
                        }
                        
                        // Encode with Opus
                        let mut output = vec![0u8; 4000];
                        let encoder = encoder.read();
                        match encoder.encode_float(data, &mut output) {
                            Ok(size) => {
                                output.truncate(size);
                                
                                // Broadcast to all connected clients
                                let mut clients = clients.write();
                                clients.retain_mut(|client| {
                                    // Try to send, remove client if send fails
                                    match futures_util::executor::block_on(
                                        async { client.send(Message::Binary(output.clone())).await }
                                    ) {
                                        Ok(_) => true,
                                        Err(e) => {
                                            debug!("Client disconnected: {}", e);
                                            false
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Failed to encode audio: {}", e);
                            }
                        }
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                ).context("Failed to build input stream")?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format"));
            }
        };
        
        stream.play().context("Failed to start audio stream")?;
        
        // Keep stream alive by forgetting it (it will be cleaned up when the process exits)
        std::mem::forget(stream);
        
        info!("✅ Audio capture started");
        
        Ok(())
    }
    
    /// Stop the audio streamer
    pub fn stop(&mut self) {
        info!("🛑 Stopping WebSocket audio streamer");
        
        let mut running = self.running.write();
        *running = false;
        
        // Disconnect all clients
        let mut clients = self.clients.write();
        clients.clear();
        
        info!("✅ WebSocket audio streamer stopped");
    }
    
    /// Check if the streamer is running
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }
    
    /// Get the WebSocket stream URL
    pub fn get_stream_url(&self) -> String {
        self.stream_url.clone()
    }
    
    /// Get the number of connected clients
    pub fn get_client_count(&self) -> usize {
        self.clients.read().len()
    }
}

impl Drop for WebSocketAudioStreamer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_audio_streamer_creation() {
        let result = WebSocketAudioStreamer::new(6900, Some("test-host".to_string()));
        assert!(result.is_ok());
        
        if let Ok(streamer) = result {
            assert_eq!(streamer.port, 6900);
            assert!(streamer.get_stream_url().contains("6900"));
            assert!(streamer.get_stream_url().contains("ws://"));
            assert!(!streamer.is_running());
        }
    }
    
    #[test]
    fn test_websocket_audio_url_format() {
        let streamer = WebSocketAudioStreamer::new(6901, Some("workstation-1".to_string())).unwrap();
        assert_eq!(streamer.get_stream_url(), "ws://workstation-1:6901/audio");
    }
}
