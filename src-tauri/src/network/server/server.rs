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

fn get_web_client_path() -> PathBuf {
    // Try multiple possible locations for the web-client directory
    let possible_paths = vec![
        "web-client",                           // Current working directory
        "src-tauri/web-client",                 // From project root
        "../src-tauri/web-client",              // From dist directory  
        "./src-tauri/web-client",               // Alternative from root
    ];
    
    for path in possible_paths {
        let full_path = PathBuf::from(path);
        if full_path.exists() && full_path.is_dir() {
            return full_path;
        }
    }
    
    // Fallback to the default path
    PathBuf::from("web-client")
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
    
    log::info!("✅ Self-signed TLS certificate generated for HTTPS (WebCodecs secure context)");
    
    Ok((cert_pem, key_pem))
}

impl WebSocketServer {
    pub async fn new(port: u16, _app_handle: AppHandle) -> Result<Self, String> {
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
        
        // Generate self-signed TLS certificate
        let (cert_pem, key_pem) = generate_self_signed_cert()?;
        let tls_config = RustlsConfig::from_pem(cert_pem, key_pem).await
            .map_err(|e| format!("Failed to configure TLS: {}", e))?;
        
        log::info!("🔒 HTTPS/WSS server listening on {}", addr);
        log::info!("   WebCodecs API will be available (secure context)");

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
