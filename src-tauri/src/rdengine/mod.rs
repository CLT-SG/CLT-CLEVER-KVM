//! RustDesk-Inspired Streaming Engine
//!
//! This module implements a low-latency remote desktop streaming engine
//! following the architectural patterns used by RustDesk:
//!
//! - **Dedicated video thread** per display (not async)
//! - **Real VPX (VP8/VP9) encoding** via libvpx
//! - **Frame deduplication** — byte-compare before encoding
//! - **Buffer reuse** — pre-allocated YUV buffers
//! - **Adaptive QoS** — dynamic FPS/bitrate based on network RTT
//! - **Separated video/control channels** to prevent HOL blocking
//! - **Real audio capture** via cpal + Opus encoding
//! - **Binary protocol** — minimal-overhead length-prefixed frames
//! - **Cursor synchronization** — host cursor position + shape streamed to client

pub mod codec;
pub mod video_service;
pub mod audio_service;
pub mod cursor_service;
pub mod input_service;
pub mod connection;
pub mod qos;
pub mod protocol;
pub mod webrtc_transport;

pub use codec::{VpxEncoder, VpxConfig, EncoderApi, EncodeInput};
pub use video_service::VideoService;
pub use audio_service::AudioService;
#[allow(unused_imports)]
pub use cursor_service::CursorService;
pub use input_service::{InputService, InputServiceConfig, InputSender};
pub use connection::{ConnectionHandler, ConnectionConfig};
pub use qos::QualityControl;
pub use protocol::{FrameMessage, ControlMsg, InputMsg};
pub use webrtc_transport::{WebRtcTransport, WebRtcConfig};
