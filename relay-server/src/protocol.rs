//! Protocol definitions for the relay server

use serde::{Deserialize, Serialize};

/// Magic bytes for packet identification
pub const MAGIC: [u8; 4] = [0x43, 0x4B, 0x56, 0x4D]; // "CKVM"

/// Protocol version
pub const VERSION: u8 = 1;

/// Maximum packet size (64KB - UDP headers)
pub const MAX_PACKET_SIZE: usize = 65507;

/// Maximum payload size for video frames
pub const MAX_PAYLOAD_SIZE: usize = 60000;

/// Packet types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Client registration
    Register = 0x01,
    /// Registration acknowledgment
    RegisterAck = 0x02,
    /// Heartbeat/keepalive
    Ping = 0x03,
    /// Heartbeat response
    Pong = 0x04,
    /// Video frame data
    VideoFrame = 0x10,
    /// Audio frame data
    AudioFrame = 0x11,
    /// Input event (mouse/keyboard)
    InputEvent = 0x20,
    /// Peer discovery request
    DiscoverPeers = 0x30,
    /// Peer list response
    PeerList = 0x31,
    /// Direct connection request (P2P)
    ConnectPeer = 0x32,
    /// Connection accepted
    ConnectAck = 0x33,
    /// Disconnect notification
    Disconnect = 0x40,
    /// Error response
    Error = 0xFF,
}

impl TryFrom<u8> for PacketType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, ()> {
        match value {
            0x01 => Ok(PacketType::Register),
            0x02 => Ok(PacketType::RegisterAck),
            0x03 => Ok(PacketType::Ping),
            0x04 => Ok(PacketType::Pong),
            0x10 => Ok(PacketType::VideoFrame),
            0x11 => Ok(PacketType::AudioFrame),
            0x20 => Ok(PacketType::InputEvent),
            0x30 => Ok(PacketType::DiscoverPeers),
            0x31 => Ok(PacketType::PeerList),
            0x32 => Ok(PacketType::ConnectPeer),
            0x33 => Ok(PacketType::ConnectAck),
            0x40 => Ok(PacketType::Disconnect),
            0xFF => Ok(PacketType::Error),
            _ => Err(()),
        }
    }
}

/// Packet header (12 bytes)
/// 
/// ```text
/// +--------+--------+--------+--------+
/// | Magic (4 bytes)                   |
/// +--------+--------+--------+--------+
/// | Version| Type   | Flags  | Reserved|
/// +--------+--------+--------+--------+
/// | Sequence Number (4 bytes)         |
/// +--------+--------+--------+--------+
/// | Payload...                        |
/// ```
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub version: u8,
    pub packet_type: PacketType,
    pub flags: PacketFlags,
    pub sequence: u32,
}

impl PacketHeader {
    pub const SIZE: usize = 12;

    pub fn new(packet_type: PacketType, sequence: u32) -> Self {
        Self {
            version: VERSION,
            packet_type,
            flags: PacketFlags::empty(),
            sequence,
        }
    }

    pub fn with_flags(mut self, flags: PacketFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4] = self.version;
        buf[5] = self.packet_type as u8;
        buf[6] = self.flags.bits();
        buf[7] = 0; // Reserved
        buf[8..12].copy_from_slice(&self.sequence.to_le_bytes());
        buf
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        // Verify magic
        if &data[0..4] != &MAGIC {
            return None;
        }

        let version = data[4];
        let packet_type = PacketType::try_from(data[5]).ok()?;
        let flags = PacketFlags::from_bits_truncate(data[6]);
        let sequence = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        Some(Self {
            version,
            packet_type,
            flags,
            sequence,
        })
    }
}

bitflags::bitflags! {
    /// Packet flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PacketFlags: u8 {
        /// Packet is compressed (LZ4)
        const COMPRESSED = 0x01;
        /// Packet requires acknowledgment
        const RELIABLE = 0x02;
        /// This is a keyframe (video)
        const KEYFRAME = 0x04;
        /// Packet is fragmented
        const FRAGMENTED = 0x08;
        /// Last fragment
        const LAST_FRAGMENT = 0x10;
    }
}

/// Video frame header (after packet header)
#[derive(Debug, Clone)]
pub struct VideoFrameHeader {
    pub width: u16,
    pub height: u16,
    pub timestamp: u64,
    pub frame_number: u32,
    pub fragment_index: u16,
    pub fragment_count: u16,
}

impl VideoFrameHeader {
    pub const SIZE: usize = 20;

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&self.width.to_le_bytes());
        buf[2..4].copy_from_slice(&self.height.to_le_bytes());
        buf[4..12].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[12..16].copy_from_slice(&self.frame_number.to_le_bytes());
        buf[16..18].copy_from_slice(&self.fragment_index.to_le_bytes());
        buf[18..20].copy_from_slice(&self.fragment_count.to_le_bytes());
        buf
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        Some(Self {
            width: u16::from_le_bytes([data[0], data[1]]),
            height: u16::from_le_bytes([data[2], data[3]]),
            timestamp: u64::from_le_bytes([
                data[4], data[5], data[6], data[7],
                data[8], data[9], data[10], data[11],
            ]),
            frame_number: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
            fragment_index: u16::from_le_bytes([data[16], data[17]]),
            fragment_count: u16::from_le_bytes([data[18], data[19]]),
        })
    }
}

/// Registration message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMessage {
    /// Client ID (unique identifier)
    pub client_id: String,
    /// Session ID (for reconnection)
    pub session_id: Option<String>,
    /// Client role (host or viewer)
    pub role: ClientRole,
    /// Requested room/channel
    pub room: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
}

/// Client role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientRole {
    /// Host - sends video/audio
    Host,
    /// Viewer - receives video/audio, sends input
    Viewer,
}

/// Client capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Supports LZ4 compression
    pub supports_compression: bool,
    /// Supports H.264 codec
    pub supports_h264: bool,
    /// Supports audio
    pub supports_audio: bool,
    /// Maximum resolution supported
    pub max_width: u16,
    pub max_height: u16,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub client_id: String,
    pub role: ClientRole,
    pub public_addr: String,
    pub private_addr: Option<String>,
}

/// Input event message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEventMessage {
    #[serde(rename = "mousemove")]
    MouseMove { x: i32, y: i32 },
    #[serde(rename = "mousedown")]
    MouseDown { x: i32, y: i32, button: String },
    #[serde(rename = "mouseup")]
    MouseUp { x: i32, y: i32, button: String },
    #[serde(rename = "wheel")]
    Wheel { x: i32, y: i32, delta_x: f64, delta_y: f64 },
    #[serde(rename = "keydown")]
    KeyDown { key: String, key_code: Option<u32> },
    #[serde(rename = "keyup")]
    KeyUp { key: String, key_code: Option<u32> },
}
