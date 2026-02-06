//! Binary Protocol for Frame Transport
//!
//! Minimal-overhead binary framing protocol inspired by RustDesk's approach.
//! RustDesk uses protobuf; we use a simpler binary format since we only need
//! video + audio + control over WebSocket.
//!
//! Frame format:
//! ```text
//! [1B type] [4B length] [payload...]
//! ```
//!
//! Video frame payload:
//! ```text
//! [1B codec] [1B flags] [4B width] [4B height] [8B timestamp_ms] [data...]
//! ```

use serde::{Deserialize, Serialize};

/// Message type identifiers (first byte of binary message)
pub const MSG_VIDEO_FRAME: u8 = 0x01;
pub const MSG_AUDIO_FRAME: u8 = 0x02;
pub const MSG_CURSOR: u8 = 0x03;
pub const MSG_CLIPBOARD: u8 = 0x04;
pub const MSG_PING: u8 = 0x05;
pub const MSG_PONG: u8 = 0x06;

/// Codec identifiers
pub const CODEC_VP8: u8 = 0x01;
pub const CODEC_VP9: u8 = 0x02;
pub const CODEC_H264: u8 = 0x03;
pub const CODEC_OPUS: u8 = 0x10;

/// Frame flags
pub const FLAG_KEYFRAME: u8 = 0x01;

/// Video frame header — sent as binary over WebSocket
#[derive(Debug, Clone)]
pub struct VideoFrameHeader {
    pub codec: u8,
    pub flags: u8,
    pub width: u32,
    pub height: u32,
    pub timestamp_ms: u64,
}

impl VideoFrameHeader {
    pub const SIZE: usize = 1 + 1 + 4 + 4 + 8; // 18 bytes

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.codec);
        buf.push(self.flags);
        buf.extend_from_slice(&self.width.to_le_bytes());
        buf.extend_from_slice(&self.height.to_le_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            codec: data[0],
            flags: data[1],
            width: u32::from_le_bytes([data[2], data[3], data[4], data[5]]),
            height: u32::from_le_bytes([data[6], data[7], data[8], data[9]]),
            timestamp_ms: u64::from_le_bytes([
                data[10], data[11], data[12], data[13],
                data[14], data[15], data[16], data[17],
            ]),
        })
    }
}

/// Audio frame header
#[derive(Debug, Clone)]
pub struct AudioFrameHeader {
    pub codec: u8,
    pub sample_rate: u32,
    pub channels: u8,
    pub timestamp_ms: u64,
}

impl AudioFrameHeader {
    pub const SIZE: usize = 1 + 4 + 1 + 8; // 14 bytes

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.codec);
        buf.extend_from_slice(&self.sample_rate.to_le_bytes());
        buf.push(self.channels);
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            codec: data[0],
            sample_rate: u32::from_le_bytes([data[1], data[2], data[3], data[4]]),
            channels: data[5],
            timestamp_ms: u64::from_le_bytes([
                data[6], data[7], data[8], data[9],
                data[10], data[11], data[12], data[13],
            ]),
        })
    }
}

/// Encode a complete binary video message
/// Format: [MSG_VIDEO_FRAME(1B)] [total_length(4B)] [header(18B)] [data...]
pub fn encode_video_message(
    codec: u8,
    is_keyframe: bool,
    width: u32,
    height: u32,
    timestamp_ms: u64,
    data: &[u8],
) -> Vec<u8> {
    let payload_len = VideoFrameHeader::SIZE + data.len();
    let total_len = 1 + 4 + payload_len;
    let mut buf = Vec::with_capacity(total_len);

    // Message type
    buf.push(MSG_VIDEO_FRAME);
    // Payload length
    buf.extend_from_slice(&(payload_len as u32).to_le_bytes());
    // Header
    let header = VideoFrameHeader {
        codec,
        flags: if is_keyframe { FLAG_KEYFRAME } else { 0 },
        width,
        height,
        timestamp_ms,
    };
    header.encode(&mut buf);
    // Data
    buf.extend_from_slice(data);

    buf
}

