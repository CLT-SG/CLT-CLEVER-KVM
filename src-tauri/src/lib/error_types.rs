//! Custom error types for CLEVER KVM
//!
//! Provides structured error types for better error handling throughout the application.

#![allow(dead_code)]

use std::fmt;

/// Custom error types for better error handling
#[derive(Debug)]
pub enum KvmError {
    ServerError(String),
    NetworkError(String),
    CaptureError(String),
    AudioError(String),
    ConfigError(String),
}

impl fmt::Display for KvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KvmError::ServerError(msg) => write!(f, "Server Error: {}", msg),
            KvmError::NetworkError(msg) => write!(f, "Network Error: {}", msg),
            KvmError::CaptureError(msg) => write!(f, "Capture Error: {}", msg),
            KvmError::AudioError(msg) => write!(f, "Audio Error: {}", msg),
            KvmError::ConfigError(msg) => write!(f, "Config Error: {}", msg),
        }
    }
}

impl std::error::Error for KvmError {}
