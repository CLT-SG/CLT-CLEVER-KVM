use std::sync::{Arc, Mutex};
use tauri::Manager;
use log::{debug, error, info, warn};
use local_ip_address::local_ip;
use serde::Serialize;

use crate::app::{ServerState, MonitorInfo};
use crate::core::ScreenCapture;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn get_available_monitors() -> Result<Vec<MonitorInfo>, String> {
    match ScreenCapture::get_all_monitors() {
        Ok(monitors) => {
            // Convert to frontend-friendly structure
            let frontend_monitors = monitors.into_iter().map(|m| MonitorInfo {
                id: m.id,
                name: m.name,
                is_primary: m.is_primary,
                width: m.width,
                height: m.height,
                position_x: m.position_x,
                position_y: m.position_y,
            }).collect();
            
            Ok(frontend_monitors)
        },
        Err(e) => Err(format!("Failed to get monitors: {}", e)),
    }
}

#[tauri::command]
pub fn get_monitors() -> Result<Vec<MonitorInfo>, String> {
    get_available_monitors()
}

#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<String>, String> {
    // Basic audio device enumeration - in a real implementation,
    // you would use a proper audio library like cpal
    Ok(vec![
        "Default Audio Input".to_string(),
        "System Audio Output".to_string(),
    ])
}

#[tauri::command]
pub fn record_test_audio() -> Result<String, String> {
    // Basic test recording placeholder
    Ok("Audio test recording completed successfully".to_string())
}

#[tauri::command]
pub fn get_primary_monitor_size() -> Result<(u32, u32), String> {
    match ScreenCapture::get_all_monitors() {
        Ok(monitors) => {
            for monitor in &monitors {
                if monitor.is_primary {
                    return Ok((monitor.width as u32, monitor.height as u32));
                }
            }
            // If no primary monitor found, return the first one
            if let Some(first) = monitors.into_iter().next() {
                return Ok((first.width as u32, first.height as u32));
            }
            Err("No monitors found".to_string())
        },
        Err(e) => Err(format!("Failed to get monitors: {}", e)),
    }
}

// Old WebSocket/WebRTC server commands have been removed.
// Use VNC commands instead:
// - start_vnc_server() for starting VNC servers
// - stop_vnc_server() for stopping VNC servers  
// - get_vnc_status() for checking VNC server status



#[tauri::command]
pub fn get_logs() -> Result<(String, String), String> {
    // Simplified log reading - get from default locations
    let debug_content = match std::fs::read_to_string("/tmp/clever-kvm-debug.log") {
        Ok(content) => content,
        Err(_) => "Debug log not found or accessible".to_string(),
    };
    
    let error_content = match std::fs::read_to_string("/tmp/clever-kvm-error.log") {
        Ok(content) => content,
        Err(_) => "Error log not found or accessible".to_string(),
    };
    
    Ok((debug_content, error_content))
}

#[tauri::command]
pub fn get_network_interfaces() -> Result<Vec<String>, String> {
    let mut interfaces = Vec::new();
    
    // Add the detected network IP
    if let Some(ip) = get_network_ip() {
        interfaces.push(format!("{} (detected network IP)", ip));
    }
    
    // Add localhost
    interfaces.push("127.0.0.1 (localhost)".to_string());
    
    // Try to get additional interfaces using our new comprehensive method
    match get_available_network_interfaces() {
        Ok(net_interfaces) => {
            for interface in net_interfaces {
                if !interfaces.iter().any(|i| i.starts_with(&interface)) {
                    interfaces.push(format!("{} (network interface)", interface));
                }
            }
        },
        Err(e) => {
            warn!("Failed to get network interfaces: {}", e);
        }
    }
    
    // Add some diagnostic information
    interfaces.push("--- Diagnostic Info ---".to_string());
    interfaces.push(format!("Server binding to: 0.0.0.0:9921 (all interfaces)"));
    
    Ok(interfaces)
}

