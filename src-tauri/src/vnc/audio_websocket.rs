//! WebSocket Audio Streaming Module
//! 
//! Implements low-latency audio streaming over WebSocket with Opus encoding.
//! Replaces the previous RTSP-based audio architecture for reduced latency.
//!
//! # TLS Support
//! 
//! This module supports both ws:// and wss:// URL schemes. For production use with TLS:
//! - Use a reverse proxy (Nginx, HAProxy, Caddy) for TLS termination
//! - Or enable use_tls flag to use direct TLS (requires certificate configuration)
//!
//! # Architecture Notes
//! 
//! This implementation uses a lockless queue to avoid blocking in audio callbacks.
//! Audio data flows from the capture callback through a crossbeam channel to an
//! async task that handles WebSocket broadcasting.

use tokio_tungstenite::{accept_async, tungstenite::Message};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Encoder, Application, Channels};
use std::sync::Arc;
use parking_lot::RwLock;
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug, trace};
use tokio::net::TcpListener;
use crossbeam_channel::{unbounded, Sender, Receiver};
use futures_util::SinkExt;

/// WebSocket audio streamer for low-latency audio transmission
pub struct WebSocketAudioStreamer {
    port: u16,
    clients: Arc<RwLock<Vec<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>>,
    running: Arc<RwLock<bool>>,
    stream_url: String,
    audio_tx: Option<Sender<Vec<u8>>>,
    listener_handle: Option<tokio::task::JoinHandle<()>>,
    broadcaster_handle: Option<tokio::task::JoinHandle<()>>,
    use_tls_url: bool, // Flag to control URL scheme (ws:// vs wss://)
}

impl WebSocketAudioStreamer {
    /// Create a new WebSocket audio streamer
    pub fn new(port: u16, hostname: Option<String>) -> Result<Self> {
        Self::new_with_tls_url(port, hostname, false)
    }
    
    /// Create a new WebSocket audio streamer with TLS URL scheme
    /// 
    /// Note: This only changes the URL scheme to wss://. For actual TLS encryption:
    /// - Use a reverse proxy (recommended for production)
    /// - Or configure TLS termination at the network layer
    pub fn new_with_tls_url(port: u16, hostname: Option<String>, use_tls_url: bool) -> Result<Self> {
        info!("🎵 Creating WebSocket audio streamer on port {} (TLS URL: {})", port, use_tls_url);
        
        let host = hostname.unwrap_or_else(|| {
            gethostname::gethostname()
                .into_string()
                .unwrap_or_else(|_| "localhost".into())
        });
        
        let protocol = if use_tls_url { "wss" } else { "ws" };
        let stream_url = format!("{}://{}:{}/audio", protocol, host, port);
        
        Ok(Self {
            port,
            clients: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            stream_url,
            audio_tx: None,
            listener_handle: None,
            broadcaster_handle: None,
            use_tls_url,
        })
    }
    
    /// Get the stream URL
    pub fn get_stream_url(&self) -> &str {
        &self.stream_url
    }
    
    /// Start the WebSocket audio streamer
    pub async fn start(&mut self) -> Result<()> {
        // Check if already running with atomic-like operation
        {
            let mut running = self.running.write();
            if *running {
                return Err(anyhow::anyhow!("Audio streamer is already running"));
            }
            *running = true;
        }
        
        info!("🚀 Starting WebSocket audio streamer on port {}", self.port);
        
        // Start WebSocket server for client connections
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await
            .context(format!("Failed to bind to port {}", self.port))?;
        
        info!("✅ WebSocket audio server listening on 0.0.0.0:{}", self.port);
        info!("📡 Audio stream available at: {}", self.stream_url);
        
        // Create channel for audio data
        let (audio_tx, audio_rx) = unbounded::<Vec<u8>>();
        self.audio_tx = Some(audio_tx);
        
        // Start WebSocket broadcaster task
        let clients = self.clients.clone();
        let running = self.running.clone();
        let broadcaster_handle = tokio::spawn(async move {
            Self::broadcast_audio_task(audio_rx, clients, running).await;
        });
        self.broadcaster_handle = Some(broadcaster_handle);
        
        // Accept client connections in background
        let clients = self.clients.clone();
        let running = self.running.clone();
        let listener_handle = tokio::spawn(async move {
            Self::accept_clients_task(listener, clients, running).await;
        });
        self.listener_handle = Some(listener_handle);
        
        // Start audio capture and streaming
        self.start_audio_capture()?;
        
        Ok(())
    }
    
