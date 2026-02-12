// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CLEVER KVM - VNC Server for Multi-Monitor Video Wall
//! 
//! A high-performance remote desktop system using VP8/VP9 encoding via libvpx,
//! following RustDesk's architecture for low-latency local network streaming.

// Use Microsoft's high-performance memory allocator for ultra-low latency
#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

// Module declarations
mod app;
mod core;
mod lib;
mod network;
mod rdengine;
mod system;
mod tls;
mod vnc;

use app::{commands::*, ServerState, APP_NAME};
use auto_launch::AutoLaunchBuilder;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, WindowEvent,
};

/// Application settings that persist across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_start: bool,
    pub start_minimized: bool,
    pub auto_start_server: bool,
    pub show_tray_icon: bool,
    pub minimize_to_tray: bool,
    pub show_notifications: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            start_minimized: false,
            auto_start_server: true,
            show_tray_icon: true,
            minimize_to_tray: true,
            show_notifications: true,
            theme: String::from("dark"),
        }
    }
}

/// Get the settings file path
fn get_settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clever-kvm");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("settings.json")
}

/// Load settings from file
fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        AppSettings::default()
    }
}

/// Save settings to file
fn save_settings_to_file(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))
}

/// Get application settings
#[tauri::command]
fn get_app_settings() -> AppSettings {
    load_settings()
}

/// Save application settings
#[tauri::command]
fn save_app_settings(settings: AppSettings) -> Result<(), String> {
    save_settings_to_file(&settings)
}

/// Set auto-start on login
#[tauri::command]
fn set_auto_start(enabled: bool) -> Result<(), String> {
    let app_name = "Clever KVM";
    
    #[cfg(target_os = "linux")]
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .to_string_lossy()
        .to_string();
    
    #[cfg(target_os = "windows")]
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .to_string_lossy()
        .to_string();
    
    #[cfg(target_os = "macos")]
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .to_string_lossy()
        .to_string();
    
    let auto_launch = AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(&app_path)
        .set_use_launch_agent(true)
        .build()
        .map_err(|e| format!("Failed to create auto-launch: {}", e))?;
    
    if enabled {
        auto_launch
            .enable()
            .map_err(|e| format!("Failed to enable auto-start: {}", e))?;
        info!("Auto-start enabled");
    } else {
        auto_launch
            .disable()
            .map_err(|e| format!("Failed to disable auto-start: {}", e))?;
        info!("Auto-start disabled");
    }
    
    // Update settings
    let mut settings = load_settings();
    settings.auto_start = enabled;
    save_settings_to_file(&settings)?;
    
    Ok(())
}

/// Check if auto-start is enabled
#[tauri::command]
fn get_auto_start() -> bool {
    let app_name = "Clever KVM";
    
    let app_path = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    
    AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(&app_path)
        .set_use_launch_agent(true)
        .build()
        .ok()
        .and_then(|al| al.is_enabled().ok())
        .unwrap_or(false)
}

/// Create the system tray menu
fn create_tray_menu() -> SystemTrayMenu {
    let show = CustomMenuItem::new("show".to_string(), "Show Window");
    let start_server = CustomMenuItem::new("start_server".to_string(), "Start Server");
    let stop_server = CustomMenuItem::new("stop_server".to_string(), "Stop Server");
    let settings = CustomMenuItem::new("settings".to_string(), "Settings");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    SystemTrayMenu::new()
        .add_item(show)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(start_server)
        .add_item(stop_server)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(settings)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

fn main() {
    // Initialize cross-platform file-based logging
    // Logs are written to platform-specific directories:
    // - Windows: %LOCALAPPDATA%\clever-kvm\logs
    // - macOS: ~/Library/Application Support/clever-kvm/logs
    // - Linux: ~/.local/share/clever-kvm/logs
    if let Err(e) = crate::lib::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        // Fall back to basic env_logger if our custom logger fails
        env_logger::init();
    }

    // Install rustls CryptoProvider before any TLS usage.
    // Required because both 'ring' (via webrtc/dtls) and 'aws-lc-rs' (via axum-server)
    // features are enabled, so rustls cannot auto-detect the provider.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");
    
    info!("Starting {} - Video Wall & Console", APP_NAME);
    
    // Load application settings
    let settings = load_settings();
    
    // Create system tray
    let system_tray = SystemTray::new().with_menu(create_tray_menu());
    
    // Run Tauri application
    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                if let Some(window) = app.get_window("main") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show" => {
                    if let Some(window) = app.get_window("main") {
                        window.show().ok();
                        window.set_focus().ok();
                    }
                }
                "start_server" => {
                    let app_handle = app.clone();
                    if let Err(e) = start_server(app_handle, Some(9921), None) {
                        log::error!("Failed to start server from tray: {}", e);
                    }
                }
                "stop_server" => {
                    let app_handle = app.clone();
                    if let Err(e) = stop_server(app_handle) {
                        log::error!("Failed to stop server from tray: {}", e);
                    }
                }
                "settings" => {
                    if let Some(window) = app.get_window("main") {
                        window.show().ok();
                        window.set_focus().ok();
                        // Emit event to switch to settings tab
                        window.emit("switch-to-settings", ()).ok();
                    }
                }
                "quit" => {
                    // Stop server before quitting
                    let app_handle = app.clone();
                    stop_server(app_handle).ok();
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .manage(Arc::new(Mutex::new(ServerState::new())))
        .invoke_handler(tauri::generate_handler![
            greet,
            get_primary_monitor_size,
            list_audio_devices,
            record_test_audio,
            get_monitors,
            get_available_monitors,
            get_logs,
            clear_app_logs,
            get_log_file_paths,
            get_network_interfaces,
            test_network_connectivity,
            get_system_info,
            check_firewall_status,
            // Settings commands
            get_app_settings,
            save_app_settings,
            set_auto_start,
            get_auto_start,
        ])
        .setup(|app| {
            info!("Tauri application initialized successfully");
            info!("KVM application ready - use the interface to start streaming");
            
            // Auto-start server on application launch if enabled
            let settings = load_settings();
            if settings.auto_start_server {
                let app_handle = app.handle();
                match start_server(app_handle.clone(), Some(9921), None) {
                    Ok(url) => {
                        info!("Auto-started KVM server at: {}", url);
                    },
                    Err(e) => {
                        log::warn!("Failed to auto-start server: {}", e);
                        info!("You can manually start the server using the interface");
                    }
                }
            }
            
            // Start minimized if enabled
            if settings.start_minimized {
                if let Some(window) = app.get_window("main") {
                    window.hide().ok();
                }
            }
            
            Ok(())
        })
        .on_window_event(|event| {
            if let WindowEvent::CloseRequested { api, .. } = event.event() {
                let settings = load_settings();
                if settings.minimize_to_tray {
                    // Hide window instead of closing
                    event.window().hide().ok();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    // Explicitly drop log guards after Tauri exits to ensure all logs are flushed
    drop(access_guard);
    drop(error_guard);
}
