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
use lib::get_log_directory;
use log::info;
use std::sync::{Arc, Mutex};
use std::fs;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

fn main() {
    // Initialize logging first
    env_logger::init();

    // Install rustls CryptoProvider before any TLS usage.
    // Required because both 'ring' (via webrtc/dtls) and 'aws-lc-rs' (via axum-server)
    // features are enabled, so rustls cannot auto-detect the provider.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");
    
    info!("Starting {} - Video Wall & Console", APP_NAME);
    
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
            get_logs,
            get_network_interfaces,
            test_network_connectivity,
            get_system_info,
            check_firewall_status,
        ])
        .setup(|app| {
            info!("Tauri application initialized successfully");
            info!("KVM application ready - use the interface to start streaming");
            
            // Auto-start server on application launch
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
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    // Explicitly drop log guards after Tauri exits to ensure all logs are flushed
    drop(access_guard);
    drop(error_guard);
}
