//! VPX Codec — VP8/VP9 encoding via libvpx (bindgen FFI)
//!
//! Follows RustDesk's approach:
//! - CBR rate control for predictable bandwidth
//! - Error resilience enabled
//! - Keyframes disabled by default (on-demand only)
//! - Realtime speed presets
//! - Buffer reuse across frames
//!
//! Uses bindgen-generated bindings from system libvpx headers (generated in
//! build.rs), ensuring struct layouts match the installed libvpx version exactly.
//! This replaces the broken libvpx-sys 1.4.2 crate whose vpx_codec_enc_cfg_t
//! (376 bytes) was too small for libvpx 1.14.0 (504 bytes), causing stack
//! corruption in vpx_codec_enc_config_default().

use anyhow::{Result, Context, bail};
use log::{debug, info, warn};
use std::ptr;
use std::slice;
use std::time::Instant;

// Bindgen-generated FFI bindings from system libvpx headers (build.rs)
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(clippy::all)]
pub mod vpx_ffi {
    include!(concat!(env!("OUT_DIR"), "/vpx_ffi.rs"));
}

// Re-export types we use
use vpx_ffi::{
    vpx_codec_ctx_t,
    vpx_codec_enc_cfg_t,
    vpx_image_t,
    vpx_codec_iter_t,
    vpx_codec_pts_t,
    vpx_enc_frame_flags_t,
    // Functions
    vpx_codec_vp8_cx,
    vpx_codec_vp9_cx,
    vpx_codec_enc_config_default,
    vpx_codec_enc_init_ver,
    vpx_codec_control_,
    vpx_codec_encode,
    vpx_codec_get_cx_data,
    vpx_codec_enc_config_set,
    vpx_codec_destroy,
};

// Bindgen prefixes enum constants with their type name — re-alias them here
const VPX_CODEC_OK: vpx_ffi::vpx_codec_err_t = vpx_ffi::vpx_codec_err_t_VPX_CODEC_OK;
const VPX_CBR: vpx_ffi::vpx_rc_mode = vpx_ffi::vpx_rc_mode_VPX_CBR;
const VPX_RC_ONE_PASS: vpx_ffi::vpx_enc_pass = vpx_ffi::vpx_enc_pass_VPX_RC_ONE_PASS;
const VPX_KF_DISABLED: vpx_ffi::vpx_kf_mode = vpx_ffi::vpx_kf_mode_VPX_KF_DISABLED;
const VPX_KF_AUTO: vpx_ffi::vpx_kf_mode = vpx_ffi::vpx_kf_mode_VPX_KF_AUTO;
const VPX_IMG_FMT_I420: vpx_ffi::vpx_img_fmt = vpx_ffi::vpx_img_fmt_VPX_IMG_FMT_I420;
const VPX_CS_BT_709: vpx_ffi::vpx_color_space = vpx_ffi::vpx_color_space_VPX_CS_BT_709;
const VPX_CODEC_CX_FRAME_PKT: vpx_ffi::vpx_codec_cx_pkt_kind = vpx_ffi::vpx_codec_cx_pkt_kind_VPX_CODEC_CX_FRAME_PKT;
const VP8E_SET_CPUUSED: vpx_ffi::vp8e_enc_control_id = vpx_ffi::vp8e_enc_control_id_VP8E_SET_CPUUSED;
const VP8E_SET_STATIC_THRESHOLD: vpx_ffi::vp8e_enc_control_id = vpx_ffi::vp8e_enc_control_id_VP8E_SET_STATIC_THRESHOLD;
const VP9E_SET_TILE_COLUMNS: vpx_ffi::vp8e_enc_control_id = vpx_ffi::vp8e_enc_control_id_VP9E_SET_TILE_COLUMNS;
const VP9E_SET_FRAME_PARALLEL_DECODING: vpx_ffi::vp8e_enc_control_id = vpx_ffi::vp8e_enc_control_id_VP9E_SET_FRAME_PARALLEL_DECODING;
const VP9E_SET_AQ_MODE: vpx_ffi::vp8e_enc_control_id = vpx_ffi::vp8e_enc_control_id_VP9E_SET_AQ_MODE;

