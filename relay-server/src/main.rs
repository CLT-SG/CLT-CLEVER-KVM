//! CLEVER KVM Relay Server v2.0
//!
//! A high-performance relay server for CLEVER KVM devices with:
//! - Web dashboard for device management
//! - mDNS service discovery for auto-discovery by Tauri apps
//! - WebSocket relay for video streaming and input events
//! - REST API for device registration and status
//!
//! Features:
//! - HTTP server with web dashboard at port 8881
//! - mDNS publishing for .local hostname discovery
//! - Device registry with heartbeat monitoring
//! - WebSocket relay for video/input streams
//! - UDP relay for low-latency streaming (optional)

mod device;
mod discovery;
mod http_server;
mod peer;
mod protocol;
mod relay;
mod ws_relay;

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use device::DeviceRegistry;
use discovery::MdnsDiscovery;
use http_server::{run_server, AppState};
use ws_relay::WsRelayState;

/// CLEVER KVM Relay Server v2.0
#[derive(Parser, Debug)]
#[command(name = "clever-relay")]
#[command(author = "CLEVER Team")]
#[command(version = "2.0.0")]
#[command(about = "CLEVER KVM Relay Server with web dashboard and device discovery")]
struct Args {
    /// HTTP/WebSocket port to listen on
    #[arg(short, long, default_value = "8881")]
    port: u16,

    /// UDP relay port (optional, for low-latency streaming)
    #[arg(long, default_value = "9922")]
    udp_port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Enable mDNS service discovery
    #[arg(long, default_value = "true")]
    mdns: bool,

    /// Enable UDP relay (in addition to WebSocket)
    #[arg(long, default_value = "false")]
    enable_udp: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Disable dashboard (API only)
    #[arg(long)]
    no_dashboard: bool,
}

#[actix_rt::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Get hostname for display
    let hostname = discovery::get_hostname();

    // Print banner
    print_banner(&args, &hostname);

    // Initialize components
    let registry = Arc::new(DeviceRegistry::new());
    let ws_relay = Arc::new(WsRelayState::new());

    // Start mDNS discovery service
    let _mdns = if args.mdns {
        match MdnsDiscovery::new(args.port) {
            Ok(mut mdns) => {
                if let Err(e) = mdns.publish() {
                    warn!("Failed to publish mDNS service: {}", e);
                    None
                } else {
                    Some(mdns)
                }
            }
            Err(e) => {
                warn!("Failed to initialize mDNS: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create application state
    let app_state = AppState {
        registry: registry.clone(),
        ws_relay: ws_relay.clone(),
        server_hostname: hostname.clone(),
        server_port: args.port,
    };

    // Spawn cleanup task for stale devices
    let cleanup_registry = registry.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let removed = cleanup_registry.cleanup_stale_devices().await;
            if !removed.is_empty() {
                info!(
                    "🧹 Cleaned up {} stale devices: {:?}",
                    removed.len(),
                    removed
                );
            }
        }
    });

    // Optionally start UDP relay
    if args.enable_udp {
        let udp_bind_addr: SocketAddr = format!("{}:{}", args.bind, args.udp_port).parse()?;
        let udp_config = relay::RelayConfig {
            bind_addr: udp_bind_addr,
            enable_compression: false,
            max_rooms: 100,
            max_peers_per_room: 10,
        };

        let udp_relay = relay::RelayServer::new(udp_config);

        tokio::spawn(async move {
            info!("🔌 UDP relay listening on {}", udp_bind_addr);
            if let Err(e) = udp_relay.run().await {
                error!("UDP relay error: {}", e);
            }
        });
    }

    // Get local IP addresses for display
    let addresses = discovery::get_local_addresses();
    info!(
        "🚀 HTTP server listening on http://{}:{}/",
        hostname, args.port
    );
    info!("   Dashboard: http://{}.local:{}/", hostname, args.port);
    for addr in &addresses {
        info!("   Also accessible at: http://{}:{}/", addr, args.port);
    }

    // Run HTTP server (blocking)
    let bind_addr = format!("{}:{}", args.bind, args.port);
    run_server(app_state, &bind_addr).await?;

    Ok(())
}

fn print_banner(args: &Args, hostname: &str) {
    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║       CLEVER KVM Relay Server v2.0.0                     ║");
    info!("╠══════════════════════════════════════════════════════════╣");
    info!(
        "║  Hostname:        {:<38} ║",
        format!("{}.local", hostname)
    );
    info!("║  HTTP Port:       {:<38} ║", args.port);
    if args.enable_udp {
        info!("║  UDP Port:        {:<38} ║", args.udp_port);
    }
    info!(
        "║  mDNS Discovery:  {:<38} ║",
        if args.mdns { "Enabled" } else { "Disabled" }
    );
    info!(
        "║  Dashboard:       {:<38} ║",
        if args.no_dashboard {
            "Disabled"
        } else {
            "Enabled"
        }
    );
    info!("╠══════════════════════════════════════════════════════════╣");
    info!(
        "║  Dashboard URL:   http://{}.local:{:<17} ║",
        hostname,
        format!("{}/", args.port)
    );
    info!("║  KVM Client:      /kvm?hostname=<device>                 ║");
    info!("║  API Endpoint:    /api/devices                          ║");
    info!("╚══════════════════════════════════════════════════════════╝");
}
