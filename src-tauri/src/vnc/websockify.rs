use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

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
            return Ok(());
        }

        let addr = format!("0.0.0.0:{}", self.listen_port);
        let listener = TcpListener::bind(&addr)
            .await
            .context(format!("Failed to bind websockify to {}", addr))?;

        info!("Websockify proxy listening on {} → {}:{}", 
              addr, self.target_host, self.target_port);

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let target_host = self.target_host.clone();
        let target_port = self.target_port;

        let handle = tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("WebSocket connection from {}", addr);
                        let target_host = target_host.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = handle_websocket_connection(
                                stream,
                                target_host,
                                target_port
                            ).await {
                                debug!("WebSocket connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept WebSocket connection: {}", e);
                    }
                }
            }
        });

        self.listener_handle = Some(handle);
        Ok(())
    }

    /// Stop the websockify proxy
    pub async fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }

        info!("Websockify proxy stopped on port {}", self.listen_port);
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
) -> Result<()> {
    // Upgrade HTTP connection to WebSocket
    let ws_stream = accept_async(stream)
        .await
        .context("Failed to accept WebSocket")?;

    debug!("WebSocket connection established, connecting to VNC server {}:{}", 
           target_host, target_port);

    // Connect to VNC server
    let vnc_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .context("Failed to connect to VNC server")?;

    debug!("Connected to VNC server {}:{}", target_host, target_port);

    // Split WebSocket and TCP streams
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (mut vnc_read, mut vnc_write) = vnc_stream.into_split();

    // Forward WebSocket → VNC (browser to server)
    let ws_to_vnc = tokio::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Forward binary data to VNC server
                    if let Err(e) = vnc_write.write_all(&data).await {
                        debug!("Error writing to VNC server: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!("WebSocket close received");
                    break;
                }
                Ok(Message::Ping(data)) => {
                    // Respond to ping with pong
                    debug!("WebSocket ping received");
                    // Note: ws_write is moved, so we can't respond here
                    // The ping/pong is usually handled automatically
                }
                Ok(_) => {
                    // Ignore text, pong, and other message types
                }
                Err(e) => {
                    debug!("WebSocket read error: {}", e);
                    break;
                }
            }
        }
        debug!("WS→VNC forwarding stopped");
    });

    // Forward VNC → WebSocket (server to browser)
    let vnc_to_ws = tokio::spawn(async move {
        let mut buffer = vec![0u8; 8192];
        loop {
            match vnc_read.read(&mut buffer).await {
                Ok(0) => {
                    // Connection closed
                    debug!("VNC server closed connection");
                    break;
                }
                Ok(n) => {
                    // Forward data to WebSocket as binary message
                    let data = buffer[..n].to_vec();
                    if let Err(e) = ws_write.send(Message::Binary(data)).await {
                        debug!("Error sending to WebSocket: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    debug!("Error reading from VNC server: {}", e);
                    break;
                }
            }
        }
        debug!("VNC→WS forwarding stopped");
        
        // Send close message
        let _ = ws_write.send(Message::Close(None)).await;
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = ws_to_vnc => {
            debug!("WS→VNC task completed");
        }
        _ = vnc_to_ws => {
            debug!("VNC→WS task completed");
        }
    }

    debug!("WebSocket connection closed");
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
