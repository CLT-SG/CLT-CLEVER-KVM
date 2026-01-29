//! Logging utilities for CLEVER KVM
//!
//! Provides cross-platform logging to home directory.

use std::path::PathBuf;

/// Get the cross-platform log directory path: ~/clever-kvm/logs/{date}/
/// 
/// Returns a path in the format:
/// - Linux: ~/.clever-kvm/logs/2024-01-15/
/// - Windows: C:\Users\{user}\clever-kvm\logs\2024-01-15\
/// - macOS: /Users/{user}/clever-kvm/logs/2024-01-15/
pub fn get_log_directory() -> PathBuf {
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    home_dir.join("clever-kvm").join("logs").join(date)
}
