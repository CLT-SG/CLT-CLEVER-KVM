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
mod core;
mod lib;
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
    // Create log directory
    let log_dir = get_log_directory();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Failed to create log directory {:?}: {}", log_dir, e);
    }
    
    // Set up file appenders for access and error logs
    let access_log = tracing_appender::rolling::never(&log_dir, "access.log");
    let error_log = tracing_appender::rolling::never(&log_dir, "error.log");
    
    // Create non-blocking writers for async logging
    // NOTE: The guards must be kept alive for the entire application lifetime
    // to ensure log messages are properly flushed
    let (access_writer, access_guard) = tracing_appender::non_blocking(access_log);
    let (error_writer, error_guard) = tracing_appender::non_blocking(error_log);
    
    // Initialize tracing with both console and file output
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true);
    
    // Access log layer (all levels)
    let access_layer = tracing_subscriber::fmt::layer()
        .with_writer(access_writer)
        .with_ansi(false)
        .with_target(true);
    
    // Error log layer (error level only) - use filter closure
    let error_layer = tracing_subscriber::fmt::layer()
        .with_writer(error_writer)
        .with_ansi(false)
        .with_target(true)
        .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);
    
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(access_layer)
        .with(error_layer)
        .init();
    
    info!("🚀 Starting {} - VNC Server for Multi-Monitor Video Wall", APP_NAME);
    info!("📁 Log directory: {:?}", log_dir);
    
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
            scan_mediamtx_servers,
            start_vnc_server,
            start_vnc_servers_all,
            stop_vnc_server,
            get_vnc_status,
            register_with_clever_service,
            set_use_tls_urls,
            get_use_tls_urls,
            get_vnc_config,
            set_vnc_config,
            get_audio_config,
            set_audio_config,
            get_connection_config,
            set_connection_config
        ])
        .setup(|_app| {
            info!("✅ Tauri application initialized successfully");
            info!("🎮 VNC KVM application ready - use the interface to start VNC servers");
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    // Explicitly drop log guards after Tauri exits to ensure all logs are flushed
    drop(access_guard);
    drop(error_guard);
}
