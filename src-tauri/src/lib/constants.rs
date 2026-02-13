//! Application constants for CLEVER KVM
//!
//! These constants define default values and limits used throughout the application.

#![allow(dead_code)]

/// Application constants
pub const APP_NAME: &str = "clever-kvm";
pub const DEFAULT_SERVER_PORT: u16 = 9921;
pub const MAX_FRAME_RATE: u32 = 60;
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024; // 1MB
