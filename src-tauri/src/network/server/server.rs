use axum::{
    routing::get,
    Router,
    handler::HandlerWithoutStateExt,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::{
    sync::{mpsc, broadcast},
    task::JoinHandle,
};
use tauri::AppHandle;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use std::convert::Infallible;
use axum::http::{StatusCode, Response};
use axum::body::Body;
use axum_server::tls_rustls::RustlsConfig;

use super::handlers::{kvm_client_handler, static_file_handler, ws_handler_with_stop};
use super::web_client_path;

fn get_web_client_path() -> PathBuf {
    web_client_path::get_web_client_path()
}

pub struct WebSocketServer {
    shutdown_tx: mpsc::Sender<()>,
    server_handle: JoinHandle<()>,
    // Add broadcast channel for signaling all connections to stop
    stop_broadcast: broadcast::Sender<()>,
}

// Define a simple function that returns a 404 response
async fn handle_404() -> Result<Response<Body>, Infallible> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap())
}

/// Get the directory for persisting TLS certificates across restarts.
/// Uses the system data directory or falls back to a local `.tls` directory.
fn get_cert_dir() -> PathBuf {
    // Try XDG data home first, then fallback to local directory
    if let Ok(data_dir) = std::env::var("XDG_DATA_HOME") {
        let dir = PathBuf::from(data_dir).join("clever-kvm");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home).join(".local/share/clever-kvm");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    // Last resort: local .tls directory
    let dir = PathBuf::from(".tls");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Load or generate a self-signed TLS certificate for HTTPS.
///
/// Certificates are persisted to disk so that the same certificate is reused
/// across application restarts. This is critical because when a web client has
/// already accepted a self-signed certificate, regenerating it on restart would
/// cause the browser to reject subsequent `wss://` WebSocket connections
/// (the browser has no opportunity to re-accept a changed certificate via
/// WebSocket — only a full page reload and manual re-acceptance would work).
///
/// The certificate is regenerated only when:
/// - No persisted certificate exists yet (first run)
/// - The persisted certificate files are corrupt or unreadable
fn load_or_generate_cert() -> Result<(Vec<u8>, Vec<u8>), String> {
    let cert_dir = get_cert_dir();
    let cert_path = cert_dir.join("server.crt");
    let key_path = cert_dir.join("server.key");

    // Try to load existing certificate
    if cert_path.exists() && key_path.exists() {
        match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
            (Ok(cert_pem), Ok(key_pem)) if !cert_pem.is_empty() && !key_pem.is_empty() => {
                log::info!(
                    "[OK] Loaded persisted TLS certificate from {}",
                    cert_dir.display()
                );
                return Ok((cert_pem, key_pem));
            }
            _ => {
                log::warn!(
                    "Persisted TLS certificate files are corrupt, regenerating..."
                );
            }
        }
    }

    // Generate a new certificate
    let (cert_pem, key_pem) = generate_self_signed_cert()?;

    // Persist to disk for future restarts
    if let Err(e) = std::fs::write(&cert_path, &cert_pem) {
        log::warn!("Failed to persist TLS certificate: {}", e);
    }
    if let Err(e) = std::fs::write(&key_path, &key_pem) {
        log::warn!("Failed to persist TLS private key: {}", e);
    }

    if cert_path.exists() && key_path.exists() {
        log::info!(
            "[OK] TLS certificate persisted to {} (reused across restarts)",
            cert_dir.display()
        );
    }

    Ok((cert_pem, key_pem))
}

