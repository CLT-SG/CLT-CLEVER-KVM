//! VNC Server Module
//! 
//! This module provides a native VNC (RFB protocol) server implementation
//! for seamless integration with the CLEVER video wall ecosystem.
//! 
//! Features:
//! - Native VNC RFB 3.8 protocol server
//! - WebSocket-based audio streaming with Opus encoding
//! - Multi-monitor support with exact positioning
//! - Multi-client support
//! - Auto-registration with clever-service

pub mod server;
pub mod input;
pub mod audio;
pub mod audio_websocket;
pub mod registration;
pub mod manager;
pub mod websockify;

#[cfg(test)]
mod tests;

pub use server::{VncKvmServer, VncServerConfig};
pub use manager::VncServerManager;
pub use registration::{register_vnc_with_clever_service, ScreencastRegistration};
