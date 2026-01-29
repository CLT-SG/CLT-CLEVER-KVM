use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{info, debug, warn, error};

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

        let handle = tokio::spawn(async move {
            let mut connection_count = 0u32;
            
            while running.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, client_addr)) => {
                        connection_count += 1;
                        info!("New WebSocket connection #{} from {} on port {}", 
                              connection_count, client_addr, listen_port);
                        
                        let target_host = target_host.clone();
                        let conn_id = connection_count;
                        
                        tokio::spawn(async move {
                            debug!("Connection #{}: Upgrading to WebSocket protocol", conn_id);
                            
                            match handle_websocket_connection(
                                stream,
                                target_host.clone(),
                                target_port,
                                conn_id
                            ).await {
                                Ok(_) => {
                                    info!("Connection #{}: Closed cleanly", conn_id);
                                }
                                Err(e) => {
                                    warn!("Connection #{}: Error: {}", conn_id, e);
                                }
                            }
                        });
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
            debug!("Websockify proxy on port {} already stopped", self.listen_port);
            return Ok(());
        }

        info!("Stopping websockify proxy on port {}...", self.listen_port);
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
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
) -> Result<()> {
    // Upgrade HTTP connection to WebSocket
    debug!("Connection #{}: Upgrading HTTP to WebSocket", conn_id);
    
    let ws_stream = accept_async(stream)
        .await
        .context("Failed to accept WebSocket")?;

    debug!("Connection #{}: WebSocket established, connecting to VNC at {}:{}", 
           conn_id, target_host, target_port);

    // Connect to VNC server
    let vnc_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .context(format!("Failed to connect to VNC server at {}:{}", target_host, target_port))?;

    info!("Connection #{}: Successfully connected to VNC server {}:{}", 
          conn_id, target_host, target_port);
    debug!("Connection #{}: Starting bidirectional data forwarding", conn_id);

    // Split WebSocket and TCP streams
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (mut vnc_read, mut vnc_write) = vnc_stream.into_split();

    // Forward WebSocket → VNC (browser to server)
    let conn_id_ws = conn_id;
    let ws_to_vnc = tokio::spawn(async move {
        let mut byte_count = 0u64;
        let mut message_count = 0u32;
        
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Forward binary data to VNC server
                    let data_len = data.len();
                    byte_count += data_len as u64;
                    message_count += 1;
                    
                    if message_count % 100 == 0 {
                        debug!("Connection #{}: WS→VNC forwarded {} messages ({} bytes)", 
                               conn_id_ws, message_count, byte_count);
                    }
                    
                    if let Err(e) = vnc_write.write_all(&data).await {
                        warn!("Connection #{}: Error writing to VNC server: {}", conn_id_ws, e);
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    info!("Connection #{}: WebSocket close received: {:?}", conn_id_ws, frame);
                    break;
                }
                Ok(Message::Ping(_)) => {
                    debug!("Connection #{}: WebSocket ping received", conn_id_ws);
                }
                Ok(_) => {
                    // Ignore text, pong, and other message types
                }
                Err(e) => {
                    warn!("Connection #{}: WebSocket read error: {}", conn_id_ws, e);
                    break;
                }
            }
        }
        
        info!("Connection #{}: WS→VNC forwarding stopped (total: {} messages, {} bytes)", 
              conn_id_ws, message_count, byte_count);
    });

    // Forward VNC → WebSocket (server to browser)
    let conn_id_vnc = conn_id;
    let vnc_to_ws = tokio::spawn(async move {
        let mut buffer = vec![0u8; 8192];
        let mut byte_count = 0u64;
        let mut message_count = 0u32;
        
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
                    byte_count += n as u64;
                    message_count += 1;
                    
                    if message_count % 100 == 0 {
                        debug!("Connection #{}: VNC→WS forwarded {} messages ({} bytes)", 
                               conn_id_vnc, message_count, byte_count);
                    }
                    
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
        
        info!("Connection #{}: VNC→WS forwarding stopped (total: {} messages, {} bytes)", 
              conn_id_vnc, message_count, byte_count);
        
        // Send close message
        let _ = ws_write.send(Message::Close(None)).await;
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = ws_to_vnc => {
            debug!("Connection #{}: WS→VNC task completed", conn_id);
        }
        _ = vnc_to_ws => {
            debug!("Connection #{}: VNC→WS task completed", conn_id);
        }
    }

    debug!("Connection #{}: Bidirectional forwarding ended", conn_id);
    Ok(())
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
