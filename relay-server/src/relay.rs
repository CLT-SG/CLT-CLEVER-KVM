//! UDP Relay Server implementation

use crate::peer::{Peer, SessionManager, PING_INTERVAL};
use crate::protocol::{
    PacketFlags, PacketHeader, PacketType, RegisterMessage,
    MAX_PACKET_SIZE, MAX_PAYLOAD_SIZE,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Relay server configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Server bind address
    pub bind_addr: SocketAddr,
    /// Enable LZ4 compression
    pub enable_compression: bool,
    /// Maximum rooms
    pub max_rooms: usize,
    /// Maximum peers per room
    pub max_peers_per_room: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9867".parse().unwrap(),
            enable_compression: true,
            max_rooms: 100,
            max_peers_per_room: 10,
        }
    }
}

/// UDP Relay Server
pub struct RelayServer {
    config: RelayConfig,
    sessions: Arc<SessionManager>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RelayServer {
    /// Create a new relay server
    pub fn new(config: RelayConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config,
            sessions: Arc::new(SessionManager::new()),
            shutdown_tx,
        }
    }

    /// Run the relay server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let socket = UdpSocket::bind(&self.config.bind_addr).await?;
        
        // Set socket options for performance
        let std_socket = socket.into_std()?;
        std_socket.set_nonblocking(true)?;
        
        // Use socket2 to set buffer sizes
        let sock2 = socket2::Socket::from(std_socket);
        if let Err(e) = sock2.set_recv_buffer_size(1024 * 1024) {
            warn!("Failed to set recv buffer size: {}", e);
        }
        if let Err(e) = sock2.set_send_buffer_size(1024 * 1024) {
            warn!("Failed to set send buffer size: {}", e);
        }
        
        let std_socket: std::net::UdpSocket = sock2.into();
        let socket = UdpSocket::from_std(std_socket)?;
        let socket = Arc::new(socket);

        info!("Relay server listening on {}", self.config.bind_addr);

        // Spawn cleanup task
        let sessions_cleanup = self.sessions.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(PING_INTERVAL);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let timed_out = sessions_cleanup.cleanup_timed_out().await;
                        if !timed_out.is_empty() {
                            info!("Cleaned up {} timed out peers", timed_out.len());
                        }
                        
