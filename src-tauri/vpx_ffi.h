// VPX FFI header for bindgen — includes all necessary VPX headers
// to generate Rust FFI bindings matching the system-installed libvpx.
// This mirrors RustDesk's approach in libs/scrap/src/bindings/vpx_ffi.h.

#include <vpx/vpx_codec.h>
#include <vpx/vpx_encoder.h>
#include <vpx/vpx_decoder.h>
#include <vpx/vpx_image.h>
#include <vpx/vp8cx.h>
#include <vpx/vp8dx.h>
