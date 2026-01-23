// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CLEVER KVM - Ultra-Low Latency Remote Desktop
//! 
//! A high-performance remote desktop system optimized for local networks
//! with ultra-low latency streaming and advanced encoding capabilities.

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
    
    info!("🚀 Starting {} - Ultra-Low Latency Remote Desktop", APP_NAME);
    
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
            info!("🎮 KVM application ready - use the interface to start streaming");
            
            // Auto-start server on application launch
            let app_handle = app.handle();
            match start_server(app_handle.clone(), Some(9921), None) {
                Ok(url) => {
                    info!("🚀 Auto-started KVM server at: {}", url);
                },
                Err(e) => {
                    log::warn!("Failed to auto-start server: {}", e);
                    info!("You can manually start the server using the interface");
                }
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