/// Encode a complete binary audio message
pub fn encode_audio_message(
    sample_rate: u32,
    channels: u8,
    timestamp_ms: u64,
    data: &[u8],
) -> Vec<u8> {
    let payload_len = AudioFrameHeader::SIZE + data.len();
    let total_len = 1 + 4 + payload_len;
    let mut buf = Vec::with_capacity(total_len);

    buf.push(MSG_AUDIO_FRAME);
    buf.extend_from_slice(&(payload_len as u32).to_le_bytes());
    let header = AudioFrameHeader {
        codec: CODEC_OPUS,
        sample_rate,
        channels,
        timestamp_ms,
    };
    header.encode(&mut buf);
    buf.extend_from_slice(data);

    buf
}

/// Encode a ping message with timestamp
pub fn encode_ping(timestamp_ms: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(13);
    buf.push(MSG_PING);
    buf.extend_from_slice(&8u32.to_le_bytes());
    buf.extend_from_slice(&timestamp_ms.to_le_bytes());
    buf
}

/// Encode a pong message echoing the timestamp
pub fn encode_pong(timestamp_ms: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(13);
    buf.push(MSG_PONG);
    buf.extend_from_slice(&8u32.to_le_bytes());
    buf.extend_from_slice(&timestamp_ms.to_le_bytes());
    buf
}

/// Parse the message type from a binary message
pub fn parse_message_type(data: &[u8]) -> Option<u8> {
    data.first().copied()
}

/// Parse payload from a binary message (skip type + length header)
pub fn parse_payload(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 5 {
        return None;
    }
    let len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    if data.len() < 5 + len {
        return None;
    }
    Some(&data[5..5 + len])
}

/// Wrapper for the complete decoded frame message
#[derive(Debug)]
pub enum FrameMessage {
    Video {
        header: VideoFrameHeader,
        data: Vec<u8>,
    },
    Audio {
        header: AudioFrameHeader,
        data: Vec<u8>,
    },
    Ping(u64),
    Pong(u64),
    Unknown(u8),
}

/// JSON control messages sent from client → server (text WebSocket messages)
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ControlMsg {
    #[serde(rename = "ping")]
    Ping { timestamp: Option<u64> },

    #[serde(rename = "pong")]
    Pong { timestamp: Option<u64> },

    #[serde(rename = "request_keyframe")]
    RequestKeyframe,

    #[serde(rename = "quality_update")]
    QualityUpdate { quality: String },

    #[serde(rename = "bitrate_update")]
    BitrateUpdate { bitrate_kbps: u32 },

    #[serde(rename = "fps_update")]
    FpsUpdate { fps: u32 },

    #[serde(rename = "network_stats")]
    NetworkStats {
        latency: Option<u32>,
        bandwidth: Option<f32>,
        packet_loss: Option<f32>,
    },

    #[serde(rename = "switch_codec")]
    SwitchCodec { codec: String },
}

/// JSON input messages sent from client → server
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum InputMsg {
    #[serde(rename = "mousemove")]
    MouseMove { x: f64, y: f64, monitor_id: Option<usize> },

    #[serde(rename = "mousedown")]
    MouseDown { x: f64, y: f64, button: u8 },

    #[serde(rename = "mouseup")]
    MouseUp { x: f64, y: f64, button: u8 },

    #[serde(rename = "wheel")]
    Wheel { delta_x: f64, delta_y: f64 },

    #[serde(rename = "keydown")]
    KeyDown { key: String, code: String, modifiers: Option<KeyModifiers> },

    #[serde(rename = "keyup")]
    KeyUp { key: String, code: String, modifiers: Option<KeyModifiers> },
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct KeyModifiers {
    pub ctrl: Option<bool>,
    pub alt: Option<bool>,
    pub shift: Option<bool>,
    pub meta: Option<bool>,
}

/// Server info sent to client on connection
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub r#type: String,
    pub width: u32,
    pub height: u32,
    pub hostname: String,
    pub codec: String,
    pub framerate: u32,
    pub bitrate_kbps: u32,
    pub audio_enabled: bool,
    pub protocol_version: u32,
}
