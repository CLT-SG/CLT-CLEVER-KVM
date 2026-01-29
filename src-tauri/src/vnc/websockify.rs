use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_hdr_async, tungstenite::Message};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tracing::{info, debug, warn, error, trace};
use parking_lot::Mutex as ParkingLotMutex;

/// WebSocket-to-VNC proxy (websockify implementation)
/// 
/// This proxy accepts WebSocket connections from NoVNC clients and forwards
/// them to native VNC servers using the VNC protocol over TCP.
/// 
/// Architecture:
/// ```
/// NoVNC Browser → WebSocket → WebsockifyProxy → TCP/VNC → VNC Server
/// ```
pub struct WebsockifyProxy {
    listen_port: u16,
    target_host: String,
    target_port: u16,
    running: Arc<AtomicBool>,
    listener_handle: Option<JoinHandle<()>>,
    /// Shared list of active connection handles that need to be aborted on stop
    connection_handles: Arc<ParkingLotMutex<Vec<JoinHandle<()>>>>,
}

impl WebsockifyProxy {
    /// Create a new websockify proxy
    /// 
    /// # Arguments
    /// * `listen_port` - Port to listen for WebSocket connections (e.g., 6080)
    /// * `target_host` - VNC server host (e.g., "localhost")
    /// * `target_port` - VNC server port (e.g., 5900)
    pub fn new(listen_port: u16, target_host: String, target_port: u16) -> Self {
        Self {
            listen_port,
            target_host,
            target_port,
            running: Arc::new(AtomicBool::new(false)),
            listener_handle: None,
            connection_handles: Arc::new(ParkingLotMutex::new(Vec::new())),
        }
    }

    /// Start the websockify proxy
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            warn!("Websockify proxy already running on port {}", self.listen_port);
            return Ok(());
        }

        let addr = format!("0.0.0.0:{}", self.listen_port);
        
        info!("Starting websockify proxy...");
        info!("  Listen address: {}", addr);
        info!("  Target VNC: {}:{}", self.target_host, self.target_port);
        
        let listener = TcpListener::bind(&addr)
            .await
            .context(format!("Failed to bind websockify to {}", addr))?;

        info!("✓ Websockify proxy successfully started on port {}", self.listen_port);
        info!("  Ready to accept NoVNC connections");

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let target_host = self.target_host.clone();
        let target_port = self.target_port;
        let listen_port = self.listen_port;
        let connection_handles = self.connection_handles.clone();

        let handle = tokio::spawn(async move {
            let mut connection_count = 0u32;
            
            while running.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, client_addr)) => {
                        connection_count += 1;
                        info!("New WebSocket connection #{} from {} on port {}", 
                              connection_count, client_addr, listen_port);
                        
                        let target_host = target_host.clone();
                        let running_clone = running.clone();
                        let conn_id = connection_count;
                        let connection_handles_clone = connection_handles.clone();
                        
                        // Spawn connection handler and track the handle
                        let conn_handle = tokio::spawn(async move {
                            trace!("Connection #{}: Upgrading to WebSocket protocol", conn_id);
                            
                            match handle_websocket_connection(
                                stream,
                                target_host.clone(),
                                target_port,
                                conn_id,
                                running_clone,
                            ).await {
                                Ok(_) => {
                                    trace!("Connection #{}: Closed cleanly", conn_id);
                                }
                                Err(e) => {
                                    warn!("Connection #{}: Error: {}", conn_id, e);
                                }
                            }
                        });
                        
                        // Store the connection handle for cleanup on stop
                        {
                            let mut handles = connection_handles_clone.lock();
                            // Clean up completed handles to prevent memory leak
                            handles.retain(|h| !h.is_finished());
                            handles.push(conn_handle);
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept WebSocket connection on port {}: {}", listen_port, e);
                    }
                }
            }
            
            info!("Websockify proxy on port {} stopped accepting connections", listen_port);
        });

        self.listener_handle = Some(handle);
        Ok(())
    }

    /// Stop the websockify proxy
    /// 
    /// Note: This is intentionally synchronous to avoid Send trait issues
    /// when the mutex guard is held across await points.
    pub fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            trace!("Websockify proxy on port {} already stopped", self.listen_port);
            return Ok(());
        }

        info!("Stopping websockify proxy on port {}...", self.listen_port);
        self.running.store(false, Ordering::SeqCst);

        // Abort the listener task
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }
        
        // Abort all active connection tasks
        {
            let mut handles = self.connection_handles.lock();
            let active_count = handles.len();
            if active_count > 0 {
                info!("Aborting {} active WebSocket connection(s) on port {}...", active_count, self.listen_port);
                for handle in handles.drain(..) {
                    handle.abort();
                }
            }
        }

        info!("✓ Websockify proxy on port {} stopped successfully", self.listen_port);
        Ok(())
    }

    /// Get the WebSocket URL for NoVNC clients
    /// 
    /// # Arguments
    /// * `hostname` - Hostname or IP address
    /// * `use_tls` - Whether to use WSS (secure WebSocket)
    /// 
    /// # Returns
    /// WebSocket URL like `ws://hostname:6080/` or `wss://hostname:6080/`
    pub fn get_websocket_url(&self, hostname: &str, use_tls: bool) -> String {
        let protocol = if use_tls { "wss" } else { "ws" };
        format!("{}://{}:{}/", protocol, hostname, self.listen_port)
    }

    /// Check if the proxy is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Handle a single WebSocket connection and forward to VNC server
