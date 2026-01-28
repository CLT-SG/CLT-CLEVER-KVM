//! VNC Server Manager Module
//! 
//! Manages multiple VNC servers for multi-monitor setups with
//! coordinated audio streaming via WebSocket.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use anyhow::{Result, Context};
use log::{info, warn, error};
use serde::{Serialize, Deserialize};

use super::server::{VncKvmServer, VncServerConfig};
use super::audio_websocket::WebSocketAudioStreamer;
use crate::core::ScreenCapture;

/// Information about a VNC server instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncServerInfo {
    pub vnc_url: String,
    pub websockify_url: String,
    pub audio_url: Option<String>,
    pub monitor_id: usize,
    pub monitor_name: String,
    pub position_x: i32,
    pub position_y: i32,
    pub width: u32,
    pub height: u32,
    pub vnc_port: u16,
    pub audio_port: Option<u16>,
    pub clients_connected: usize,
}

/// Manager for multiple VNC servers and audio streamers
pub struct VncServerManager {
    vnc_servers: HashMap<String, Arc<Mutex<VncKvmServer>>>,
    audio_streamer: Option<Arc<Mutex<WebSocketAudioStreamer>>>,
    hostname: String,
    use_tls_urls: bool, // Flag to generate wss:// URLs instead of ws://
}

impl VncServerManager {
    /// Create a new VNC server manager
    pub fn new() -> Result<Self> {
        let hostname = gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "localhost".into());
        
        info!("🖥️  Initializing VNC Server Manager with hostname: {}", hostname);
        