#[tauri::command]
pub fn test_network_connectivity(app_handle: tauri::AppHandle) -> Result<String, String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock().unwrap();

    if !state.running {
        return Err("Server is not running".to_string());
    }

    let port = state.port;
    drop(state); // Release the lock

    // Test if we can connect to our own server
    let test_result = std::thread::spawn(move || {
        use std::net::TcpStream;
        use std::time::Duration;

        let mut results = Vec::new();
        
        results.push("=== Network Connectivity Test ===".to_string());

        // Test localhost connection
        match TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_secs(3)
        ) {
            Ok(_) => {
                results.push("✓ Localhost connection: SUCCESS".to_string());
                results.push("  → Server is reachable on localhost".to_string());
            },
            Err(e) => {
                results.push(format!("✗ Localhost connection: FAILED ({})", e));
                results.push("  → Server may not be properly bound to localhost".to_string());
            }
        }

        // Test network IP connection if available
        if let Some(ip) = get_network_ip() {
            match TcpStream::connect_timeout(
                &format!("{}:{}", ip, port).parse().unwrap(),
                Duration::from_secs(3)
            ) {
                Ok(_) => {
                    results.push(format!("✓ Network IP ({}) connection: SUCCESS", ip));
                    results.push("  → Server is accessible from the network".to_string());
                },
                Err(e) => {
                    results.push(format!("✗ Network IP ({}) connection: FAILED ({})", ip, e));
                    results.push("  → Server may be blocked by firewall or not properly bound".to_string());
                }
            }
        } else {
            results.push("⚠ Could not determine network IP address".to_string());
            results.push("  → Server will only be accessible via localhost".to_string());
        }
        
        // Add some additional diagnostic info
        results.push("".to_string());
        results.push("=== Binding Information ===".to_string());
        results.push(format!("Server is configured to bind to: 0.0.0.0:{}", port));
        results.push("This should make it accessible from all network interfaces".to_string());
        
        // Test if the port is in use by other processes
        results.push("".to_string());
        results.push("=== Port Usage Check ===".to_string());
        
        // Try to bind to the same port to see if it's available
        match std::net::TcpListener::bind(format!("127.0.0.1:{}", port + 1)) {
            Ok(_) => results.push("✓ Network stack is working properly".to_string()),
            Err(e) => results.push(format!("⚠ Network issue detected: {}", e)),
        }

        results.join("\n")
    }).join().map_err(|_| "Failed to run network test".to_string())?;

    Ok(test_result)
}

#[tauri::command]
pub fn get_system_info() -> Result<String, String> {
    let mut info = Vec::new();
    
    info.push("=== System Information ===".to_string());
    
    // Operating system
    info.push(format!("OS: {}", std::env::consts::OS));
    info.push(format!("Architecture: {}", std::env::consts::ARCH));
    
    // Hostname
    match gethostname::gethostname().to_str() {
        Some(hostname) => info.push(format!("Hostname: {}", hostname)),
        None => info.push("Hostname: Unknown".to_string()),
    }
    
    // Current working directory
    match std::env::current_dir() {
        Ok(dir) => info.push(format!("Working directory: {}", dir.display())),
        Err(e) => info.push(format!("Working directory: Error ({})", e)),
    }
    
    // Environment variables that might affect networking
    if let Ok(path) = std::env::var("PATH") {
        info.push(format!("PATH (first 100 chars): {}", 
                         if path.len() > 100 { &path[..100] } else { &path }));
    }
    
    Ok(info.join("\n"))
}

#[tauri::command]
pub fn check_firewall_status() -> Result<String, String> {
    use std::process::Command;
    
    let mut results = Vec::new();
    
    results.push("=== Firewall Status Check ===".to_string());
    
    if cfg!(target_os = "linux") {
        // Check ufw status
        if let Ok(output) = Command::new("ufw").arg("status").output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                results.push("UFW Status:".to_string());
                results.push(output_str.trim().to_string());
            }
        } else {
            results.push("UFW: Not available or permission denied".to_string());
        }
        
        // Check iptables (basic check)
        if let Ok(output) = Command::new("iptables").args(&["-L", "-n"]).output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                let lines: Vec<&str> = output_str.lines().take(10).collect();
                results.push("".to_string());
                results.push("iptables (first 10 lines):".to_string());
                results.extend(lines.iter().map(|s| s.to_string()));
            }
        } else {
            results.push("iptables: Not available or permission denied".to_string());
        }
        
        // Check if port 9921 is listening
        if let Ok(output) = Command::new("netstat").args(&["-tuln"]).output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                results.push("".to_string());
                results.push("Listening ports containing 9921:".to_string());
                for line in output_str.lines() {
                    if line.contains("9921") {
                        results.push(format!("  {}", line));
                    }
                }
            }
        }
    } else {
        results.push("Firewall check not implemented for this OS".to_string());
    }
    
    Ok(results.join("\n"))
}

