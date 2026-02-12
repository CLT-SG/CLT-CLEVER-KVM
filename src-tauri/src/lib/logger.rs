//! Cross-platform logging module for Clever KVM
//!
//! Provides file-based logging with cross-platform support for Windows, Linux, and macOS.
//! Logs are stored in the application's data directory and can be accessed via the
//! `get_logs` Tauri command.

use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use chrono::Local;
use fern::Dispatch;
use log::LevelFilter;

/// In-memory log buffer for recent logs (last N entries)
const MAX_MEMORY_LOG_LINES: usize = 1000;

// Global in-memory log buffers for real-time access
lazy_static::lazy_static! {
    static ref DEBUG_LOG_BUFFER: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::with_capacity(MAX_MEMORY_LOG_LINES)));
    static ref ERROR_LOG_BUFFER: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::with_capacity(MAX_MEMORY_LOG_LINES)));
}

/// Get the cross-platform log directory path
/// 
/// Returns the appropriate directory for each platform:
/// - **Windows**: `%LOCALAPPDATA%\clever-kvm\logs` (e.g., `C:\Users\<user>\AppData\Local\clever-kvm\logs`)
/// - **macOS**: `~/Library/Application Support/clever-kvm/logs`
/// - **Linux**: `~/.local/share/clever-kvm/logs` or `$XDG_DATA_HOME/clever-kvm/logs`
pub fn get_log_directory() -> PathBuf {
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(|| {
            // Fallback to home directory if data_local_dir is not available
            dirs::home_dir()
                .map(|h| h.join(".clever-kvm"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    
    base_dir.join("clever-kvm").join("logs")
}

/// Get the path to the debug log file
pub fn get_debug_log_path() -> PathBuf {
    get_log_directory().join("clever-kvm.log")
}

/// Get the path to the error log file
pub fn get_error_log_path() -> PathBuf {
    get_log_directory().join("clever-kvm-error.log")
}

/// Initialize the logging system with file and console output
/// 
/// Sets up logging to:
/// 1. Console (stdout) for immediate feedback
/// 2. Debug log file (all log levels)
/// 3. Error log file (WARN and ERROR levels only)
/// 4. In-memory buffers for real-time UI access
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = get_log_directory();
    
    // Create log directory if it doesn't exist
    fs::create_dir_all(&log_dir)?;
    
    let debug_log_path = get_debug_log_path();
    let error_log_path = get_error_log_path();
    
    // Open log files with append mode
    let debug_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&debug_log_path)?;
    
    let error_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&error_log_path)?;
    
    // Clone Arc references for the closures
    let debug_buffer = DEBUG_LOG_BUFFER.clone();
    let error_buffer = ERROR_LOG_BUFFER.clone();
    
    // Create the logging dispatcher
    Dispatch::new()
        // Format log messages with timestamp, level, and target
        .format(move |out, message, record| {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let formatted = format!(
                "[{}] [{}] [{}] {}",
                timestamp,
                record.level(),
                record.target(),
                message
            );
            
            // Store in memory buffers
            {
                if let Ok(mut buffer) = debug_buffer.write() {
                    if buffer.len() >= MAX_MEMORY_LOG_LINES {
                        buffer.remove(0);
                    }
                    buffer.push(formatted.clone());
                }
            }
            
            // Also store errors in error buffer
            if record.level() <= log::Level::Warn {
                if let Ok(mut buffer) = error_buffer.write() {
                    if buffer.len() >= MAX_MEMORY_LOG_LINES {
                        buffer.remove(0);
                    }
                    buffer.push(formatted.clone());
                }
            }
            
            out.finish(format_args!("{}", formatted))
        })
        // Set the minimum log level
        .level(LevelFilter::Debug)
        // Filter out noisy crates
        .level_for("hyper", LevelFilter::Info)
        .level_for("rustls", LevelFilter::Info)
        .level_for("webrtc", LevelFilter::Info)
        .level_for("tokio_tungstenite", LevelFilter::Info)
        .level_for("tungstenite", LevelFilter::Info)
        // Console output
        .chain(std::io::stdout())
        // Debug log file (all levels)
        .chain(debug_file)
        // Error log file (WARN and above)
        .chain(
            Dispatch::new()
                .level(LevelFilter::Warn)
                .chain(error_file)
        )
        .apply()?;
    
    log::info!("Logging initialized");
    log::info!("Log directory: {}", log_dir.display());
    log::info!("Debug log: {}", debug_log_path.display());
    log::info!("Error log: {}", error_log_path.display());
    
    Ok(())
}

/// Read the debug log file contents
/// 
/// Returns the contents of the debug log file, or an error message if the file
/// cannot be read. Prioritizes in-memory buffer for recent logs.
pub fn read_debug_log() -> String {
    // First try to get from memory buffer (most recent logs)
    if let Ok(buffer) = DEBUG_LOG_BUFFER.read() {
        if !buffer.is_empty() {
            return buffer.join("\n");
        }
    }
    
    // Fall back to reading from file
    let path = get_debug_log_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            if content.is_empty() {
                "No debug logs recorded yet. Start the server to generate logs.".to_string()
            } else {
                // Return last 500 lines to avoid huge responses
                let lines: Vec<&str> = content.lines().collect();
                let start = if lines.len() > 500 { lines.len() - 500 } else { 0 };
                lines[start..].join("\n")
            }
        }
        Err(e) => {
            format!(
                "Debug log not available.\nPath: {}\nError: {}\n\nThis is normal on first run - logs will appear once the application generates output.",
                path.display(),
                e
            )
        }
    }
}