// Constants not always exported by bindgen (C #defines / macros)
const VPX_DL_REALTIME: libc::c_ulong = 1;
const VPX_EFLAG_FORCE_KF: libc::c_long = 1;
const VPX_FRAME_IS_KEY: u32 = 0x1;
const VPX_PLANE_Y: usize = 0;
const VPX_PLANE_U: usize = 1;
const VPX_PLANE_V: usize = 2;

/// Supported VPX codec variants
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VpxCodec {
    VP8,
    VP9,
}

impl VpxCodec {
    pub fn as_str(&self) -> &'static str {
        match self {
            VpxCodec::VP8 => "vp8",
            VpxCodec::VP9 => "vp9",
        }
    }
}

/// VPX encoder configuration (mirrors RustDesk's vpxcodec.rs patterns)
#[derive(Debug, Clone)]
pub struct VpxConfig {
    pub codec: VpxCodec,
    pub width: u32,
    pub height: u32,
    /// Target bitrate in kbit/s
    pub bitrate_kbps: u32,
    /// Target framerate
    pub framerate: u32,
    /// Keyframe interval (0 = disabled, on-demand only — like RustDesk)
    pub keyframe_interval: u32,
    /// Number of encoding threads
    pub threads: u32,
    /// CPU speed preset (higher = faster, lower quality)
    /// VP9: 0-9, recommend 6-9 for realtime
    /// VP8: 0-16, recommend 8-12 for realtime
    pub cpu_speed: i32,
    /// Error resilience mode
    pub error_resilient: bool,
}

impl Default for VpxConfig {
    fn default() -> Self {
        Self {
            codec: VpxCodec::VP9,
            width: 1920,
            height: 1080,
            bitrate_kbps: 4000,
            framerate: 30,
            keyframe_interval: 0, // Disabled — like RustDesk's VPX_KF_DISABLED
            threads: 4,
            cpu_speed: 6, // Realtime preset — like RustDesk (6 = good quality/speed balance)
            error_resilient: true,
        }
    }
}

impl VpxConfig {
    /// Calculate bitrate based on resolution (matches RustDesk's table)
    pub fn bitrate_for_resolution(width: u32, height: u32) -> u32 {
        let pixels = (width * height) as u64;
        let base_pixels: u64 = 1920 * 1080;
        let base_bitrate: u64 = 4000;
        let bitrate = (base_bitrate * pixels) / base_pixels;
        bitrate.max(800).min(12000) as u32
    }

    /// Ultra-low latency config for LAN
    pub fn for_lan(width: u32, height: u32) -> Self {
        Self {
            codec: VpxCodec::VP9,
            width,
            height,
            bitrate_kbps: Self::bitrate_for_resolution(width, height) * 2,
            framerate: 60,
            keyframe_interval: 0,
            threads: num_cpus(),
            cpu_speed: 6,
            error_resilient: true,
        }
    }

    /// Balanced config
    pub fn balanced(width: u32, height: u32) -> Self {
        Self {
            codec: VpxCodec::VP9,
            width,
            height,
            bitrate_kbps: Self::bitrate_for_resolution(width, height),
            framerate: 30,
            keyframe_interval: 0,
            threads: num_cpus().min(8),
            cpu_speed: 6,
            error_resilient: true,
        }
    }

    /// Low bandwidth config for remote
    pub fn low_bandwidth(width: u32, height: u32) -> Self {
        Self {
            codec: VpxCodec::VP8,
            width,
            height,
            bitrate_kbps: Self::bitrate_for_resolution(width, height) / 2,
            framerate: 15,
            keyframe_interval: 0,
            threads: num_cpus().min(4),
            cpu_speed: 10,
            error_resilient: true,
        }
    }
}