                        let stats = sessions_cleanup.get_stats().await;
                        debug!(
                            "Stats: {} peers, {} rooms, sent: {} bytes, recv: {} bytes",
                            stats.active_peers,
                            stats.active_rooms,
                            stats.total_bytes_sent,
                            stats.total_bytes_received
                        );
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        // Main receive loop
        let mut buf = vec![0u8; MAX_PACKET_SIZE];
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, addr)) => {
                            if let Err(e) = self.handle_packet(&socket, &buf[..len], addr).await {
                                debug!("Error handling packet from {}: {}", addr, e);
                            }
                        }
                        Err(e) => {
                            error!("Socket receive error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Relay server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle incoming packet
    async fn handle_packet(
        &self,
        socket: &Arc<UdpSocket>,
        data: &[u8],
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse header
        let header = match PacketHeader::decode(data) {
            Some(h) => h,
            None => {
                debug!("Invalid packet header from {}", addr);
                return Ok(());
            }
        };

        let payload = &data[PacketHeader::SIZE..];

        match header.packet_type {
            PacketType::Register => {
                self.handle_register(socket, addr, payload).await?;
            }
            PacketType::Ping => {
                self.handle_ping(socket, addr, header.sequence).await?;
            }
            PacketType::VideoFrame => {
                self.handle_video_frame(socket, addr, &header, payload).await?;
            }
            PacketType::AudioFrame => {
                self.handle_audio_frame(socket, addr, &header, payload).await?;
            }
            PacketType::InputEvent => {
                self.handle_input_event(socket, addr, payload).await?;
            }
            PacketType::Disconnect => {
                self.handle_disconnect(addr).await?;
            }
            PacketType::DiscoverPeers => {
                self.handle_discover_peers(socket, addr).await?;
            }
            _ => {
                debug!("Unhandled packet type {:?} from {}", header.packet_type, addr);
            }
        }

        Ok(())
    }

    /// Handle registration
    async fn handle_register(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse registration message (JSON)
        let msg: RegisterMessage = serde_json::from_slice(payload)?;
        
        info!(
            "Registering client {} as {:?} in room '{}'",
            msg.client_id, msg.role, msg.room
        );

        // Create session ID if not provided
        let session_id = msg.session_id.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            format!("{}_{}", msg.client_id, ts)
        });

        // Create peer
        let peer = Peer::new(
            msg.client_id.clone(),
            session_id.clone(),
            msg.role,
            addr,
            msg.room,
            msg.capabilities,
        );

        let peer = self.sessions.register_peer(peer).await;

        // Send acknowledgment
        let response = serde_json::json!({
            "success": true,
            "session_id": session_id,
            "server_time": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        });
        let response_data = serde_json::to_vec(&response)?;

        let header = PacketHeader::new(PacketType::RegisterAck, peer.read().await.sequence);
        let mut packet = Vec::with_capacity(PacketHeader::SIZE + response_data.len());
        packet.extend_from_slice(&header.encode());
        packet.extend_from_slice(&response_data);

        socket.send_to(&packet, addr).await?;

        Ok(())
    }

    /// Handle ping
    async fn handle_ping(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
        sequence: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update peer last_seen
        if let Some(peer) = self.sessions.get_peer_by_addr(&addr).await {
            peer.write().await.touch();
        }

        // Send pong
        let header = PacketHeader::new(PacketType::Pong, sequence);
        socket.send_to(&header.encode(), addr).await?;

        Ok(())
    }

    /// Handle video frame - relay to viewers
    async fn handle_video_frame(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
        header: &PacketHeader,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get sender peer
        let peer = match self.sessions.get_peer_by_addr(&addr).await {
            Some(p) => p,
            None => {
                debug!("Video frame from unknown peer {}", addr);
                return Ok(());
            }
        };

        let room_name = {
            let mut p = peer.write().await;
            p.touch();
            p.record_received(payload.len() + PacketHeader::SIZE);
            p.room.clone()
        };

        // Get room and relay to all viewers
        if let Some(room) = self.sessions.get_room(&room_name).await {
            let viewers = room.read().await.get_viewers();

            // Reconstruct full packet
            let mut packet = Vec::with_capacity(PacketHeader::SIZE + payload.len());
            packet.extend_from_slice(&header.encode());
            packet.extend_from_slice(payload);

            // Send to all viewers in parallel
            for viewer in viewers {
                let viewer_addr = viewer.read().await.addr;
                if viewer_addr != addr {
                    if let Err(e) = socket.send_to(&packet, viewer_addr).await {
                        debug!("Failed to relay video to {}: {}", viewer_addr, e);
                    } else {
                        viewer.write().await.record_sent(packet.len());
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle audio frame - relay to viewers
    async fn handle_audio_frame(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
        header: &PacketHeader,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Similar to video frame
        let peer = match self.sessions.get_peer_by_addr(&addr).await {
            Some(p) => p,
            None => return Ok(()),
        };

        let room_name = {
            let mut p = peer.write().await;
            p.touch();
            p.record_received(payload.len() + PacketHeader::SIZE);
            p.room.clone()
        };

        if let Some(room) = self.sessions.get_room(&room_name).await {
            let viewers = room.read().await.get_viewers();

            let mut packet = Vec::with_capacity(PacketHeader::SIZE + payload.len());
            packet.extend_from_slice(&header.encode());
            packet.extend_from_slice(payload);

            for viewer in viewers {
                let viewer_addr = viewer.read().await.addr;
                if viewer_addr != addr {
                    let _ = socket.send_to(&packet, viewer_addr).await;
                }
            }
        }

        Ok(())
    }

    /// Handle input event - relay to host
    async fn handle_input_event(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get sender (viewer)
        let peer = match self.sessions.get_peer_by_addr(&addr).await {
            Some(p) => p,
            None => return Ok(()),
        };

        let room_name = {
            let mut p = peer.write().await;
            p.touch();
            p.room.clone()
        };

        // Get host and relay input
        if let Some(room) = self.sessions.get_room(&room_name).await {
            if let Some(host) = room.read().await.get_host() {
                let host_addr = host.read().await.addr;
                
                // Forward the full packet to host
                let header = PacketHeader::new(PacketType::InputEvent, 0);
                let mut packet = Vec::with_capacity(PacketHeader::SIZE + payload.len());
                packet.extend_from_slice(&header.encode());
                packet.extend_from_slice(payload);
                
                let _ = socket.send_to(&packet, host_addr).await;
            }
        }

        Ok(())
    }

    /// Handle disconnect
    async fn handle_disconnect(
        &self,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Client disconnected: {}", addr);
        self.sessions.remove_peer(&addr).await;
        Ok(())
    }

    /// Handle peer discovery
    async fn handle_discover_peers(
        &self,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let peer = match self.sessions.get_peer_by_addr(&addr).await {
            Some(p) => p,
            None => return Ok(()),
        };

        let room_name = peer.read().await.room.clone();
        
        if let Some(room) = self.sessions.get_room(&room_name).await {
            let room = room.read().await;
            let peers: Vec<_> = room
                .peers
                .values()
                .filter_map(|p| {
                    if let Ok(peer) = p.try_read() {
                        if peer.addr != addr {
                            Some(peer.to_peer_info())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            let response = serde_json::to_vec(&peers)?;
            let header = PacketHeader::new(PacketType::PeerList, 0);
            let mut packet = Vec::with_capacity(PacketHeader::SIZE + response.len());
            packet.extend_from_slice(&header.encode());
            packet.extend_from_slice(&response);

            socket.send_to(&packet, addr).await?;
        }

        Ok(())
    }

    /// Shutdown the server
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Fragment a large payload into multiple packets
pub fn fragment_payload(
    data: &[u8],
    packet_type: PacketType,
    base_sequence: u32,
) -> Vec<Vec<u8>> {
    let mut fragments = Vec::new();
    let chunks: Vec<_> = data.chunks(MAX_PAYLOAD_SIZE).collect();
    let fragment_count = chunks.len() as u16;

    for (i, chunk) in chunks.into_iter().enumerate() {
        let mut flags = PacketFlags::FRAGMENTED;
        if i == fragment_count as usize - 1 {
            flags |= PacketFlags::LAST_FRAGMENT;
        }

        let header = PacketHeader::new(packet_type, base_sequence + i as u32)
            .with_flags(flags);

        // Add fragment header (4 bytes: index u16 + count u16)
        let mut packet = Vec::with_capacity(PacketHeader::SIZE + 4 + chunk.len());
        packet.extend_from_slice(&header.encode());
        packet.extend_from_slice(&(i as u16).to_le_bytes());
        packet.extend_from_slice(&fragment_count.to_le_bytes());
        packet.extend_from_slice(chunk);

        fragments.push(packet);
    }

    fragments
}

/// Compress data with LZ4
pub fn compress_lz4(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

/// Decompress LZ4 data
pub fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, lz4_flex::block::DecompressError> {
    lz4_flex::decompress_size_prepended(data)
}
