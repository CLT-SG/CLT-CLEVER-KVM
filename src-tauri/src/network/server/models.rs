//! Data models and constants for the KVM server
//!
//! Contains WebSocket message types, configuration constants, and parameter structures.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Constants
pub const NETWORK_PERFORMANCE_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
pub const DEFAULT_QUALITY: u8 = 85;
pub const MIN_QUALITY: u8 = 25;
pub const MAX_QUALITY: u8 = 95;

#[derive(Debug, Deserialize)]
pub struct KvmParams {
    pub stretch: Option<String>,
    pub mute: Option<String>,
    pub audio: Option<String>,
    pub remote_only: Option<String>,
    pub encryption: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FrameData {
    pub width: usize,
    pub height: usize,
    pub image: String,
    pub timestamp: u128,
}

#[derive(Debug, Serialize)]
pub struct DeltaFrameData {
    pub tiles: HashMap<usize, String>,
    pub timestamp: u128,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub width: usize,
    pub height: usize,
    pub hostname: String,
    pub tile_width: usize,
    pub tile_height: usize,
    pub tile_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkStats {
    pub latency: u32,  // in milliseconds
    pub bandwidth: f32, // in Mbps
    pub packet_loss: f32, // percentage
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            latency: 50,    // 50ms default latency
            bandwidth: 10.0, // 10 Mbps default bandwidth
            packet_loss: 0.0, // 0% packet loss
        }
    }
}