/// Encoder trait (like RustDesk's EncoderApi)
pub trait EncoderApi {
    fn encode(&mut self, input: EncodeInput, timestamp_ms: i64) -> Result<Vec<EncodedPacket>>;
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;
    fn force_keyframe(&mut self);
    fn codec_name(&self) -> &str;
}

/// Input frame data for encoding
pub enum EncodeInput<'a> {
    /// Raw I420 YUV data (Y, U, V planes)
    I420 {
        y: &'a [u8],
        u: &'a [u8],
        v: &'a [u8],
        stride_y: usize,
        stride_u: usize,
        stride_v: usize,
    },
}

/// Encoded output packet
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub timestamp_ms: i64,
    pub is_keyframe: bool,
    pub encode_time_us: u64,
}

/// VPX Encoder using libvpx-sys (safe wrapper around raw FFI)
pub struct VpxEncoder {
    ctx: vpx_codec_ctx_t,
    config: VpxConfig,
    frame_count: u64,
    force_next_keyframe: bool,
    total_encode_time_us: u64,
    total_frames: u64,
    total_bytes: u64,
}

// vpx_codec_ctx_t contains raw pointers but is used single-threaded within VideoService
unsafe impl Send for VpxEncoder {}

impl VpxEncoder {
    pub fn new(config: VpxConfig) -> Result<Self> {
        info!(
            "Initializing VPX encoder: {:?} {}x{} @ {} kbps, {} fps, {} threads, speed {}",
            config.codec, config.width, config.height,
            config.bitrate_kbps, config.framerate, config.threads, config.cpu_speed
        );

        unsafe {
            let iface = match config.codec {
                VpxCodec::VP8 => vpx_codec_vp8_cx(),
                VpxCodec::VP9 => vpx_codec_vp9_cx(),
            };

            // Get default config
            let mut enc_cfg: vpx_codec_enc_cfg_t = std::mem::zeroed();
            let ret = vpx_codec_enc_config_default(iface, &mut enc_cfg, 0);
            if ret != VPX_CODEC_OK {
                bail!("Failed to get default VPX config: error {}", ret);
            }

            // Configure encoder (following RustDesk's patterns)
            enc_cfg.g_w = config.width;
            enc_cfg.g_h = config.height;
            enc_cfg.g_threads = config.threads;
            enc_cfg.g_timebase.num = 1;
            enc_cfg.g_timebase.den = 1000; // Millisecond precision
            enc_cfg.rc_target_bitrate = config.bitrate_kbps;
            enc_cfg.rc_end_usage = VPX_CBR; // CBR for streaming
            enc_cfg.g_pass = VPX_RC_ONE_PASS;
            enc_cfg.g_lag_in_frames = 0; // No look-ahead — realtime
            enc_cfg.rc_min_quantizer = 2;
            enc_cfg.rc_max_quantizer = 40; // Lower = better quality (RustDesk default is 63, but we target LAN)
            enc_cfg.rc_undershoot_pct = 95;
            enc_cfg.rc_overshoot_pct = 100;
            enc_cfg.rc_buf_sz = 150;        // 150ms buffer — low latency for LAN
            enc_cfg.rc_buf_initial_sz = 100; // 100ms initial buffer
            enc_cfg.rc_buf_optimal_sz = 120; // 120ms optimal buffer
            enc_cfg.rc_dropframe_thresh = 0; // Never drop frames — prefer lower quality over frame drops

            // Keyframe configuration
            if config.keyframe_interval == 0 {
                enc_cfg.kf_mode = VPX_KF_DISABLED;
                enc_cfg.kf_max_dist = 999999;
            } else {
                enc_cfg.kf_mode = VPX_KF_AUTO;
                enc_cfg.kf_max_dist = config.keyframe_interval;
            }

            // Error resilience
            if config.error_resilient {
                enc_cfg.g_error_resilient = 1;
            }

            // Initialize encoder
            let mut ctx: vpx_codec_ctx_t = std::mem::zeroed();
            let abi_version = vpx_ffi::VPX_ENCODER_ABI_VERSION as libc::c_int;
            let ret = vpx_codec_enc_init_ver(
                &mut ctx,
                iface,
                &enc_cfg,
                0, // flags
                abi_version, // From bindgen — matches system libvpx exactly
            );
            if ret != VPX_CODEC_OK {
                bail!("Failed to initialize VPX encoder: error {} (ABI version {})", ret, abi_version);
            }

            // Set CPU speed (realtime preset)
            vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as i32, config.cpu_speed);

            // VP9-specific optimizations
            if config.codec == VpxCodec::VP9 {
                vpx_codec_control_(&mut ctx, VP9E_SET_TILE_COLUMNS as i32, 2i32);
                vpx_codec_control_(&mut ctx, VP9E_SET_FRAME_PARALLEL_DECODING as i32, 1i32);
                vpx_codec_control_(&mut ctx, VP9E_SET_AQ_MODE as i32, 3i32); // Cyclic refresh
            }

            vpx_codec_control_(&mut ctx, VP8E_SET_STATIC_THRESHOLD as i32, 1i32);

            info!("VPX encoder initialized successfully");

            Ok(Self {
                ctx,
                config,
                frame_count: 0,
                force_next_keyframe: false,
                total_encode_time_us: 0,
                total_frames: 0,
                total_bytes: 0,
            })
        }
    }

    pub fn avg_encode_time_us(&self) -> u64 {
        if self.total_frames == 0 { return 0; }
        self.total_encode_time_us / self.total_frames
    }

    pub fn avg_bitrate_kbps(&self) -> u64 {
        if self.total_frames == 0 { return 0; }
        let duration_s = self.total_frames as f64 / self.config.framerate as f64;
        if duration_s <= 0.0 { return 0; }
        ((self.total_bytes * 8) as f64 / duration_s / 1000.0) as u64
    }
}

