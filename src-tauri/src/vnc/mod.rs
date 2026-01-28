//! VNC Server Module
//! 
//! This module provides a native VNC (RFB protocol) server implementation
//! for seamless integration with the CLEVER video wall ecosystem.
//! 
//! Features:
//! - Native VNC RFB 3.8 protocol server
//! - Separate audio streaming via RTSP
//! - Optional RFB audio extension support
//! - Multi-client support
//! - Auto-registration with clever-service

pub mod server;
pub mod input;
pub mod audio;
pub mod registration;

#[cfg(test)]
mod tests;

pub use server::{VncKvmServer, VncServerConfig};
pub use audio::SeparateAudioStream;
pub use registration::{register_vnc_with_clever_service, ScreencastRegistration};