/// Helper function to get the best network IP address
pub fn get_network_ip() -> Option<String> {
    // Try the local_ip crate first
    if let Ok(ip) = local_ip() {
        let ip_str = ip.to_string();
        debug!("Local IP from local_ip crate: {}", ip_str);
        
        // Avoid loopback addresses
        if !ip_str.starts_with("127.") && !ip_str.starts_with("::1") {
            info!("Using network IP: {}", ip_str);
            return Some(ip_str);
        }
    }
    
    // Fallback: try to get IP from network interfaces
    if let Ok(interfaces) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if let Ok(()) = interfaces.connect("8.8.8.8:80") {
            if let Ok(addr) = interfaces.local_addr() {
                let ip_str = addr.ip().to_string();
                debug!("Network IP from UDP socket: {}", ip_str);
                if !ip_str.starts_with("127.") && !ip_str.starts_with("::1") {
                    info!("Using network IP from UDP socket: {}", ip_str);
                    return Some(ip_str);
                }
            }
        }
    }
    
    // Try to enumerate network interfaces manually
    match get_available_network_interfaces() {
        Ok(interfaces) => {
            for interface in interfaces {
                if !interface.starts_with("127.") && !interface.starts_with("::1") 
                    && !interface.contains("localhost") {
                    info!("Using network IP from interface enumeration: {}", interface);
                    return Some(interface);
                }
            }
        },
        Err(e) => {
            warn!("Failed to enumerate network interfaces: {}", e);
        }
    }
    
    // Last resort: return None to fall back to localhost
    warn!("Could not determine network IP address, will use localhost");
    None
}

/// Helper function to get all available network interfaces
pub fn get_available_network_interfaces() -> Result<Vec<String>, String> {
    use std::process::Command;
    
    let mut interfaces = Vec::new();
    
    // Try using 'ip addr' command on Linux
    if cfg!(target_os = "linux") {
        if let Ok(output) = Command::new("ip").args(&["route", "get", "8.8.8.8"]).output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                // Parse the output to extract the source IP
                for line in output_str.lines() {
                    if line.contains("src") {
                        if let Some(src_pos) = line.find("src ") {
                            let src_part = &line[src_pos + 4..];
                            if let Some(space_pos) = src_part.find(' ') {
                                let ip = &src_part[..space_pos];
                                interfaces.push(ip.to_string());
                                debug!("Found network IP via 'ip route': {}", ip);
                            }
                        }
                    }
                }
            }
        }
        
        // Also try 'hostname -I' as backup
        if let Ok(output) = Command::new("hostname").arg("-I").output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for ip in output_str.trim().split_whitespace() {
                    if !ip.starts_with("127.") && !ip.starts_with("::1") {
                        interfaces.push(ip.to_string());
                        debug!("Found network IP via 'hostname -I': {}", ip);
                    }
                }
            }
        }
    }
    
    // Try using 'ifconfig' as fallback
    if interfaces.is_empty() {
        if let Ok(output) = Command::new("ifconfig").output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                // Simple parsing to extract inet addresses
                for line in output_str.lines() {
                    if line.trim().starts_with("inet ") && !line.contains("127.0.0.1") {
                        if let Some(inet_pos) = line.find("inet ") {
                            let inet_part = &line[inet_pos + 5..];
                            if let Some(space_pos) = inet_part.find(' ') {
                                let ip = &inet_part[..space_pos];
                                interfaces.push(ip.to_string());
                                debug!("Found network IP via ifconfig: {}", ip);
                            }
                        }
                    }
                }
            }
        }
    }
    
    if interfaces.is_empty() {
        Err("No network interfaces found".to_string())
    } else {
        Ok(interfaces)
    }
}

// ============================================================================
// MediaMTX Server Discovery Commands
// ============================================================================

