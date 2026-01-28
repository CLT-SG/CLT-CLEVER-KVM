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
    audio_streamers: HashMap<String, Arc<Mutex<WebSocketAudioStreamer>>>,
    hostname: String,
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
            audio_streamers: HashMap::new(),
            hostname,
        })
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
        let audio_port = if enable_audio {
            Some(audio_port.unwrap_or(6900 + monitor_id as u16))
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
        
        // Start audio streamer if enabled
        let audio_url = if let Some(audio_port) = audio_port {
            let mut audio_streamer = WebSocketAudioStreamer::new(audio_port, Some(self.hostname.clone()))
                .context("Failed to create audio streamer")?;
            
            audio_streamer.start().await
                .context("Failed to start audio streamer")?;
            
            info!("✅ Audio streamer started on port {}", audio_port);
            
            let url = audio_streamer.get_stream_url();
            self.audio_streamers.insert(key.clone(), Arc::new(Mutex::new(audio_streamer)));
            
            Some(url)
        } else {
            None
        };
        
        // Store VNC server
        let vnc_url = format!("vnc://{}:{}", self.hostname, vnc_port);
        let clients_connected = vnc_server.get_client_count();
        
        self.vnc_servers.insert(key, Arc::new(Mutex::new(vnc_server)));
        
        Ok(VncServerInfo {
            vnc_url,
            audio_url,
            monitor_id,
            monitor_name: monitor.name.clone(),
            position_x: monitor.position_x,
            position_y: monitor.position_y,
            width: monitor.width,
            height: monitor.height,
            vnc_port,
            audio_port,
            clients_connected,
        })
    }
    
    /// Stop a VNC server for a specific monitor
    pub async fn stop_vnc_server(&mut self, monitor_id: usize) -> Result<()> {
        info!("🛑 Stopping VNC server for monitor {}", monitor_id);
        
        let key = format!("monitor_{}", monitor_id);
        
        // Stop audio streamer if exists
        if let Some(audio_streamer) = self.audio_streamers.remove(&key) {
            let mut streamer = audio_streamer.lock();
            streamer.stop();
            info!("✅ Audio streamer stopped for monitor {}", monitor_id);
        }
        
        // Stop VNC server if exists
        if let Some(vnc_server) = self.vnc_servers.remove(&key) {
            let mut server = vnc_server.lock();
            server.stop().await
                .context("Failed to stop VNC server")?;
            info!("✅ VNC server stopped for monitor {}", monitor_id);
        } else {
            warn!("VNC server for monitor {} not found", monitor_id);
        }
        
        Ok(())
    }
    
    /// Stop all VNC servers
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("🛑 Stopping all VNC servers");
        
        // Stop all audio streamers
        for (_, audio_streamer) in self.audio_streamers.drain() {
            let mut streamer = audio_streamer.lock();
            streamer.stop();
        }
        
        // Stop all VNC servers
        for (_, vnc_server) in self.vnc_servers.drain() {
            let mut server = vnc_server.lock();
            if let Err(e) = server.stop().await {
                error!("Failed to stop VNC server: {}", e);
            }
        }
        
        info!("✅ All VNC servers stopped");
        Ok(())
    }
    
    /// Get status of all running VNC servers
    pub fn get_status(&self) -> Vec<VncServerInfo> {
        let mut servers = Vec::new();
        
        for (key, vnc_server) in &self.vnc_servers {
            let server = vnc_server.lock();
            
            if !server.is_running() {
                continue;
            }
            
            let config = server.get_config();
            let monitor_id = config.monitor_id;
            
            // Get monitor info
            if let Ok(monitors) = ScreenCapture::get_all_monitors() {
                if let Some(monitor) = monitors.get(monitor_id) {
                    let audio_url = self.audio_streamers.get(key)
                        .map(|streamer| streamer.lock().get_stream_url());
                    
                    servers.push(VncServerInfo {
                        vnc_url: format!("vnc://{}:{}", self.hostname, config.port),
                        audio_url,
                        monitor_id,
                        monitor_name: monitor.name.clone(),
                        position_x: monitor.position_x,
                        position_y: monitor.position_y,
                        width: monitor.width,
                        height: monitor.height,
                        vnc_port: config.port,
                        audio_port: config.audio_port,
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

impl Default for VncServerManager {
    fn default() -> Self {
        Self::new().expect("Failed to create VNC server manager")
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
