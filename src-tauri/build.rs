fn main() {
    // Generate VPX FFI bindings from system headers using bindgen.
    // This ensures struct layouts (vpx_codec_enc_cfg_t etc.) match the installed
    // libvpx version exactly. The old libvpx-sys 1.4.2 crate had a 376-byte
    // vpx_codec_enc_cfg_t but libvpx 1.14.0 needs 504 bytes, causing stack
    // corruption when vpx_codec_enc_config_default() writes past the struct.
    generate_vpx_bindings();

    // Link system libvpx
    // Rerun if environment changes
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=LIB_VPX_PATH");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLATION_ROOT");
    println!("cargo:rerun-if-env-changed=HOMEBREW_PREFIX");

    // On Windows, we need to tell the linker where to find vpx.lib
    #[cfg(target_os = "windows")]
    {
        let mut lib_path_found = false;
        
        // Check for VCPKG_ROOT environment variable (set by GitHub Actions)
        if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
            let lib_path = format!("{}\\installed\\x64-windows-static-md\\lib", vcpkg_root);
            println!("cargo:warning=Using VCPKG_ROOT lib path: {}", lib_path);
            println!("cargo:rustc-link-search=native={}", lib_path);
            lib_path_found = true;
        }
        // Also check VCPKG_INSTALLATION_ROOT (default on GitHub runners)
        else if let Ok(vcpkg_root) = std::env::var("VCPKG_INSTALLATION_ROOT") {
            let lib_path = format!("{}\\installed\\x64-windows-static-md\\lib", vcpkg_root);
            println!("cargo:warning=Using VCPKG_INSTALLATION_ROOT lib path: {}", lib_path);
            println!("cargo:rustc-link-search=native={}", lib_path);
            lib_path_found = true;
        }
        
        // Also check LIB_VPX_PATH for custom installations
        if let Ok(lib_path) = std::env::var("LIB_VPX_PATH") {
            println!("cargo:warning=Using LIB_VPX_PATH: {}", lib_path);
            println!("cargo:rustc-link-search=native={}", lib_path);
            lib_path_found = true;
        }
        
        if !lib_path_found {
            println!("cargo:warning=No VPX library path found. Set VCPKG_ROOT or LIB_VPX_PATH environment variable.");
        }
        
        // Use static linking on Windows
        println!("cargo:rustc-link-lib=static=vpx");
    }

    // On macOS, libvpx is typically installed via Homebrew
    #[cfg(target_os = "macos")]
    {
        // Check for custom LIB_VPX_PATH first
        if let Ok(lib_path) = std::env::var("LIB_VPX_PATH") {
            println!("cargo:warning=Using LIB_VPX_PATH: {}", lib_path);
            println!("cargo:rustc-link-search=native={}", lib_path);
        } else {
            // Try to find libvpx via pkg-config first
            if let Ok(output) = std::process::Command::new("pkg-config")
                .args(["--libs-only-L", "vpx"])
                .output()
            {
                if output.status.success() {
                    let lib_path = String::from_utf8_lossy(&output.stdout);
                    let lib_path = lib_path.trim().trim_start_matches("-L");
                    if !lib_path.is_empty() {
                        println!("cargo:warning=Using pkg-config vpx lib path: {}", lib_path);
                        println!("cargo:rustc-link-search=native={}", lib_path);
                    }
                }
            }
            
            // Also add common Homebrew paths as fallback
            // Apple Silicon (arm64) Homebrew prefix
            let homebrew_arm64 = "/opt/homebrew/lib";
            // Intel (x86_64) Homebrew prefix
            let homebrew_intel = "/usr/local/lib";
            
            if std::path::Path::new(homebrew_arm64).exists() {
                println!("cargo:warning=Adding Homebrew ARM64 lib path: {}", homebrew_arm64);
                println!("cargo:rustc-link-search=native={}", homebrew_arm64);
            }
            if std::path::Path::new(homebrew_intel).exists() {
                println!("cargo:warning=Adding Homebrew Intel lib path: {}", homebrew_intel);
                println!("cargo:rustc-link-search=native={}", homebrew_intel);
            }
        }
        
        println!("cargo:rustc-link-lib=vpx");
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, libvpx is typically in standard paths, but check LIB_VPX_PATH
        if let Ok(lib_path) = std::env::var("LIB_VPX_PATH") {
            println!("cargo:warning=Using LIB_VPX_PATH: {}", lib_path);
            println!("cargo:rustc-link-search=native={}", lib_path);
        }
        println!("cargo:rustc-link-lib=vpx");
    }

    // Emit the source directory as a compile-time constant.
    // In dev mode this gives the absolute path to src-tauri/ on the build
    // machine, which is used as a reliable fallback when resolve_resource
    // or CWD-relative lookups fail.
    println!(
        "cargo:rustc-env=CLEVER_KVM_MANIFEST_DIR={}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );

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
                    <title>CLEVER KVM</title>
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
                    <h1>CLEVER KVM</h1>
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