        Ok(Self {
            vnc_servers: HashMap::new(),
            audio_streamer: None,
            hostname,
            use_tls_urls: false, // Default to non-TLS URLs
        })
    }
    
    /// Enable or disable TLS URLs (wss:// instead of ws://)
    pub fn set_use_tls_urls(&mut self, use_tls: bool) {
        info!("🔒 Setting TLS URLs: {}", use_tls);
        self.use_tls_urls = use_tls;
    }
    
    /// Get current TLS URL setting
    pub fn get_use_tls_urls(&self) -> bool {
        self.use_tls_urls
    }
    
    /// Start a VNC server for a specific monitor
    pub async fn start_vnc_server(
        &mut self,
        monitor_id: usize,
        enable_audio: bool,
        vnc_port: Option<u16>,
        audio_port: Option<u16>,
    ) -> Result<VncServerInfo> {
        info!("🚀 Starting VNC server for monitor {}", monitor_id);
        
        // Get monitor information
        let monitors = ScreenCapture::get_all_monitors()
            .context("Failed to get monitor information")?;
        
        if monitor_id >= monitors.len() {
            return Err(anyhow::anyhow!("Monitor {} not found (total: {})", monitor_id, monitors.len()));
        }
        
        let monitor = &monitors[monitor_id];
        
        // Calculate ports
        let vnc_port = vnc_port.unwrap_or(5900 + monitor_id as u16);
        // All monitors share the same audio port 6900
        let audio_port = if enable_audio {
            Some(audio_port.unwrap_or(6900))
        } else {
            None
        };
        
        // Check if VNC server already exists for this monitor
        let key = format!("monitor_{}", monitor_id);
        if self.vnc_servers.contains_key(&key) {
            return Err(anyhow::anyhow!("VNC server already running for monitor {}", monitor_id));
        }
        
        // Create VNC server configuration
        let config = VncServerConfig {
            port: vnc_port,
            monitor_id,
            enable_audio: false, // We handle audio separately via WebSocket
            audio_port: None,
            max_clients: 10,
            password: None,
            hostname: Some(self.hostname.clone()),
        };
        
        // Create and start VNC server
        let mut vnc_server = VncKvmServer::new(config.clone())
            .context("Failed to create VNC server")?;
        
        vnc_server.start().await
            .context("Failed to start VNC server")?;
        
        info!("✅ VNC server started on port {}", vnc_port);
        
        // Start shared audio streamer if enabled and not already running
        let audio_url = if let Some(audio_port) = audio_port {
            if self.audio_streamer.is_none() {
                // Create and start the shared audio streamer with TLS URL support
                let mut audio_streamer = WebSocketAudioStreamer::new_with_tls_url(
                    audio_port,
                    Some(self.hostname.clone()),
                    self.use_tls_urls
                ).context("Failed to create audio streamer")?;
                
                audio_streamer.start().await
                    .context("Failed to start audio streamer")?;
                
                info!("✅ Shared audio streamer started on port {}", audio_port);
                
                self.audio_streamer = Some(Arc::new(Mutex::new(audio_streamer)));
            }
            
            // Get URL from the shared audio streamer
            self.audio_streamer.as_ref()
                .map(|streamer| streamer.lock().get_stream_url().to_string())
        } else {
            None
        };
        
        // Store VNC server
        let vnc_url = format!("vnc://{}:{}", self.hostname, vnc_port);
        let ws_protocol = if self.use_tls_urls { "wss" } else { "ws" };
        let websockify_url = format!("{}://{}:{}/websockify", ws_protocol, self.hostname, vnc_port);
        let clients_connected = vnc_server.get_client_count();
        
        self.vnc_servers.insert(key, Arc::new(Mutex::new(vnc_server)));
        
        Ok(VncServerInfo {
            vnc_url,
            websockify_url,
            audio_url,
            monitor_id,
            monitor_name: monitor.name.clone(),
            position_x: monitor.position_x,
            position_y: monitor.position_y,
            width: monitor.width as u32,
            height: monitor.height as u32,
            vnc_port,
            audio_port,
            clients_connected,
        })
    }
    
    /// Stop a VNC server for a specific monitor
    pub async fn stop_vnc_server(&mut self, monitor_id: usize) -> Result<()> {
        info!("🛑 Stopping VNC server for monitor {}", monitor_id);
        
        let key = format!("monitor_{}", monitor_id);
        
        // Stop VNC server if exists
        if let Some(vnc_server) = self.vnc_servers.remove(&key) {
            let mut server = vnc_server.lock();
            server.stop().await
                .context("Failed to stop VNC server")?;
            info!("✅ VNC server stopped for monitor {}", monitor_id);
        } else {
            warn!("VNC server for monitor {} not found", monitor_id);
        }
        
        // If no more VNC servers are running, stop the shared audio streamer
        if self.vnc_servers.is_empty() {
            if let Some(audio_streamer) = self.audio_streamer.take() {
                let mut streamer = audio_streamer.lock();
                streamer.stop();
                info!("✅ Shared audio streamer stopped (no more VNC servers)");
            }
        }
        
        Ok(())
    }
    
    /// Stop all VNC servers
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("🛑 Stopping all VNC servers");
        
        // Stop all VNC servers
        for (_, vnc_server) in self.vnc_servers.drain() {
            let mut server = vnc_server.lock();
            if let Err(e) = server.stop().await {
                error!("Failed to stop VNC server: {}", e);
            }
        }
        
        // Stop the shared audio streamer
        if let Some(audio_streamer) = self.audio_streamer.take() {
            let mut streamer = audio_streamer.lock();
            streamer.stop();
            info!("✅ Shared audio streamer stopped");
        }
        
        info!("✅ All VNC servers stopped");
        Ok(())
    }
    
    /// Get status of all running VNC servers
    pub fn get_status(&self) -> Vec<VncServerInfo> {
        let mut servers = Vec::new();
        
        for (_key, vnc_server) in &self.vnc_servers {
            let server = vnc_server.lock();
            
            if !server.is_running() {
                continue;
            }
            
            let config = server.get_config();
            let monitor_id = config.monitor_id;
            
            // Get monitor info
            if let Ok(monitors) = ScreenCapture::get_all_monitors() {
                if let Some(monitor) = monitors.get(monitor_id) {
                    // Get URL from the shared audio streamer
                    let audio_url = self.audio_streamer.as_ref()
                        .map(|streamer| streamer.lock().get_stream_url().to_string());
                    
                    let ws_protocol = if self.use_tls_urls { "wss" } else { "ws" };
                    
                    servers.push(VncServerInfo {
                        vnc_url: format!("vnc://{}:{}", self.hostname, config.port),
                        websockify_url: format!("{}://{}:{}/websockify", ws_protocol, self.hostname, config.port),
                        audio_url: audio_url.clone(),
                        monitor_id,
                        monitor_name: monitor.name.clone(),
                        position_x: monitor.position_x,
                        position_y: monitor.position_y,
                        width: monitor.width as u32,
                        height: monitor.height as u32,
                        vnc_port: config.port,
                        audio_port: if audio_url.is_some() { Some(6900) } else { None },
                        clients_connected: server.get_client_count(),
                    });
                }
            }
        }
        
        servers
    }
    
    /// Get the number of running VNC servers
    pub fn get_server_count(&self) -> usize {
        self.vnc_servers.len()
    }
    
    /// Check if a VNC server is running for a specific monitor
    pub fn is_running(&self, monitor_id: usize) -> bool {
        let key = format!("monitor_{}", monitor_id);
        self.vnc_servers.contains_key(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vnc_server_manager_creation() {
        let result = VncServerManager::new();
        assert!(result.is_ok());
        
        if let Ok(manager) = result {
            assert_eq!(manager.get_server_count(), 0);
        }
    }
}