/// Read the error log file contents
/// 
/// Returns the contents of the error log file (WARN and ERROR levels),
/// or an error message if the file cannot be read.
pub fn read_error_log() -> String {
    // First try to get from memory buffer (most recent errors)
    if let Ok(buffer) = ERROR_LOG_BUFFER.read() {
        if !buffer.is_empty() {
            return buffer.join("\n");
        }
    }
    
    // Fall back to reading from file
    let path = get_error_log_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            if content.is_empty() {
                "No errors or warnings logged. Your application is running smoothly!".to_string()
            } else {
                // Return last 200 lines for errors
                let lines: Vec<&str> = content.lines().collect();
                let start = if lines.len() > 200 { lines.len() - 200 } else { 0 };
                lines[start..].join("\n")
            }
        }
        Err(e) => {
            format!(
                "Error log not available.\nPath: {}\nError: {}\n\nThis is normal on first run - error logs will appear if any warnings or errors occur.",
                path.display(),
                e
            )
        }
    }
}

/// Clear all log files and memory buffers
pub fn clear_logs() -> Result<(), Box<dyn std::error::Error>> {
    // Clear memory buffers
    if let Ok(mut buffer) = DEBUG_LOG_BUFFER.write() {
        buffer.clear();
    }
    if let Ok(mut buffer) = ERROR_LOG_BUFFER.write() {
        buffer.clear();
    }
    
    // Clear file contents (truncate to 0 bytes)
    let debug_path = get_debug_log_path();
    let error_path = get_error_log_path();
    
    if debug_path.exists() {
        File::create(&debug_path)?;
    }
    if error_path.exists() {
        File::create(&error_path)?;
    }
    
    log::info!("Log files cleared");
    Ok(())
}

/// Get log file paths for diagnostics
pub fn get_log_paths() -> (String, String) {
    (
        get_debug_log_path().to_string_lossy().to_string(),
        get_error_log_path().to_string_lossy().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_log_directory_exists() {
        let dir = get_log_directory();
        // Should return a valid path (may not exist yet)
        assert!(!dir.as_os_str().is_empty());
    }
    
    #[test]
    fn test_log_paths_are_valid() {
        let debug_path = get_debug_log_path();
        let error_path = get_error_log_path();
        
        assert!(debug_path.to_string_lossy().contains("clever-kvm"));
        assert!(error_path.to_string_lossy().contains("clever-kvm"));
    }
}
