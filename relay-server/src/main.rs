//! CLEVER KVM Relay Server
//!
//! High-performance UDP relay server for low-latency video streaming.
//! Supports peer-to-peer connection establishment and fallback relay mode.
//!
//! Features:
//! - UDP-based transport for minimal latency
//! - NAT traversal support (STUN-like)
//! - Optional compression (LZ4)
//! - Connection multiplexing
//! - Automatic peer discovery

mod relay;
mod protocol;
mod peer;

use clap::Parser;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;
use std::net::SocketAddr;
use anyhow::Result;

use relay::{RelayServer, RelayConfig};

/// CLEVER KVM UDP Relay Server
#[derive(Parser, Debug)]
#[command(name = "clever-relay")]
#[command(author = "CLEVER Team")]
#[command(version = "1.0.0")]
#[command(about = "High-performance UDP relay server for CLEVER KVM")]
struct Args {
    /// UDP port to listen on
    #[arg(short, long, default_value = "9922")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Enable compression
    #[arg(short, long, default_value = "false")]
    compress: bool,

    /// Maximum clients
    #[arg(short, long, default_value = "100")]
    max_clients: usize,

    /// Session timeout in seconds
    #[arg(short, long, default_value = "60")]
    timeout: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging with tracing
    let log_level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let bind_addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;

    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║          CLEVER KVM UDP Relay Server v1.0.0              ║");
    info!("╠══════════════════════════════════════════════════════════╣");
    info!("║  Bind Address:    {:^38} ║", bind_addr);
    info!("║  Compression:     {:^38} ║", if args.compress { "Enabled (LZ4)" } else { "Disabled" });
    info!("║  Max Clients:     {:^38} ║", args.max_clients);
    info!("║  Session Timeout: {:^38} ║", format!("{}s", args.timeout));
    info!("╚══════════════════════════════════════════════════════════╝");

    // Create config
    let config = RelayConfig {
        bind_addr,
        enable_compression: args.compress,
        max_rooms: 100,
        max_peers_per_room: args.max_clients,
    };

    // Create and run the relay server
    let server = RelayServer::new(config);

    // Run until interrupted
    if let Err(e) = server.run().await {
        error!("Server error: {:?}", e);
        std::process::exit(1);
    }

    Ok(())
}
