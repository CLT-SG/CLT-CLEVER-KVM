//! mDNS Service Discovery
//!
//! Publishes the relay server as a discoverable service on the local network
//! using mDNS/DNS-SD. This allows Tauri KVM apps to automatically find the relay server.

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use tracing::{info, warn, error};

/// Service type for CLEVER KVM Relay
pub const SERVICE_TYPE: &str = "_clever-kvm._tcp.local.";

/// Service name
pub const SERVICE_NAME: &str = "CLEVER KVM Relay Server";

/// mDNS Discovery Service
pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
    service_fullname: Option<String>,
    port: u16,
    hostname: String,
}

impl MdnsDiscovery {
    /// Create a new mDNS discovery service
    pub fn new(port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;
        let hostname = get_hostname();
        
        Ok(Self {
            daemon,
            service_fullname: None,
            port,
            hostname,
        })
    }
    
    /// Publish the relay server service
    pub fn publish(&mut self) -> Result<()> {
        let service_name = format!("clever-relay-{}", &self.hostname);
        
        // Build service properties
        let mut properties = HashMap::new();
        properties.insert("version".to_string(), "2.0.0".to_string());
        properties.insert("hostname".to_string(), self.hostname.clone());
        properties.insert("type".to_string(), "relay".to_string());
        
        // Get local IP addresses
        let addresses = get_local_addresses();
        
        // Create service info
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &service_name,
            &format!("{}.local.", self.hostname),
            &addresses.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(","),
            self.port,
            properties,
        )?;
        
        // Register the service
        self.daemon.register(service.clone())?;
        self.service_fullname = Some(service.get_fullname().to_string());
        
        info!(
            "📡 mDNS service published: {} on port {} ({})",
            service_name,
            self.port,
            self.get_local_address()
        );
        
        if !addresses.is_empty() {
            info!("   Accessible at: http://{}.local:{}/", self.hostname, self.port);
            for addr in &addresses {
                info!("   Accessible at: http://{}:{}/", addr, self.port);
            }
        }
        
        Ok(())
    }
    
    /// Unpublish the service
    pub fn unpublish(&mut self) -> Result<()> {
        if let Some(fullname) = self.service_fullname.take() {
            self.daemon.unregister(&fullname)?;
            info!("📡 mDNS service unpublished");
        }
        Ok(())
    }
    
    /// Get the hostname being advertised
    pub fn get_hostname(&self) -> &str {
        &self.hostname
    }
    
    /// Get the local hostname with .local suffix
    pub fn get_local_hostname(&self) -> String {
        format!("{}.local", self.hostname)
    }
    
    /// Get the primary local IP address
    pub fn get_local_address(&self) -> String {
        get_local_addresses()
            .first()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }
}

impl Drop for MdnsDiscovery {
    fn drop(&mut self) {
        if let Err(e) = self.unpublish() {
            warn!("Failed to unpublish mDNS service: {}", e);
        }
        if let Err(e) = self.daemon.shutdown() {
            warn!("Failed to shutdown mDNS daemon: {}", e);
        }
    }
}

/// Get the system hostname
pub fn get_hostname() -> String {
    gethostname::gethostname()
        .to_string_lossy()
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// Get local IP addresses (non-loopback)
pub fn get_local_addresses() -> Vec<IpAddr> {
    let mut addresses = Vec::new();
    
    // Try to get addresses from network interfaces
    if let Ok(interfaces) = get_network_interfaces() {
        for addr in interfaces {
            // Skip loopback and link-local addresses
            match addr {
                IpAddr::V4(v4) => {
                    if !v4.is_loopback() && !v4.is_link_local() {
                        addresses.push(addr);
                    }
                }
                IpAddr::V6(v6) => {
                    if !v6.is_loopback() {
                        // Skip link-local IPv6
                        let segments = v6.segments();
                        if segments[0] != 0xfe80 {
                            addresses.push(addr);
                        }
                    }
                }
            }
        }
    }
    
    // Prioritize IPv4 addresses
    addresses.sort_by(|a, b| {
        match (a, b) {
            (IpAddr::V4(_), IpAddr::V6(_)) => std::cmp::Ordering::Less,
            (IpAddr::V6(_), IpAddr::V4(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
    
    addresses
}

/// Get network interface addresses
fn get_network_interfaces() -> Result<Vec<IpAddr>> {
    use std::net::UdpSocket;
    
    let mut addresses = Vec::new();
    
    // Connect to a public address to determine local IP
    // This doesn't actually send data, just determines routing
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        // Try to connect to Google DNS to determine primary interface
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                addresses.push(local_addr.ip());
            }
        }
    }
    
    // Also try to get all interface addresses using DNS lookup
    if let Ok(hostname) = hostname::get() {
        if let Ok(hostname_str) = hostname.into_string() {
            if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(hostname_str.as_str(), 0)) {
                for addr in addrs {
                    let ip = addr.ip();
                    if !addresses.contains(&ip) {
                        addresses.push(ip);
                    }
                }
            }
        }
    }
    
    Ok(addresses)
}

/// mDNS client for discovering relay servers
pub struct MdnsClient {
    daemon: ServiceDaemon,
}

impl MdnsClient {
    /// Create a new mDNS client
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self { daemon })
    }
    
    /// Browse for CLEVER KVM relay servers
    pub fn browse(&self) -> Result<mdns_sd::Receiver<mdns_sd::ServiceEvent>> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        Ok(receiver)
    }
    
    /// Discover relay servers with timeout
    pub async fn discover_timeout(&self, timeout_ms: u64) -> Vec<DiscoveredRelay> {
        let mut relays = Vec::new();
        
        match self.browse() {
            Ok(receiver) => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
                
                while std::time::Instant::now() < deadline {
                    match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                        Ok(event) => {
                            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                                let relay = DiscoveredRelay {
                                    hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                    port: info.get_port(),
                                    addresses: info.get_addresses().iter().map(|a| a.to_string()).collect(),
                                    properties: info.get_properties().iter().map(|p| {
                                        (p.key().to_string(), p.val_str().to_string())
                                    }).collect(),
                                };
                                
                                if !relays.iter().any(|r: &DiscoveredRelay| r.hostname == relay.hostname) {
                                    relays.push(relay);
                                }
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
            Err(e) => {
                error!("Failed to browse mDNS services: {}", e);
            }
        }
        
        relays
    }
}

/// Discovered relay server information
#[derive(Debug, Clone)]
pub struct DiscoveredRelay {
    pub hostname: String,
    pub port: u16,
    pub addresses: Vec<String>,
    pub properties: HashMap<String, String>,
}

impl DiscoveredRelay {
    /// Get the best address to connect to (prefer IPv4)
    pub fn get_connect_address(&self) -> Option<String> {
        // Try IPv4 first
        for addr in &self.addresses {
            if !addr.contains(':') {
                return Some(format!("{}:{}", addr, self.port));
            }
        }
        // Fall back to any address
        self.addresses.first().map(|a| format!("{}:{}", a, self.port))
    }
    
    /// Get the .local hostname URL
    pub fn get_local_url(&self) -> String {
        format!("http://{}:{}", self.hostname, self.port)
    }
}