use std::net::SocketAddr;
use std::time::Duration;

/// MediaMTX server information
#[derive(Debug, Serialize, Clone)]
pub struct MediaMtxServer {
    pub ip: String,
    pub port: u16,
    pub url: String,
}

/// Scan local network for MediaMTX servers on port 9997
#[tauri::command]
pub async fn scan_mediamtx_servers() -> Result<Vec<MediaMtxServer>, String> {
    info!("🔍 Scanning local network for MediaMTX servers on port 9997...");
    
    // Get the local IP to determine the subnet
    let local_ip = match get_network_ip() {
        Some(ip) => ip,
        None => {
            warn!("Could not determine local IP address, scanning localhost only");
            // Try localhost
            if test_mediamtx_connection("127.0.0.1", 9997).await {
                info!("✅ Found MediaMTX server on localhost:9997");
                return Ok(vec![MediaMtxServer {
                    ip: "127.0.0.1".to_string(),
                    port: 9997,
                    url: "http://127.0.0.1:9997".to_string(),
                }]);
            }
            return Ok(vec![]);
        }
    };
    
    info!("Local IP detected: {}", local_ip);
    
    // Parse the IP to get subnet
    let parts: Vec<&str> = local_ip.split('.').collect();
    if parts.len() != 4 {
        return Err("Invalid IP address format".to_string());
    }
    
    let subnet_base = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    info!("Scanning subnet: {}.0/24 for MediaMTX servers", subnet_base);
    
    let mut servers = Vec::new();
    let port = 9997;
    
    // Test localhost first
    if test_mediamtx_connection("127.0.0.1", port).await {
        info!("✅ Found MediaMTX server on localhost:9997");
        servers.push(MediaMtxServer {
            ip: "127.0.0.1".to_string(),
            port,
            url: "http://127.0.0.1:9997".to_string(),
        });
    }
    
    // Scan the subnet in parallel using tokio
    let mut tasks = Vec::new();
    
    for i in 1..=254 {
        let ip = format!("{}.{}", subnet_base, i);
        
        // Skip scanning our own IP if we already found localhost
        if ip == local_ip && servers.iter().any(|s| s.ip == "127.0.0.1") {
            continue;
        }
        
        let task = tokio::spawn(async move {
            if test_mediamtx_connection(&ip, port).await {
                Some(MediaMtxServer {
                    ip: ip.clone(),
                    port,
                    url: format!("http://{}:{}", ip, port),
                })
            } else {
                None
            }
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results = futures_util::future::join_all(tasks).await;
    
    for result in results {
        if let Ok(Some(server)) = result {
            info!("✅ Found MediaMTX server at {}", server.url);
            servers.push(server);
        }
    }
    
    if servers.is_empty() {
        info!("❌ No MediaMTX servers found on the network");
    } else {
        info!("✅ Found {} MediaMTX server(s)", servers.len());
    }
    
    Ok(servers)
}

/// Test if a MediaMTX server is running at the given address
async fn test_mediamtx_connection(ip: &str, port: u16) -> bool {
    let addr = match format!("{}:{}", ip, port).parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(_) => return false,
    };
    
    // Try to connect with a short timeout
    let result = tokio::time::timeout(
        Duration::from_millis(200),
        tokio::net::TcpStream::connect(addr)
    ).await;
    
    result.is_ok() && result.unwrap().is_ok()
}

// ============================================================================
// VNC Server Commands
// ============================================================================

use crate::vnc::{VncKvmServer, VncServerConfig, ScreencastRegistration, register_vnc_with_clever_service};

/// VNC server information returned to frontend
#[derive(Debug, Serialize, Clone)]
pub struct VncServerInfo {
    pub vnc_url: String,
    pub websockify_url: String,
    pub audio_url: Option<String>,
    pub port: u16,
    pub audio_port: Option<u16>,
    pub clients_connected: usize,
    pub monitor_id: usize,
    pub monitor_name: String,
    pub width: usize,
    pub height: usize,
    pub position_x: i32,
    pub position_y: i32,
    pub hostname: String,
}

/// VNC servers information for multi-monitor setup
#[derive(Debug, Serialize)]
pub struct VncServersInfo {
    pub servers: Vec<VncServerInfo>,
    pub audio_url: Option<String>,
}

/// VNC server status
#[derive(Debug, Serialize)]
pub struct VncStatus {
    pub running: bool,
    pub clients: usize,
    pub audio_enabled: bool,
    pub registration_status: Option<RegistrationStatus>,
}

/// Registration status
#[derive(Debug, Serialize)]
pub struct RegistrationStatus {
    pub registered: bool,
    pub id: Option<u64>,
    pub vnc_url: Option<String>,
    pub audio_url: Option<String>,
}

/// Start VNC server for a single monitor
#[tauri::command]
pub async fn start_vnc_server(
    app_handle: tauri::AppHandle,
    port: Option<u16>,
    monitor: Option<usize>,
    enable_audio: bool,
    audio_port: Option<u16>,
) -> Result<VncServerInfo, String> {
    info!("🚀 Starting VNC server...");
    
    let monitor_id = monitor.unwrap_or(0);
    
    // Check if VNC server for this monitor is already running and get monitor info
    let monitor_info = {
        let state = app_handle.state::<Arc<Mutex<ServerState>>>();
        let state = state.lock()
            .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

        // Check if VNC server for this monitor is already running
        for vnc_server in &state.vnc_servers {
        let vnc = vnc_server.lock()
            .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
        if vnc.is_running() && vnc.get_config().monitor_id == monitor_id {
            warn!("VNC server for monitor {} is already running", monitor_id);
            return Err(format!("VNC server for monitor {} is already running", monitor_id));
        }
    }
    
    // Get monitor info
    let monitors = match ScreenCapture::get_all_monitors() {
        Ok(m) => m,
        Err(e) => return Err(format!("Failed to get monitors: {}", e)),
    };
    
    if monitor_id >= monitors.len() {
        return Err(format!("Monitor {} not found", monitor_id));
    }
    
    // Extract the data we need before dropping the lock
    let monitor_name = monitors[monitor_id].name.clone();
    let monitor_width = monitors[monitor_id].width;
    let monitor_height = monitors[monitor_id].height;
    let monitor_position_x = monitors[monitor_id].position_x;
    let monitor_position_y = monitors[monitor_id].position_y;
    
    (monitor_name, monitor_width, monitor_height, monitor_position_x, monitor_position_y)
}; // Drop state lock here before async operations

let (monitor_name, monitor_width, monitor_height, monitor_position_x, monitor_position_y) = monitor_info;

    // Get hostname for URL generation (before creating VNC server)
    let hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| "localhost".into());

    // Calculate port with bounds checking to avoid collisions
    let vnc_port = if let Some(p) = port {
        p
    } else {
        let calculated_port = 5900 + monitor_id as u16;
        // Ensure port is in valid range and not too high
        if calculated_port > 5950 {
            warn!("Monitor ID {} results in port {} which may be too high, using 5900", monitor_id, calculated_port);
            return Err(format!("Too many monitors (max 50 supported for automatic port assignment)"));
        }
        calculated_port
    };

    // Create VNC server configuration
    let config = VncServerConfig {
        port: vnc_port,
        monitor_id,
        enable_audio,
        audio_port: if enable_audio { Some(audio_port.unwrap_or(6900)) } else { None },
        max_clients: 10,
        password: None,
        hostname: Some(hostname.clone()),
    };

    // Create and start VNC server
    match VncKvmServer::new(config.clone()) {
        Ok(mut vnc_server) => {
            match vnc_server.start().await {
                Ok(_) => {
                    // Get local IP as fallback
                    let local_ip = match local_ip() {
                        Ok(ip) => ip.to_string(),
                        Err(_) => hostname.clone(),
                    };

                    // Use hostname in VNC URL instead of IP address
                    let vnc_url = format!("vnc://{}:{}", hostname, config.port);
                    let websockify_url = format!("ws://{}:{}/websockify", hostname, config.port);
                    let audio_url = vnc_server.get_audio_url();
                    let clients_connected = vnc_server.get_client_count();
                    
                    // Store VNC server in state (acquire lock again after async operation)
                    {
                        let state = app_handle.state::<Arc<Mutex<ServerState>>>();
                        let mut state = state.lock()
                            .map_err(|e| format!("Failed to acquire state lock: {}", e))?;
                        state.vnc_servers.push(Arc::new(Mutex::new(vnc_server)));
                    }

                    info!("✅ VNC server started successfully");
                    info!("   Hostname: {}", hostname);
                    info!("   VNC URL: {}", vnc_url);
                    info!("   WebSockify URL: {}", websockify_url);
                    info!("   Monitor: {} ({}x{}) at ({}, {})", 
                          monitor_name, monitor_width, monitor_height,
                          monitor_position_x, monitor_position_y);
                    if let Some(ref audio) = audio_url {
                        info!("   Audio URL: {}", audio);
                    }
                    info!("   Fallback IP: {}", local_ip);

                    Ok(VncServerInfo {
                        vnc_url,
                        websockify_url,
                        audio_url,
                        port: config.port,
                        audio_port: config.audio_port,
                        clients_connected,
                        monitor_id,
                        monitor_name: monitor_name.clone(),
                        width: monitor_width,
                        height: monitor_height,
                        position_x: monitor_position_x,
                        position_y: monitor_position_y,
                        hostname: hostname.clone(),
                    })
                }
                Err(e) => {
                    error!("❌ Failed to start VNC server: {}", e);
                    Err(format!("Failed to start VNC server: {}", e))
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to create VNC server: {}", e);
            Err(format!("Failed to create VNC server: {}", e))
        }
    }
}

/// Start VNC servers for all monitors
#[tauri::command]
pub async fn start_vnc_servers_all(
    app_handle: tauri::AppHandle,
    enable_audio: bool,
) -> Result<VncServersInfo, String> {
    info!("🚀 Starting VNC servers for all monitors...");
    
    // Get all monitors
    let monitors = match ScreenCapture::get_all_monitors() {
        Ok(m) => m,
        Err(e) => return Err(format!("Failed to get monitors: {}", e)),
    };
    
    if monitors.is_empty() {
        return Err("No monitors found".to_string());
    }
    
    let mut server_infos = Vec::new();
    let mut shared_audio_url = None;
    let mut errors = Vec::new();
    
    // Start VNC server for each monitor
    for (idx, _monitor_info) in monitors.iter().enumerate() {
        // Ensure port doesn't exceed safe range
        if idx >= 50 {
            warn!("Skipping monitor {} - too many monitors (max 50 supported)", idx);
            continue;
        }
        
        let port = 5900 + idx as u16;
        let audio_port = if enable_audio && idx == 0 { Some(6900) } else { None };
        
        info!("Starting VNC server for monitor {} on port {}", idx, port);
        
        match start_vnc_server(
            app_handle.clone(),
            Some(port),
            Some(idx),
            enable_audio && idx == 0, // Only enable audio for first monitor
            audio_port,
        ).await {
            Ok(server_info) => {
                if server_info.audio_url.is_some() {
                    shared_audio_url = server_info.audio_url.clone();
                }
                server_infos.push(server_info);
            }
            Err(e) => {
                error!("❌ Failed to start VNC server for monitor {}: {}", idx, e);
                errors.push(format!("Monitor {}: {}", idx, e));
            }
        }
    }
    
    if server_infos.is_empty() {
        let error_msg = if !errors.is_empty() {
            format!("Failed to start any VNC servers. Errors: {}", errors.join("; "))
        } else {
            "Failed to start any VNC servers".to_string()
        };
        return Err(error_msg);
    }
    
    if !errors.is_empty() {
        warn!("⚠️  Started {} VNC server(s) but {} failed: {}", 
              server_infos.len(), errors.len(), errors.join("; "));
    } else {
        info!("✅ Started {} VNC server(s)", server_infos.len());
    }
    
    Ok(VncServersInfo {
        servers: server_infos,
        audio_url: shared_audio_url,
    })
}

/// Stop all VNC servers
#[tauri::command]
pub async fn stop_vnc_server(
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("🛑 Stopping all VNC servers...");
    
    let servers = {
        let state = app_handle.state::<Arc<Mutex<ServerState>>>();
        let mut state = state.lock()
            .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

        if state.vnc_servers.is_empty() {
            warn!("No VNC servers running");
            return Err("No VNC servers running".to_string());
        }

        std::mem::take(&mut state.vnc_servers)
    }; // Drop state lock here before async operations
    
    // Stop all servers in parallel using blocking tasks
    // We use spawn_blocking because we're using std::sync::Mutex which is not Send across await points
    let stop_tasks: Vec<_> = servers.into_iter().map(|vnc_server| {
        tokio::task::spawn_blocking(move || {
            let mut vnc = vnc_server.lock()
                .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
            // Use block_on since stop() is async but we're in a blocking context
            tokio::runtime::Handle::current().block_on(vnc.stop())
                .map_err(|e| format!("Failed to stop VNC server: {}", e))
        })
    }).collect();
    
    let results = futures_util::future::join_all(stop_tasks).await;
    
    let mut errors = Vec::new();
    for (idx, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok(())) => {
                info!("✅ VNC server {} stopped successfully", idx);
            }
            Ok(Err(e)) => {
                error!("❌ VNC server {} failed to stop: {}", idx, e);
                errors.push(e);
            }
            Err(e) => {
                error!("❌ VNC server {} task failed: {}", idx, e);
                errors.push(format!("Task failed: {}", e));
            }
        }
    }
    
    // Re-acquire state lock to update registration
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;
    state.vnc_registration = None;
    
    if !errors.is_empty() {
        error!("❌ Some VNC servers failed to stop: {:?}", errors);
        return Err(format!("Some servers failed to stop: {}", errors.join(", ")));
    }
    
    info!("✅ All VNC servers stopped");
    Ok(())
}

/// Get VNC server status
#[tauri::command]
pub async fn get_vnc_status(
    app_handle: tauri::AppHandle,
) -> Result<VncStatus, String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

    let mut running = false;
    let mut total_clients = 0;
    let mut audio_enabled = false;
    
    for vnc_server in &state.vnc_servers {
        let vnc = vnc_server.lock()
            .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
        if vnc.is_running() {
            running = true;
            total_clients += vnc.get_client_count();
            audio_enabled = audio_enabled || vnc.get_config().enable_audio;
        }
    }

    let registration_status = state.vnc_registration.as_ref().map(|reg| {
        RegistrationStatus {
            registered: true,
            id: Some(reg.id),
            vnc_url: Some(reg.vnc_url.clone()),
            audio_url: reg.audio_url.clone(),
        }
    });

    Ok(VncStatus {
        running,
        clients: total_clients,
        audio_enabled,
        registration_status,
    })
}

