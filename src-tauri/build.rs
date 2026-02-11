fn main() {
    // Set FFmpeg environment variables for compilation
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=avformat");
        println!("cargo:rustc-link-lib=avcodec");
        println!("cargo:rustc-link-lib=avutil");
        println!("cargo:rustc-link-lib=swscale");
        println!("cargo:rustc-link-lib=swresample");
    }

    // Generate VPX FFI bindings from system headers using bindgen.
    // This ensures struct layouts (vpx_codec_enc_cfg_t etc.) match the installed
    // libvpx version exactly. The old libvpx-sys 1.4.2 crate had a 376-byte
    // vpx_codec_enc_cfg_t but libvpx 1.14.0 needs 504 bytes, causing stack
    // corruption when vpx_codec_enc_config_default() writes past the struct.
    generate_vpx_bindings();

    // Link system libvpx
    println!("cargo:rustc-link-lib=vpx");

    // Create web-client directory if it doesn't exist
    let web_client_dir = std::path::Path::new("web-client");
    if !web_client_dir.exists() {
        std::fs::create_dir_all(web_client_dir).expect("Failed to create web-client directory");
        
        // Create a basic index.html file as a placeholder
        let index_html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Clever KVM</title>
    <style>
        body {
            font-family: sans-serif;
            margin: 0;
            padding: 20px;
            text-align: center;
        }
        h1 {
            color: #2c3e50;
        }
        p {
            color: #7f8c8d;
        }
    </style>
</head>
<body>
    <h1>Clever KVM</h1>
    <p>To access the KVM functionality, use the /kvm endpoint.</p>
    <p>Example: <a href="/kvm">Open KVM Client</a></p>
</body>
</html>
"#;
        
        std::fs::write(web_client_dir.join("index.html"), index_html)
            .expect("Failed to create index.html");
    }
    let kvm_dir = std::path::Path::new("kvm");
    if !kvm_dir.exists() {
        std::fs::create_dir_all(kvm_dir).expect("Failed to create kvm directory");
    }
    
    tauri_build::build()
}

/// Generate VPX FFI bindings from system libvpx headers using bindgen.
/// This mirrors RustDesk's approach (libs/scrap/build.rs → gen_vcpkg_package).
/// By generating bindings from the *installed* headers, we guarantee struct
/// layouts like vpx_codec_enc_cfg_t, vpx_codec_ctx_t, vpx_image_t match
/// exactly, regardless of which libvpx version is installed.
fn generate_vpx_bindings() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    // Re-run if header changes
    println!("cargo:rerun-if-changed=vpx_ffi.h");

    let bindings = bindgen::Builder::default()
        .header("vpx_ffi.h")
        // Only generate bindings for VPX symbols
        .allowlist_type("^[vV].*")
        .allowlist_function("^vpx_.*")
        .allowlist_var("^(?i)(vpx_|VP[89]|VPX_).*")
        // Derive useful traits
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        // Use libc types for C type compatibility
        .ctypes_prefix("libc")
        .raw_line("use libc;")
        .generate()
        .expect("Failed to generate VPX bindings. Is libvpx-dev installed?");

    let out_path = std::path::PathBuf::from(&out_dir).join("vpx_ffi.rs");
    bindings
        .write_to_file(&out_path)
        .expect("Failed to write vpx_ffi.rs");

    println!("cargo:warning=Generated VPX FFI bindings at {}", out_path.display());
}