async fn handle_websocket_connection(
    stream: TcpStream,
    target_host: String,
    target_port: u16,
    conn_id: u32,
    running: Arc<AtomicBool>,
) -> Result<()> {
    // Check if we should still be running before starting
    if !running.load(Ordering::SeqCst) {
        return Ok(());
    }
    
    // Upgrade HTTP connection to WebSocket with proper subprotocol negotiation
    // NoVNC requires 'binary' or 'base64' subprotocol in Sec-WebSocket-Protocol header
    trace!("Connection #{}: Upgrading HTTP to WebSocket", conn_id);
    
    let callback = |req: &Request, mut response: Response| {
        // Check for Sec-WebSocket-Protocol header
        if let Some(protocols) = req.headers().get("Sec-WebSocket-Protocol") {
            if let Ok(protocols_str) = protocols.to_str() {
                // Check if client supports 'binary' (preferred) or 'base64'
                let requested: Vec<&str> = protocols_str.split(',').map(|s| s.trim()).collect();
                
                if requested.iter().any(|&p| p == "binary") {
                    // Prefer binary protocol (more efficient)
                    response.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        "binary".parse().unwrap()
                    );
                    info!("Connection #{}: Negotiated 'binary' subprotocol", conn_id);
                } else if requested.iter().any(|&p| p == "base64") {
                    // Fallback to base64 if binary not available
                    response.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        "base64".parse().unwrap()
                    );
                    info!("Connection #{}: Negotiated 'base64' subprotocol", conn_id);
                }
            }
        }
        Ok(response)
    };
    
    let ws_stream = accept_hdr_async(stream, callback)
        .await
        .context("Failed to accept WebSocket connection")?;

    trace!("Connection #{}: WebSocket established, connecting to VNC at {}:{}", 
           conn_id, target_host, target_port);

    // Connect to VNC server
    let vnc_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .context(format!("Failed to connect to VNC server at {}:{}", target_host, target_port))?;

    info!("Connection #{}: NoVNC client connected to VNC server {}:{}", 
          conn_id, target_host, target_port);
    trace!("Connection #{}: Starting bidirectional data forwarding", conn_id);

    // Split WebSocket and TCP streams
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (mut vnc_read, mut vnc_write) = vnc_stream.into_split();

    // Forward WebSocket → VNC (browser to server)
    let conn_id_ws = conn_id;
    let ws_to_vnc = tokio::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Forward binary data to VNC server
                    if let Err(e) = vnc_write.write_all(&data).await {
                        warn!("Connection #{}: Error writing to VNC server: {}", conn_id_ws, e);
                        break;
                    }
                    // Flush to ensure data is sent immediately
                    if let Err(e) = vnc_write.flush().await {
                        warn!("Connection #{}: Error flushing to VNC server: {}", conn_id_ws, e);
                        break;
                    }
                }
                Ok(Message::Text(text)) => {
                    // NoVNC with base64 subprotocol sends text messages
                    // Decode base64 and forward to VNC server
                    if let Ok(data) = base64_decode(&text) {
                        if let Err(e) = vnc_write.write_all(&data).await {
                            warn!("Connection #{}: Error writing base64 data to VNC server: {}", conn_id_ws, e);
                            break;
                        }
                        if let Err(e) = vnc_write.flush().await {
                            warn!("Connection #{}: Error flushing to VNC server: {}", conn_id_ws, e);
                            break;
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    info!("Connection #{}: WebSocket close received: {:?}", conn_id_ws, frame);
                    break;
                }
                Ok(Message::Ping(_)) => {
                    // Ping received, pong is sent automatically by tungstenite
                }
                Ok(_) => {
                    // Ignore pong and other message types
                }
                Err(e) => {
                    warn!("Connection #{}: WebSocket read error: {}", conn_id_ws, e);
                    break;
                }
            }
        }
        
        info!("Connection #{}: WS→VNC forwarding stopped", conn_id_ws);
    });

    // Forward VNC → WebSocket (server to browser)
    let conn_id_vnc = conn_id;
    let vnc_to_ws = tokio::spawn(async move {
        // Larger buffer for screen updates (64KB for better throughput)
        let mut buffer = vec![0u8; 65536];
        
        loop {
            match vnc_read.read(&mut buffer).await {
                Ok(0) => {
                    // Connection closed
                    info!("Connection #{}: VNC server closed connection", conn_id_vnc);
                    break;
                }
                Ok(n) => {
                    // Forward data to WebSocket as binary message
                    let data = buffer[..n].to_vec();
                    
                    if let Err(e) = ws_write.send(Message::Binary(data)).await {
                        warn!("Connection #{}: Error sending to WebSocket: {}", conn_id_vnc, e);
                        break;
                    }
                }
                Err(e) => {
                    warn!("Connection #{}: Error reading from VNC server: {}", conn_id_vnc, e);
                    break;
                }
            }
        }
        
        info!("Connection #{}: VNC→WS forwarding stopped", conn_id_vnc);
        
        // Send close message
        let _ = ws_write.send(Message::Close(None)).await;
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = ws_to_vnc => {
            trace!("Connection #{}: WS→VNC task completed", conn_id);
        }
        _ = vnc_to_ws => {
            trace!("Connection #{}: VNC→WS task completed", conn_id);
        }
    }

    trace!("Connection #{}: Bidirectional forwarding ended", conn_id);
    Ok(())
}

/// Decode base64 string to bytes (for base64 subprotocol support)
fn base64_decode(input: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.decode(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websockify_proxy_new() {
        let proxy = WebsockifyProxy::new(6080, "localhost".to_string(), 5900);
        assert!(!proxy.is_running());
    }

    #[test]
    fn test_get_websocket_url() {
        let proxy = WebsockifyProxy::new(6080, "localhost".to_string(), 5900);
        
        let url = proxy.get_websocket_url("workstation-1", false);
        assert_eq!(url, "ws://workstation-1:6080/");
        
        let url_tls = proxy.get_websocket_url("workstation-1", true);
        assert_eq!(url_tls, "wss://workstation-1:6080/");
    }
}
