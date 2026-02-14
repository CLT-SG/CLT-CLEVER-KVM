//! Shared web-client path resolution for the KVM server.
//!
//! Resolves the `web-client/` directory that contains the browser-based KVM
//! viewer files (HTML, JS, CSS). The path is resolved once at server startup
//! using the Tauri `AppHandle` (which knows the correct resource location on
//! every platform), then cached in a global `OnceLock` so that both the Axum
//! router (`server.rs`) and the request handlers (`handlers.rs`) can use it
//! without needing their own `AppHandle` reference.
//!
//! ## Resolution order (each candidate must contain `kvm-template.html`)
//! 1. **Tauri resource path** — set via [`init_web_client_path`] at startup.
//!    Works for installed packages on Linux (.deb), macOS (.app) and Windows (.msi/.exe).
//! 2. **Next to the executable** — covers AppImage and portable builds.
//! 3. **Compile-time source path** — `CARGO_MANIFEST_DIR/web-client` baked in at build time.
//!    Reliable for `cargo run` / `npm run tauri dev` on the build machine.
//! 4. **Relative development paths** — CWD-relative lookups as a last resort.
//! 5. **Fallback** — returns `"web-client"` so that errors are informative.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The template file we use to verify a candidate directory is valid.
const TEMPLATE_FILE: &str = "kvm-template.html";

/// Absolute path to `src-tauri/` on the machine that compiled the binary.
/// Only useful for dev builds; in production, Tauri resources or
/// executable-relative paths take precedence.
const MANIFEST_DIR: &str = env!("CLEVER_KVM_MANIFEST_DIR");

/// Global cache for the resolved web-client directory.
static RESOLVED_WEB_CLIENT_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Check whether `dir` contains the KVM template file.
fn is_valid_web_client_dir(dir: &std::path::Path) -> bool {
    dir.is_dir() && dir.join(TEMPLATE_FILE).is_file()
}

/// Initialise the web-client path from a Tauri `AppHandle`.
///
/// Should be called exactly once, inside `WebSocketServer::new`, before any
/// HTTP handler needs the path. Subsequent calls are harmless (the `OnceLock`
/// simply ignores them).
pub fn init_web_client_path(app_handle: &tauri::AppHandle) {
    log::info!("[INFO] Resolving web-client directory...");
    log::info!("[INFO] Compile-time MANIFEST_DIR: {}", MANIFEST_DIR);

    if let Ok(cwd) = std::env::current_dir() {
        log::info!("[INFO] Current working directory: {:?}", cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        log::info!("[INFO] Executable path: {:?}", exe);
    }

    // 1. Tauri resource resolver (platform-specific installed location)
    //   Linux  (deb) : /usr/lib/<app-name>/web-client/
    //   macOS  (.app): Contents/Resources/web-client/
    //   Windows      : next to the .exe
    if let Some(resource_path) = app_handle
        .path_resolver()
        .resolve_resource("web-client")
    {
        log::info!("[INFO] Tauri resolve_resource('web-client') => {:?}", resource_path);
        if is_valid_web_client_dir(&resource_path) {
            log::info!("[OK] Web-client path resolved from Tauri resources: {:?}", resource_path);
            let _ = RESOLVED_WEB_CLIENT_PATH.set(resource_path);
            return;
        }
    } else {
        log::info!("[INFO] Tauri resolve_resource('web-client') returned None");
    }

    // 2. Next to the executable
    if let Ok(exe_path) = std::env::current_exe() {
        // Resolve symlinks so we get the real binary location
        let real_exe = exe_path.canonicalize().unwrap_or(exe_path);
        if let Some(exe_dir) = real_exe.parent() {
            let candidate = exe_dir.join("web-client");
            log::info!("[INFO] Checking exe-relative: {:?}", candidate);
            if is_valid_web_client_dir(&candidate) {
                log::info!("[OK] Web-client path resolved next to executable: {:?}", candidate);
                let _ = RESOLVED_WEB_CLIENT_PATH.set(candidate);
                return;
            }
        }
    }

    // 3. Compile-time source directory (reliable in dev builds)
    let manifest_candidate = PathBuf::from(MANIFEST_DIR).join("web-client");
    log::info!("[INFO] Checking compile-time path: {:?}", manifest_candidate);
    if is_valid_web_client_dir(&manifest_candidate) {
        log::info!("[OK] Web-client path resolved from compile-time MANIFEST_DIR: {:?}", manifest_candidate);
        let _ = RESOLVED_WEB_CLIENT_PATH.set(manifest_candidate);
        return;
    }

    // 4. CWD-relative development paths
    let dev_paths: &[&str] = &[
        "web-client",
        "src-tauri/web-client",
        "../src-tauri/web-client",
    ];
    for path in dev_paths {
        let full_path = PathBuf::from(path);
        log::info!("[INFO] Checking dev path: {:?}", full_path);
        if is_valid_web_client_dir(&full_path) {
            // Canonicalize to absolute path to avoid CWD sensitivity later
            let abs = full_path.canonicalize().unwrap_or(full_path);
            log::info!("[OK] Web-client path resolved from dev path: {:?}", abs);
            let _ = RESOLVED_WEB_CLIENT_PATH.set(abs);
            return;
        }
    }

    log::error!(
        "[ERROR] Could not find web-client directory with {} in any expected location.",
        TEMPLATE_FILE
    );
}

/// Return the absolute path to the `web-client/` directory.
///
/// Uses the path cached by [`init_web_client_path`]. If no valid path was
/// found (or `init` was never called), re-checks fallback locations.
pub fn get_web_client_path() -> PathBuf {
    // Fast path: use cached value
    if let Some(path) = RESOLVED_WEB_CLIENT_PATH.get() {
        return path.clone();
    }

    // Slow path: init was never called or failed. Try all candidates again.
    log::warn!("[WARNING] get_web_client_path called before init or init failed; retrying lookups");

    // Exe-relative
    if let Ok(exe_path) = std::env::current_exe() {
        let real_exe = exe_path.canonicalize().unwrap_or(exe_path);
        if let Some(exe_dir) = real_exe.parent() {
            let candidate = exe_dir.join("web-client");
            if is_valid_web_client_dir(&candidate) {
                return candidate;
            }
        }
    }

    // Compile-time path
    let manifest_candidate = PathBuf::from(MANIFEST_DIR).join("web-client");
    if is_valid_web_client_dir(&manifest_candidate) {
        return manifest_candidate;
    }

    // CWD-relative
    for path in &["web-client", "src-tauri/web-client", "../src-tauri/web-client"] {
        let full_path = PathBuf::from(path);
        if is_valid_web_client_dir(&full_path) {
            return full_path.canonicalize().unwrap_or(full_path);
        }
    }

    // Bare fallback
    log::error!("[ERROR] web-client directory not found; returning bare fallback");
    PathBuf::from("web-client")
}