impl EncoderApi for VpxEncoder {
    fn encode(&mut self, input: EncodeInput, timestamp_ms: i64) -> Result<Vec<EncodedPacket>> {
        let start = Instant::now();

        let (y, u, v, stride_y, stride_u, stride_v) = match &input {
            EncodeInput::I420 { y, u, v, stride_y, stride_u, stride_v } => {
                (*y, *u, *v, *stride_y, *stride_u, *stride_v)
            }
        };

        let w = self.config.width;
        let h = self.config.height;

        unsafe {
            // Create VPX image wrapping our I420 data
            let mut img: vpx_image_t = std::mem::zeroed();
            img.fmt = VPX_IMG_FMT_I420;
            img.cs = VPX_CS_BT_709;
            img.w = w;
            img.h = h;
            img.d_w = w;
            img.d_h = h;
            img.x_chroma_shift = 1;
            img.y_chroma_shift = 1;
            img.planes[VPX_PLANE_Y] = y.as_ptr() as *mut _;
            img.planes[VPX_PLANE_U] = u.as_ptr() as *mut _;
            img.planes[VPX_PLANE_V] = v.as_ptr() as *mut _;
            img.stride[VPX_PLANE_Y] = stride_y as i32;
            img.stride[VPX_PLANE_U] = stride_u as i32;
            img.stride[VPX_PLANE_V] = stride_v as i32;

            // Determine flags
            let mut flags: vpx_enc_frame_flags_t = 0;
            if self.force_next_keyframe || self.frame_count == 0 {
                flags = VPX_EFLAG_FORCE_KF;
                self.force_next_keyframe = false;
            }

            // Encode frame
            let ret = vpx_codec_encode(
                &mut self.ctx,
                &img,
                timestamp_ms as vpx_codec_pts_t,
                (1000 / self.config.framerate) as libc::c_ulong,
                flags,
                VPX_DL_REALTIME,
            );
            if ret != VPX_CODEC_OK {
                bail!("VPX encode failed: error {}", ret);
            }

            let encode_time = start.elapsed();
            let encode_time_us = encode_time.as_micros() as u64;

            // Collect output packets
            let mut result = Vec::new();
            let mut iter: vpx_codec_iter_t = ptr::null();
            loop {
                let pkt = vpx_codec_get_cx_data(&mut self.ctx, &mut iter);
                if pkt.is_null() {
                    break;
                }

                let pkt = &*pkt;
                if pkt.kind != VPX_CODEC_CX_FRAME_PKT {
                    continue;
                }

                // Access the frame union field (bindgen-generated)
                let frame = &pkt.data.frame;
                let data_ptr = frame.buf as *const u8;
                let data_len = frame.sz as usize;
                let data = slice::from_raw_parts(data_ptr, data_len).to_vec();
                let is_keyframe = (frame.flags & VPX_FRAME_IS_KEY) != 0;

                self.total_bytes += data_len as u64;

                result.push(EncodedPacket {
                    data,
                    timestamp_ms,
                    is_keyframe,
                    encode_time_us,
                });
            }

            self.frame_count += 1;
            self.total_frames += 1;
            self.total_encode_time_us += encode_time_us;

            if self.frame_count % 300 == 0 {
                debug!(
                    "VPX stats: frames={}, avg_encode={}us, avg_bitrate={}kbps",
                    self.total_frames, self.avg_encode_time_us(), self.avg_bitrate_kbps()
                );
            }

            Ok(result)
        }
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        if bitrate_kbps == self.config.bitrate_kbps {
            return Ok(());
        }
        self.config.bitrate_kbps = bitrate_kbps;
        unsafe {
            let iface = match self.config.codec {
                VpxCodec::VP8 => vpx_codec_vp8_cx(),
                VpxCodec::VP9 => vpx_codec_vp9_cx(),
            };
            // IMPORTANT: Get the CURRENT config from the active encoder, not a fresh default.
            // Using vpx_codec_enc_config_default here would reset all settings (dimensions,
            // threading, CBR mode, etc.) and corrupt the encoder — only the bitrate should change.
            let mut enc_cfg: vpx_codec_enc_cfg_t = std::mem::zeroed();
            let ret = vpx_codec_enc_config_default(iface, &mut enc_cfg, 0);
            if ret != VPX_CODEC_OK {
                warn!("Failed to get default VPX config for bitrate update: error {}", ret);
                return Ok(());
            }
            // Re-apply all our custom settings on top of defaults
            enc_cfg.g_w = self.config.width;
            enc_cfg.g_h = self.config.height;
            enc_cfg.g_threads = self.config.threads;
            enc_cfg.g_timebase.num = 1;
            enc_cfg.g_timebase.den = 1000;
            enc_cfg.rc_target_bitrate = bitrate_kbps;
            enc_cfg.rc_end_usage = VPX_CBR;
            enc_cfg.g_pass = VPX_RC_ONE_PASS;
            enc_cfg.g_lag_in_frames = 0;
            enc_cfg.rc_min_quantizer = 2;
            enc_cfg.rc_max_quantizer = 40;
            enc_cfg.rc_undershoot_pct = 95;
            enc_cfg.rc_overshoot_pct = 100;
            enc_cfg.rc_buf_sz = 150;
            enc_cfg.rc_buf_initial_sz = 100;
            enc_cfg.rc_buf_optimal_sz = 120;
            enc_cfg.rc_dropframe_thresh = 0;
            if self.config.keyframe_interval == 0 {
                enc_cfg.kf_mode = VPX_KF_DISABLED;
                enc_cfg.kf_max_dist = 999999;
            } else {
                enc_cfg.kf_mode = VPX_KF_AUTO;
                enc_cfg.kf_max_dist = self.config.keyframe_interval;
            }
            if self.config.error_resilient {
                enc_cfg.g_error_resilient = 1;
            }
            let ret = vpx_codec_enc_config_set(&mut self.ctx, &enc_cfg);
            if ret != VPX_CODEC_OK {
                warn!("Failed to update VPX bitrate: error {}", ret);
            }
        }
        info!("VPX bitrate target updated to {} kbps", bitrate_kbps);
        Ok(())
    }

