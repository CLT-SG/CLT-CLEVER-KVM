//! VNC Server Implementation
//! 
//! Implements VNC RFB protocol 3.8 server with screen capture,
//! keyboard/mouse input handling, and multi-client support.

use std::sync::{Arc, Mutex};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Read, Write, Result as IoResult};
use log::{info, warn, error, debug};
use parking_lot::RwLock;
use anyhow::{Result, Context};

use crate::core::ScreenCapture;
use super::audio::SeparateAudioStream;

/// VNC server configuration
#[derive(Debug, Clone)]
pub struct VncServerConfig {
    pub port: u16,
    pub monitor_id: usize,
    pub enable_audio: bool,
    pub audio_port: Option<u16>,
    pub max_clients: usize,
    pub password: Option<String>,
}

impl Default for VncServerConfig {
    fn default() -> Self {
        Self {
            port: 5900,
            monitor_id: 0,
            enable_audio: true,
            audio_port: Some(5901),
            max_clients: 10,
            password: None,
        }
    }
}

/// VNC client connection information
#[derive(Debug, Clone)]
pub struct VncClient {
    pub id: usize,
    pub address: String,
    pub connected_at: std::time::Instant,
}

/// Main VNC KVM Server
pub struct VncKvmServer {
    config: VncServerConfig,
    screen_capture: Arc<Mutex<ScreenCapture>>,
    audio_stream: Option<Arc<Mutex<SeparateAudioStream>>>,
    clients: Arc<RwLock<Vec<VncClient>>>,
    running: Arc<RwLock<bool>>,
    listener: Option<Arc<Mutex<TcpListener>>>,
}

impl VncKvmServer {
    /// Create a new VNC KVM server instance
    pub fn new(config: VncServerConfig) -> Result<Self> {
        info!("🖥️  Initializing VNC KVM Server on port {}", config.port);
        
        // Initialize screen capture for the specified monitor
        let screen_capture = match ScreenCapture::new(Some(config.monitor_id)) {
            Ok(sc) => {
                info!("✅ Screen capture initialized for monitor {}", config.monitor_id);
                Arc::new(Mutex::new(sc))
            },
            Err(e) => {
                error!("❌ Failed to initialize screen capture: {}", e);
                return Err(anyhow::anyhow!("Failed to initialize screen capture: {}", e));
            }
        };

        // Initialize audio stream if enabled
        let audio_stream = if config.enable_audio {
            match config.audio_port {
                Some(audio_port) => {
                    info!("🎵 Initializing audio stream on port {}", audio_port);
                    match SeparateAudioStream::new(audio_port) {
                        Ok(stream) => {
                            info!("✅ Audio stream initialized");
                            Some(Arc::new(Mutex::new(stream)))
                        },
                        Err(e) => {
                            warn!("⚠️  Failed to initialize audio stream: {}", e);
                            None
                        }
                    }
                },
                None => {
                    warn!("⚠️  Audio enabled but no audio port specified");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            screen_capture,
            audio_stream,
            clients: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            listener: None,
        })
    }

    /// Start the VNC server
    pub async fn start(&mut self) -> Result<()> {
        // Check if already running
        {
            let running = self.running.read();
            if *running {
                return Err(anyhow::anyhow!("VNC server is already running"));
            }
        }

        info!("🚀 Starting VNC server on port {}", self.config.port);

        // Bind to TCP port
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.config.port))
            .context(format!("Failed to bind to port {}", self.config.port))?;
        
        listener.set_nonblocking(false)?;
        info!("✅ VNC server listening on 0.0.0.0:{}", self.config.port);

        let listener = Arc::new(Mutex::new(listener));
        self.listener = Some(listener.clone());

        // Mark as running
        {
            let mut running = self.running.write();
            *running = true;
        }

        // Start audio stream if configured
        if let Some(audio_stream) = &self.audio_stream {
            let stream = audio_stream.clone();
            tokio::spawn(async move {
                if let Ok(mut stream) = stream.lock() {
                    if let Err(e) = stream.start_streaming().await {
                        error!("❌ Audio streaming error: {}", e);
                    }
                }
            });
        }

        // Accept client connections
        let clients = self.clients.clone();
        let running = self.running.clone();
        let screen_capture = self.screen_capture.clone();
        let max_clients = self.config.max_clients;
        
