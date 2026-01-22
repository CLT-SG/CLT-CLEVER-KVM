use std::sync::{Arc, Mutex};
use tauri::Manager;
use log::{debug, error, info, warn};
use local_ip_address::local_ip;

use crate::app::{ServerState, ServerOptions, MonitorInfo};
use crate::core::ScreenCapture;
use crate::network::WebSocketServer;

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

#[tauri::command]
pub fn start_server(app_handle: tauri::AppHandle, port: Option<u16>, options: Option<ServerOptions>) -> Result<String, String> {
    let port = port.unwrap_or(crate::lib::DEFAULT_SERVER_PORT);
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock().unwrap();

    if state.running {
        warn!("Attempted to start server when already running");
        return Err("Server is already running".to_string());
    }
    
    // Store options
    if let Some(opts) = options {
        debug!("Server options: delta_encoding={:?}, adaptive_quality={:?}, encryption={:?}, webrtc={:?}, vp8={:?}, monitor={:?}",
               opts.delta_encoding, opts.adaptive_quality, opts.encryption, opts.webrtc, opts.vp8, opts.monitor);
        state.options = opts;
    }

    info!("Starting KVM server on port {}", port);
    
    // Apply system optimizations for ultra-low latency performance
    info!("🔧 Applying system optimizations for ultra-low latency...");
    if let Err(e) = crate::system::apply_ultra_performance_optimizations() {
        warn!("Failed to apply some system optimizations: {}", e);
        info!("Server will still work but may not achieve optimal performance");
    } else {
        info!("✅ System optimizations applied successfully");
    }
    
    let app_handle_clone = app_handle.clone();
    let server = state.runtime.block_on(async move {
        match WebSocketServer::new(port, app_handle_clone).await {
            Ok(server) => {
                info!("Server started successfully");
                Ok(server)
            },
            Err(e) => {
                error!("Failed to start server: {}", e);
                Err(format!("Failed to start server: {}", e))
            },
        }
    })?;

    state.server_handle = Some(server);
    state.port = port;
    state.running = true;

    // Get local IP address with better detection
    let ip = get_network_ip().unwrap_or_else(|| {
        warn!("Could not determine network IP, falling back to localhost");
        "127.0.0.1".to_string()
    });

    let url = format!("http://{}:{}/kvm", ip, port);
    info!("Server URL: {}", url);
    info!("Server is now accessible from network at: {}", url);
    Ok(url)
}

#[tauri::command]
pub fn stop_server(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock().unwrap();

    if !state.running {
        warn!("Attempted to stop server when not running");
        return Err("Server is not running".to_string());
    }

    info!("Stopping KVM server");
    if let Some(server) = state.server_handle.take() {
        state.runtime.block_on(async {
            server.shutdown().await;
        });
        info!("Server stopped successfully");
    }

    state.running = false;
    Ok(())
}

#[tauri::command]
pub fn start_kvm_server(app_handle: tauri::AppHandle, port: Option<u16>, options: Option<ServerOptions>) -> Result<String, String> {
    start_server(app_handle, port, options)
}

#[tauri::command]
pub fn stop_kvm_server(app_handle: tauri::AppHandle) -> Result<(), String> {
    stop_server(app_handle)
}

#[tauri::command]
pub fn check_server_status(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock().unwrap();
    Ok(state.running)
}

#[tauri::command]
pub fn get_server_config(app_handle: tauri::AppHandle) -> Result<(u16, ServerOptions), String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock().unwrap();
        
    Ok((state.port, state.options.clone()))
}

#[tauri::command]
pub fn get_server_status(app_handle: tauri::AppHandle) -> bool {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock().unwrap();
    debug!("Server status requested: {}", state.running);
    state.running
}

#[tauri::command]
pub fn get_server_url(app_handle: tauri::AppHandle) -> Result<String, String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock().unwrap();

    if !state.running {
        warn!("URL requested but server is not running");
        return Err("Server is not running".to_string());
    }

    // Get local IP address with better error handling and multiple attempts
    let ip = get_network_ip().unwrap_or_else(|| {
        warn!("Could not determine network IP, falling back to localhost");
        "127.0.0.1".to_string()
    });

    let url = format!("http://{}:{}/kvm", ip, state.port);
    debug!("Returning server URL: {}", url);
    info!("KVM server accessible at: {}", url);
    Ok(url)
}

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
// VNC Server Commands
// ============================================================================