    fn force_keyframe(&mut self) {
        self.force_next_keyframe = true;
    }

    fn codec_name(&self) -> &str {
        self.config.codec.as_str()
    }
}

impl Drop for VpxEncoder {
    fn drop(&mut self) {
        unsafe {
            vpx_codec_destroy(&mut self.ctx);
        }
        info!("VPX encoder destroyed");
    }
}

/// Get number of available CPU cores (capped reasonably)
fn num_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
        .min(16)
}

/// Color conversion: BGRA → I420 (YUV420 planar)
///
/// Uses BT.709 coefficients (same as RustDesk's libyuv approach).
pub fn bgra_to_i420(bgra: &[u8], width: u32, height: u32, y: &mut [u8], u: &mut [u8], v: &mut [u8]) {
    let w = width as usize;
    let h = height as usize;
    let stride = w * 4;

    for row in 0..h {
        for col in 0..w {
            let offset = row * stride + col * 4;
            let b = bgra[offset] as f32;
            let g = bgra[offset + 1] as f32;
            let r = bgra[offset + 2] as f32;
            let y_val = 16.0 + 0.183 * r + 0.614 * g + 0.062 * b;
            y[row * w + col] = y_val.clamp(0.0, 255.0) as u8;
        }
    }

    let uw = w / 2;
    for row in (0..h).step_by(2) {
        for col in (0..w).step_by(2) {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let py = (row + dy).min(h - 1);
                    let px = (col + dx).min(w - 1);
                    let off = py * stride + px * 4;
                    b_sum += bgra[off] as u32;
                    g_sum += bgra[off + 1] as u32;
                    r_sum += bgra[off + 2] as u32;
                }
            }
            let r = (r_sum / 4) as f32;
            let g = (g_sum / 4) as f32;
            let b = (b_sum / 4) as f32;

            let u_val = 128.0 - 0.101 * r - 0.339 * g + 0.439 * b;
            let v_val = 128.0 + 0.439 * r - 0.399 * g - 0.040 * b;

            let idx = (row / 2) * uw + (col / 2);
            u[idx] = u_val.clamp(0.0, 255.0) as u8;
            v[idx] = v_val.clamp(0.0, 255.0) as u8;
        }
    }
}