    /// Task to accept WebSocket client connections
    async fn accept_clients_task(
        listener: TcpListener,
        clients: Arc<RwLock<Vec<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>>,
        running: Arc<RwLock<bool>>,
    ) {
        while *running.read() {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    trace!("New WebSocket connection from: {}", addr);
                    match accept_async(stream).await {
                        Ok(ws_stream) => {
                            info!("✅ WebSocket audio client connected: {}", addr);
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
    }
    
    /// Task to broadcast audio data to all connected clients
    async fn broadcast_audio_task(
        audio_rx: Receiver<Vec<u8>>,
        clients: Arc<RwLock<Vec<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>>,
        running: Arc<RwLock<bool>>,
    ) {
        while *running.read() {
            // Try to receive audio data with timeout
            match audio_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(audio_data) => {
                    // Get the list of clients (take ownership temporarily to avoid holding lock across await)
                    let mut clients_list = {
                        let mut clients_guard = clients.write();
                        std::mem::take(&mut *clients_guard)
                    };
                    
                    let mut indices_to_remove = Vec::new();
                    
                    for (idx, client) in clients_list.iter_mut().enumerate() {
                        // Try to send, mark for removal if send fails
                        if let Err(e) = client.send(Message::Binary(audio_data.clone())).await {
                            trace!("Audio client {} disconnected: {}", idx, e);
                            indices_to_remove.push(idx);
                        }
                    }
                    
                    // Remove disconnected clients (in reverse order to maintain indices)
                    for idx in indices_to_remove.into_iter().rev() {
                        clients_list.remove(idx);
                    }
                    
                    // Put the clients list back
                    {
                        let mut clients_guard = clients.write();
                        *clients_guard = clients_list;
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Normal timeout, continue
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    // Channel closed, exit loop
                    break;
                }
            }
        }
    }
    
    /// Start capturing and streaming audio
    /// 
    /// Note: System audio loopback must be configured separately.
    /// - Linux: Use PulseAudio monitor devices
    /// - Windows: Use WASAPI loopback or third-party tools
    /// - macOS: Use BlackHole or similar virtual audio devices
    fn start_audio_capture(&self) -> Result<()> {
        let host = cpal::default_host();
        
        // Try to get an input device (microphone by default)
        // For system audio, platform-specific loopback configuration is required
        let device = host.default_input_device()
            .context("No input audio device available")?;
        
        info!("🎤 Using audio device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
        
        // Get default config
        let config = device.default_input_config()
            .context("Failed to get audio config")?;
        
        info!("🎵 Audio config: {:?}", config);
        
        // Create Opus encoder (48kHz, stereo, low delay)
        let mut encoder = Encoder::new(48000, Channels::Stereo, Application::LowDelay)
            .context("Failed to create Opus encoder")?;
        
        // Get channel sender for audio data
        let audio_tx = self.audio_tx.clone()
            .ok_or_else(|| anyhow::anyhow!("Audio channel not initialized"))?;
        
        let running = self.running.clone();
        
        // Build audio stream with non-blocking callback
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let config: cpal::StreamConfig = config.into();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !*running.read() {
                            return;
                        }
                        
                        // Encode with Opus (dedicated encoder per callback)
                        // Use a larger buffer to accommodate various frame sizes
                        let mut output = vec![0u8; 8000];
                        match encoder.encode_float(data, &mut output) {
                            Ok(size) => {
                                output.truncate(size);
                                
                                // Send to broadcaster task via non-blocking channel
                                // If send fails (channel full), skip this frame
                                let _ = audio_tx.try_send(output);
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
        
        // Keep stream alive - it will be dropped when the struct is dropped
        // Note: This is intentionally leaked to keep the audio stream running
        std::mem::forget(stream);
        
        info!("✅ Audio capture started");
        
        Ok(())
    }
    
    /// Stop the audio streamer
    pub fn stop(&mut self) {
        info!("🛑 Stopping WebSocket audio streamer");
        
        // Set running to false to signal tasks to stop
        {
            let mut running = self.running.write();
            *running = false;
        }
        
        // Drop audio channel sender to signal broadcaster task
        self.audio_tx = None;
        
        // Wait for tasks to complete (with timeout)
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.broadcaster_handle.take() {
            handle.abort();
        }
        
        // Disconnect all clients
        let mut clients = self.clients.write();
        clients.clear();
        
        info!("✅ WebSocket audio streamer stopped");
    }
    
    /// Check if the streamer is running
    pub fn is_running(&self) -> bool {
        *self.running.read()
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
