// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CLEVER KVM - VNC Server for Multi-Monitor Video Wall
//! 
//! A high-performance VNC server implementation for multi-monitor setups
//! with separate audio streaming and exact display positioning.

// Use Microsoft's high-performance memory allocator for ultra-low latency
#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

// Module declarations
mod app;
mod audio;
mod core;
mod lib;
mod network;
mod streaming;
mod system;
mod vnc;

use app::{commands::*, ServerState, APP_NAME};
use log::info;
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    // Initialize logging first
    env_logger::init();
    
    info!("🚀 Starting {} - VNC Server for Multi-Monitor Video Wall", APP_NAME);
    
    // Run Tauri application
    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(ServerState::new())))
        .invoke_handler(tauri::generate_handler![
            greet,
            get_primary_monitor_size,
            list_audio_devices,
            record_test_audio,
            get_monitors,
            get_available_monitors,
            start_server,
            stop_server,
            start_kvm_server,
            stop_kvm_server,
            check_server_status,
            get_server_config,
            get_server_status,
            get_server_url,
            get_logs,
            get_network_interfaces,
            test_network_connectivity,
            get_system_info,
            check_firewall_status,
            start_vnc_server,
            start_vnc_servers_all,
            stop_vnc_server,
            get_vnc_status,
            register_with_clever_service
        ])
        .setup(|app| {
            info!("✅ Tauri application initialized successfully");
            info!("🎮 VNC KVM application ready - use the interface to start VNC servers");
            
            // Note: WebSocket/WebRTC server auto-start disabled - VNC mode only
            // Use start_vnc_server or start_vnc_servers_all commands to start VNC servers
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