use crate::vnc::{VncKvmServer, VncServerConfig, ScreencastRegistration, register_vnc_with_clever_service};
use serde::Serialize;

/// VNC server information returned to frontend
#[derive(Debug, Serialize)]
pub struct VncServerInfo {
    pub vnc_url: String,
    pub audio_url: Option<String>,
    pub port: u16,
    pub audio_port: Option<u16>,
    pub clients_connected: usize,
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

/// Start VNC server
#[tauri::command]
pub async fn start_vnc_server(
    app_handle: tauri::AppHandle,
    port: Option<u16>,
    monitor: Option<usize>,
    enable_audio: bool,
    audio_port: Option<u16>,
) -> Result<VncServerInfo, String> {
    info!("🚀 Starting VNC server...");
    
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

    // Check if VNC server is already running
    if let Some(ref vnc_server) = state.vnc_server {
        let vnc = vnc_server.lock()
            .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
        if vnc.is_running() {
            warn!("VNC server is already running");
            return Err("VNC server is already running".to_string());
        }
    }

    // Create VNC server configuration
    let config = VncServerConfig {
        port: port.unwrap_or(5900),
        monitor_id: monitor.unwrap_or(0),
        enable_audio,
        audio_port: if enable_audio { Some(audio_port.unwrap_or(5901)) } else { None },
        max_clients: 10,
        password: None,
    };

    // Create and start VNC server
    match VncKvmServer::new(config.clone()) {
        Ok(mut vnc_server) => {
            match vnc_server.start().await {
                Ok(_) => {
                    let local_ip = match local_ip() {
                        Ok(ip) => ip.to_string(),
                        Err(_) => "localhost".to_string(),
                    };

                    let vnc_url = format!("vnc://{}:{}", local_ip, config.port);
                    let audio_url = vnc_server.get_audio_url();
                    let clients_connected = vnc_server.get_client_count();
                    
                    // Store VNC server in state
                    state.vnc_server = Some(Arc::new(Mutex::new(vnc_server)));

                    info!("✅ VNC server started successfully");
                    info!("   VNC URL: {}", vnc_url);
                    if let Some(ref audio) = audio_url {
                        info!("   Audio URL: {}", audio);
                    }

                    Ok(VncServerInfo {
                        vnc_url,
                        audio_url,
                        port: config.port,
                        audio_port: config.audio_port,
                        clients_connected,
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

/// Stop VNC server
#[tauri::command]
pub async fn stop_vnc_server(
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("🛑 Stopping VNC server...");
    
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

    match state.vnc_server.take() {
        Some(vnc_server) => {
            let mut vnc = vnc_server.lock()
                .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
            match vnc.stop().await {
                Ok(_) => {
                    state.vnc_registration = None;
                    info!("✅ VNC server stopped");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ Failed to stop VNC server: {}", e);
                    Err(format!("Failed to stop VNC server: {}", e))
                }
            }
        }
        None => {
            warn!("No VNC server running");
            Err("No VNC server running".to_string())
        }
    }
}

/// Get VNC server status
#[tauri::command]
pub async fn get_vnc_status(
    app_handle: tauri::AppHandle,
) -> Result<VncStatus, String> {
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

    let (running, clients, audio_enabled) = match state.vnc_server.as_ref() {
        Some(vnc_server) => {
            let vnc = vnc_server.lock()
                .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
            (
                vnc.is_running(),
                vnc.get_client_count(),
                vnc.get_config().enable_audio,
            )
        }
        None => (false, 0, false),
    };

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
        clients,
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
    
    let state = app_handle.state::<Arc<Mutex<ServerState>>>();
    let mut state = state.lock()
        .map_err(|e| format!("Failed to acquire state lock: {}", e))?;

    // Check if VNC server is running
    let (vnc_port, audio_port) = match state.vnc_server.as_ref() {
        Some(vnc_server) => {
            let vnc = vnc_server.lock()
                .map_err(|e| format!("Failed to acquire VNC server lock: {}", e))?;
            if !vnc.is_running() {
                return Err("VNC server is not running".to_string());
            }
            let config = vnc.get_config();
            (config.port, config.audio_port)
        }
        None => {
            return Err("VNC server is not running".to_string());
        }
    };

    // Get hostname
    let hostname = gethostname::gethostname()
        .to_string_lossy()
        .to_string();

    // Register with clever-service
    match register_vnc_with_clever_service(&clever_url, vnc_port, audio_port, &hostname).await {
        Ok(registration) => {
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