        tokio::spawn(async move {
            let mut client_id = 0;
            
            loop {
                // Check if still running
                {
                    let is_running = running.read();
                    if !*is_running {
                        info!("🛑 VNC server stopped accepting connections");
                        break;
                    }
                }

                // Check client count
                let current_clients = {
                    let clients_lock = clients.read();
                    clients_lock.len()
                };

                if current_clients >= max_clients {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                // Accept new connection (non-blocking check)
                let listener_lock = listener.lock().unwrap();
                match listener_lock.accept() {
                    Ok((stream, addr)) => {
                        client_id += 1;
                        info!("📥 New VNC client connected: {} (ID: {})", addr, client_id);
                        
                        let client = VncClient {
                            id: client_id,
                            address: addr.to_string(),
                            connected_at: std::time::Instant::now(),
                        };

                        // Add to client list
                        {
                            let mut clients_lock = clients.write();
                            clients_lock.push(client.clone());
                        }

                        // Handle client in separate thread
                        let clients_for_handler = clients.clone();
                        let screen_capture_for_handler = screen_capture.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_vnc_client(stream, client.id, screen_capture_for_handler) {
                                error!("❌ VNC client {} error: {}", client.id, e);
                            }

                            // Remove client from list
                            let mut clients_lock = clients_for_handler.write();
                            clients_lock.retain(|c| c.id != client.id);
                            info!("📤 VNC client {} disconnected", client.id);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connections available, wait a bit
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        error!("❌ Error accepting connection: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        info!("✅ VNC server started successfully");
        Ok(())
    }

    /// Stop the VNC server
    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 Stopping VNC server...");

        // Mark as not running
        {
            let mut running = self.running.write();
            *running = false;
        }

        // Stop audio stream
        if let Some(audio_stream) = &self.audio_stream {
            if let Ok(mut stream) = audio_stream.lock() {
                stream.stop();
            }
        }

        // Clear listener
        self.listener = None;

        // Disconnect all clients
        {
            let mut clients = self.clients.write();
            clients.clear();
        }

        info!("✅ VNC server stopped");
        Ok(())
    }

    /// Get the number of connected clients
    pub fn get_client_count(&self) -> usize {
        let clients = self.clients.read();
        clients.len()
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        let running = self.running.read();
        *running
    }

    /// Get server configuration
    pub fn get_config(&self) -> &VncServerConfig {
        &self.config
    }

    /// Get audio stream URL if available
    pub fn get_audio_url(&self) -> Option<String> {
        self.audio_stream.as_ref().and_then(|stream| {
            stream.lock().ok().map(|s| s.get_stream_url())
        })
    }
}

/// Handle a single VNC client connection
fn handle_vnc_client(
    mut stream: TcpStream,
    client_id: usize,
    screen_capture: Arc<Mutex<ScreenCapture>>,
) -> IoResult<()> {
    debug!("Handling VNC client {}", client_id);

    // Set TCP options for low latency
    stream.set_nodelay(true)?;

    // VNC Protocol Handshake (RFB 3.8)
    // Step 1: Send protocol version
    stream.write_all(b"RFB 003.008\n")?;
    
    // Step 2: Read client protocol version
    let mut client_version = [0u8; 12];
    stream.read_exact(&mut client_version)?;
    
    debug!("Client {} protocol: {:?}", client_id, 
           String::from_utf8_lossy(&client_version));

    // Step 3: Send security types (1 = None)
    stream.write_all(&[1u8, 1u8])?; // 1 security type, type 1 (None)
    
    // Step 4: Read client's chosen security type
    let mut security_type = [0u8; 1];
    stream.read_exact(&mut security_type)?;
    
    if security_type[0] != 1 {
        warn!("Client {} requested unsupported security type: {}", 
              client_id, security_type[0]);
        stream.write_all(&[0u8, 0u8, 0u8, 1u8])?; // Security result: failed
        return Ok(());
    }

    // Step 5: Send security result (success)
    stream.write_all(&[0u8, 0u8, 0u8, 0u8])?;

    // Step 6: Read client init message
    let mut client_init = [0u8; 1];
    stream.read_exact(&mut client_init)?;
    let _shared = client_init[0] != 0;

    // Step 7: Send server init message
    let (width, height) = {
        let capture = screen_capture.lock().unwrap();
        (capture.width as u16, capture.height as u16)
    };

    // Framebuffer width and height
    stream.write_all(&width.to_be_bytes())?;
    stream.write_all(&height.to_be_bytes())?;

    // Pixel format (16 bytes)
    let pixel_format = [
        32, // bits per pixel
        24, // depth
        0,  // big-endian flag
        1,  // true-color flag
        0, 255, // red-max (255)
        0, 255, // green-max (255)
        0, 255, // blue-max (255)
        16, // red-shift
        8,  // green-shift
        0,  // blue-shift
        0, 0, 0, // padding
    ];
    stream.write_all(&pixel_format)?;

    // Name length and name
    let name = b"CLEVER KVM";
    stream.write_all(&(name.len() as u32).to_be_bytes())?;
    stream.write_all(name)?;

    info!("✅ VNC client {} handshake completed", client_id);

    // Main message loop
    loop {
        let mut msg_type = [0u8; 1];
        match stream.read_exact(&mut msg_type) {
            Ok(_) => {
                match msg_type[0] {
                    0 => {
                        // SetPixelFormat
                        let mut msg = [0u8; 19];
                        stream.read_exact(&mut msg)?;
                        debug!("Client {} SetPixelFormat", client_id);
                    }
                    2 => {
                        // SetEncodings
                        let mut header = [0u8; 3];
                        stream.read_exact(&mut header)?;
                        let num_encodings = u16::from_be_bytes([header[1], header[2]]);
                        let mut encodings = vec![0u8; (num_encodings as usize) * 4];
                        stream.read_exact(&mut encodings)?;
                        debug!("Client {} SetEncodings: {} encodings", 
                               client_id, num_encodings);
                    }
                    3 => {
                        // FramebufferUpdateRequest
                        let mut msg = [0u8; 9];
                        stream.read_exact(&mut msg)?;
                        
                        // Send framebuffer update
                        send_framebuffer_update(&mut stream, &screen_capture)?;
                    }
                    4 => {
                        // KeyEvent
                        let mut msg = [0u8; 7];
                        stream.read_exact(&mut msg)?;
                        let down_flag = msg[0] != 0;
                        let key = u32::from_be_bytes([msg[3], msg[4], msg[5], msg[6]]);
                        
                        if let Err(e) = super::input::handle_vnc_keyboard(key, down_flag) {
                            warn!("Failed to handle keyboard event: {}", e);
                        }
                    }
                    5 => {
                        // PointerEvent
                        let mut msg = [0u8; 5];
                        stream.read_exact(&mut msg)?;
                        let button_mask = msg[0];
                        let x = u16::from_be_bytes([msg[1], msg[2]]);
                        let y = u16::from_be_bytes([msg[3], msg[4]]);
                        
                        if let Err(e) = super::input::handle_vnc_mouse(button_mask, x, y) {
                            warn!("Failed to handle mouse event: {}", e);
                        }
                    }
                    6 => {
                        // ClientCutText
                        let mut header = [0u8; 7];
                        stream.read_exact(&mut header)?;
                        let length = u32::from_be_bytes([header[3], header[4], header[5], header[6]]);
                        let mut _text = vec![0u8; length as usize];
                        stream.read_exact(&mut _text)?;
                        debug!("Client {} ClientCutText: {} bytes", client_id, length);
                    }
                    _ => {
                        warn!("Unknown message type from client {}: {}", 
                              client_id, msg_type[0]);
                    }
                }
            }
            Err(e) => {
                debug!("Client {} connection closed: {}", client_id, e);
                break;
            }
        }
    }

    Ok(())
}

/// Send a framebuffer update to the client
fn send_framebuffer_update(
    stream: &mut TcpStream,
    screen_capture: &Arc<Mutex<ScreenCapture>>,
) -> IoResult<()> {
    let (width, height, pixels) = {
        let mut capture = screen_capture.lock().unwrap();
        match capture.capture_frame() {
            Ok(frame) => {
                let width = capture.width as u16;
                let height = capture.height as u16;
                
                // Convert frame data to RGB pixels
                let pixel_data = frame.to_vec();
                (width, height, pixel_data)
            }
            Err(e) => {
                error!("Failed to capture frame: {}", e);
                return Ok(()); // Skip this update
            }
        }
    };

    // FramebufferUpdate message header
    stream.write_all(&[0u8])?; // Message type: FramebufferUpdate
    stream.write_all(&[0u8])?; // Padding
    stream.write_all(&1u16.to_be_bytes())?; // Number of rectangles: 1

    // Rectangle header
    stream.write_all(&0u16.to_be_bytes())?; // x-position: 0
    stream.write_all(&0u16.to_be_bytes())?; // y-position: 0
    stream.write_all(&width.to_be_bytes())?; // width
    stream.write_all(&height.to_be_bytes())?; // height
    stream.write_all(&0i32.to_be_bytes())?; // encoding-type: Raw (0)

    // Send pixel data (Raw encoding)
    stream.write_all(&pixels)?;

    Ok(())
}