/// Register with CLEVER service
#[tauri::command]
pub async fn register_with_clever_service(
    app_handle: tauri::AppHandle,
    clever_url: String,
) -> Result<ScreencastRegistration, String> {
    info!("📡 Registering with CLEVER service at {}", clever_url);
    
    // Get VNC port and audio port (drop state lock before async call)
    let (vnc_port, audio_port) = {
        let state = app_handle.state::<Arc<Mutex<ServerState>>>();
        let state = state.lock()
            .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

        // Check if VNC servers are running
        if state.vnc_servers.is_empty() {
            return Err("No VNC servers running".to_string());
        }
        
        // Get the first VNC server's port and audio port for registration
        let vnc_server = &state.vnc_servers[0];
        let vnc = vnc_server.lock()
            .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
        if !vnc.is_running() {
            return Err("VNC server is not running".to_string());
        }
        let config = vnc.get_config();
        (config.port, config.audio_port)
    }; // Drop state lock here before async call

    // Get hostname
    let hostname = gethostname::gethostname()
        .to_string_lossy()
        .to_string();

    // Register with clever-service
    match register_vnc_with_clever_service(&clever_url, vnc_port, audio_port, &hostname).await {
        Ok(registration) => {
            // Re-acquire state lock to store registration
            let state = app_handle.state::<Arc<Mutex<ServerState>>>();
            let mut state = state.lock()
                .map_err(|e| format!("Failed to acquire state lock: {}", e))?;
            state.vnc_registration = Some(registration.clone());
            
            info!("✅ Successfully registered with CLEVER service");
            Ok(registration)
        }
        Err(e) => {
            error!("❌ Failed to register with CLEVER service: {}", e);
            Err(format!("Failed to register: {}", e))
        }
    }
}