/// Generate a self-signed TLS certificate for HTTPS.
/// WebCodecs API requires a secure context (HTTPS) when accessed from non-localhost.
fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>), String> {
    use rcgen::generate_simple_self_signed;
    
    let mut subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ];
    
    // Add local network IP so the cert is valid for LAN access
    if let Ok(ip) = local_ip_address::local_ip() {
        subject_alt_names.push(ip.to_string());
    }
    
    // Add all network interface IPs for broader LAN compatibility
    if let Ok(interfaces) = local_ip_address::list_afinet_netifas() {
        for (_, ip) in interfaces {
            let ip_str = ip.to_string();
            if !ip.is_loopback() && !subject_alt_names.contains(&ip_str) {
                subject_alt_names.push(ip_str);
            }
        }
    }
    
    log::info!("Generating self-signed TLS certificate with SANs: {:?}", subject_alt_names);
    
    let cert = generate_simple_self_signed(subject_alt_names)
        .map_err(|e| format!("Failed to generate self-signed certificate: {}", e))?;
    
    let cert_pem = cert.serialize_pem().map_err(|e| format!("Failed to serialize cert PEM: {}", e))?.into_bytes();
    let key_pem = cert.serialize_private_key_pem().into_bytes();
    
    log::info!("[OK] Self-signed TLS certificate generated for HTTPS (WebCodecs secure context)");
    
    Ok((cert_pem, key_pem))
}

impl WebSocketServer {
    pub async fn new(port: u16, _app_handle: AppHandle) -> Result<Self, String> {
        // Resolve web-client resource path from the Tauri AppHandle.
        // This must happen before any HTTP handler tries to serve the files.
        web_client_path::init_web_client_path(&_app_handle);

        // Channel for shutdown signal
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        
        // Broadcast channel for stopping all connections
        let (stop_broadcast, _) = broadcast::channel::<()>(10);
        let stop_broadcast_clone = stop_broadcast.clone();
        
        // Get the correct web-client path
        let web_client_path = get_web_client_path();
        log::info!("Using web-client directory: {:?}", web_client_path);
        
        // Set up the router
        let app = Router::new()
            .route("/ws", get(move |ws: axum::extract::ws::WebSocketUpgrade, query: axum::extract::Query<std::collections::HashMap<String, String>>| {
                let stop_rx = stop_broadcast_clone.subscribe();
                async move { ws_handler_with_stop(ws, query, stop_rx).await }
            }))
            .route("/kvm", get(kvm_client_handler))
            .route("/static/*path", get(static_file_handler))
            .fallback_service(
                ServeDir::new(&web_client_path)
                    .append_index_html_on_directories(true)
                    .not_found_service(handle_404.into_service())
            )
            .layer(TraceLayer::new_for_http());

        // Bind address to all interfaces (0.0.0.0)
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        log::info!("Attempting to bind HTTPS server to address: {}", addr);
        
        // Load persisted TLS certificate or generate a new one.
        // Persisting the cert ensures browsers that previously accepted it
        // can reconnect via wss:// after a server restart without needing
        // to re-accept a new certificate.
        let (cert_pem, key_pem) = load_or_generate_cert()?;
        let tls_config = RustlsConfig::from_pem(cert_pem, key_pem).await
            .map_err(|e| format!("Failed to configure TLS: {}", e))?;
        
        log::info!("[INFO] HTTPS/WSS server listening on {}", addr);
        log::info!("    WebCodecs API will be available (secure context)");

        // Create HTTPS server with axum-server
        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        
        let server = axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service());

        // Spawn the server task
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("Server error: {}", e);
            }
        });
        
        // Spawn a task to listen for shutdown signal
        let shutdown_tx_clone = shutdown_tx.clone();
        tokio::spawn(async move {
            shutdown_rx.recv().await;
            log::info!("Shutdown signal received, stopping HTTPS server...");
            shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(2)));
        });

        Ok(WebSocketServer {
            shutdown_tx,
            server_handle,
            stop_broadcast,
        })
    }

    pub async fn shutdown(self) {
        // Signal all connections to stop
        let _ = self.stop_broadcast.send(());
        
        // Give connections a moment to clean up
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Send shutdown signal
        if let Err(e) = self.shutdown_tx.send(()).await {
            log::error!("Failed to send shutdown signal: {}", e);
        }

        // Wait for server to shutdown
        if let Err(e) = self.server_handle.await {
            log::error!("Failed to join server task: {}", e);
        }

        log::info!("HTTPS server shut down");
    }
}