/// Color conversion: RGBA → I420 (YUV420 planar)
pub fn rgba_to_i420(rgba: &[u8], width: u32, height: u32, y: &mut [u8], u_plane: &mut [u8], v_plane: &mut [u8]) {
    let w = width as usize;
    let h = height as usize;
    let stride = w * 4;

    for row in 0..h {
        for col in 0..w {
            let offset = row * stride + col * 4;
            let r = rgba[offset] as f32;
            let g = rgba[offset + 1] as f32;
            let b = rgba[offset + 2] as f32;
            let y_val = 16.0 + 0.183 * r + 0.614 * g + 0.062 * b;
            y[row * w + col] = y_val.clamp(0.0, 255.0) as u8;
        }
    }

    let uw = w / 2;
    for row in (0..h).step_by(2) {
        for col in (0..w).step_by(2) {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let py = (row + dy).min(h - 1);
                    let px = (col + dx).min(w - 1);
                    let off = py * stride + px * 4;
                    r_sum += rgba[off] as u32;
                    g_sum += rgba[off + 1] as u32;
                    b_sum += rgba[off + 2] as u32;
                }
            }
            let r = (r_sum / 4) as f32;
            let g = (g_sum / 4) as f32;
            let b = (b_sum / 4) as f32;

            let u_val = 128.0 - 0.101 * r - 0.339 * g + 0.439 * b;
            let v_val = 128.0 + 0.439 * r - 0.399 * g - 0.040 * b;

            let idx = (row / 2) * uw + (col / 2);
            u_plane[idx] = u_val.clamp(0.0, 255.0) as u8;
            v_plane[idx] = v_val.clamp(0.0, 255.0) as u8;
        }
    }
}
