//! Peer and session management

use crate::protocol::{ClientCapabilities, ClientRole, PeerInfo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Session timeout duration
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

/// Ping interval
pub const PING_INTERVAL: Duration = Duration::from_secs(5);

/// Peer connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Just connected, not yet registered
    Connecting,
    /// Registered and active
    Active,
    /// Temporarily disconnected (may reconnect)
    Disconnected,
}

/// Peer/client representation
#[derive(Debug)]
pub struct Peer {
    /// Unique client ID
    pub client_id: String,
    /// Session ID for reconnection
    pub session_id: String,
    /// Client role
    pub role: ClientRole,
    /// Current state
    pub state: PeerState,
    /// UDP address
    pub addr: SocketAddr,
    /// Private/local address (for P2P)
    pub private_addr: Option<SocketAddr>,
    /// Room/channel name
    pub room: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Last activity timestamp
    pub last_seen: Instant,
    /// Packet sequence number
    pub sequence: u32,
    /// RTT estimate (microseconds)
    pub rtt_us: u64,
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
}

impl Peer {
    pub fn new(
        client_id: String,
        session_id: String,
        role: ClientRole,
        addr: SocketAddr,
        room: String,
        capabilities: ClientCapabilities,
    ) -> Self {
        Self {
            client_id,
            session_id,
            role,
            state: PeerState::Active,
            addr,
            private_addr: None,
            room,
            capabilities,
            last_seen: Instant::now(),
            sequence: 0,
            rtt_us: 0,
            packets_sent: 0,
            packets_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    /// Check if peer is timed out
    pub fn is_timed_out(&self) -> bool {
        self.last_seen.elapsed() > SESSION_TIMEOUT
    }

    /// Update last seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = Instant::now();
    }

    /// Get next sequence number
    pub fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }

    /// Record packet sent
    pub fn record_sent(&mut self, bytes: usize) {
        self.packets_sent += 1;
        self.bytes_sent += bytes as u64;
    }

    /// Record packet received
    pub fn record_received(&mut self, bytes: usize) {
        self.packets_received += 1;
        self.bytes_received += bytes as u64;
    }

    /// Convert to PeerInfo for protocol messages
    pub fn to_peer_info(&self) -> PeerInfo {
        PeerInfo {
            client_id: self.client_id.clone(),
            role: self.role,
            public_addr: self.addr.to_string(),
            private_addr: self.private_addr.map(|a| a.to_string()),
        }
    }
}

/// Room/channel with connected peers
#[derive(Debug)]
pub struct Room {
    /// Room name
    pub name: String,
    /// Connected peers (client_id -> peer)
    pub peers: HashMap<String, Arc<RwLock<Peer>>>,
    /// Host client ID
    pub host_id: Option<String>,
    /// Creation time
    pub created_at: Instant,
}

impl Room {
    pub fn new(name: String) -> Self {
        Self {
            name,
            peers: HashMap::new(),
            host_id: None,
            created_at: Instant::now(),
        }
    }

    /// Add a peer to the room
    pub fn add_peer(&mut self, peer: Arc<RwLock<Peer>>, is_host: bool) {
        let client_id = {
            // We need to get the client_id synchronously for HashMap key
            // This is called from an async context, so we'll use try_read
            if let Ok(p) = peer.try_read() {
                p.client_id.clone()
            } else {
                return;
            }
        };
        
        if is_host {
            self.host_id = Some(client_id.clone());
        }
        self.peers.insert(client_id, peer);
    }

    /// Remove a peer from the room
    pub fn remove_peer(&mut self, client_id: &str) {
        self.peers.remove(client_id);
        if self.host_id.as_deref() == Some(client_id) {
            self.host_id = None;
        }
    }

    /// Check if room is empty
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Get all peers except the specified one
    pub fn get_other_peers(&self, exclude_id: &str) -> Vec<Arc<RwLock<Peer>>> {
        self.peers
            .iter()
            .filter(|(id, _)| *id != exclude_id)
            .map(|(_, peer)| peer.clone())
            .collect()
    }

    /// Get host peer
    pub fn get_host(&self) -> Option<Arc<RwLock<Peer>>> {
        self.host_id.as_ref().and_then(|id| self.peers.get(id).cloned())
    }

    /// Get all viewers
    pub fn get_viewers(&self) -> Vec<Arc<RwLock<Peer>>> {
        self.peers
            .iter()
            .filter(|(id, _)| self.host_id.as_deref() != Some(*id))
            .map(|(_, peer)| peer.clone())
            .collect()
    }
}

/// Session manager for peer and room management
pub struct SessionManager {
    /// All connected peers (addr -> peer)
    peers_by_addr: RwLock<HashMap<SocketAddr, Arc<RwLock<Peer>>>>,
    /// Peers by client ID
    peers_by_id: RwLock<HashMap<String, Arc<RwLock<Peer>>>>,
    /// Peers by session ID (for reconnection)
    peers_by_session: RwLock<HashMap<String, Arc<RwLock<Peer>>>>,
    /// Rooms
    rooms: RwLock<HashMap<String, Arc<RwLock<Room>>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            peers_by_addr: RwLock::new(HashMap::new()),
            peers_by_id: RwLock::new(HashMap::new()),
            peers_by_session: RwLock::new(HashMap::new()),
            rooms: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new peer
    pub async fn register_peer(&self, peer: Peer) -> Arc<RwLock<Peer>> {
        let addr = peer.addr;
        let client_id = peer.client_id.clone();
        let session_id = peer.session_id.clone();
        let room_name = peer.room.clone();
        let is_host = peer.role == ClientRole::Host;

        let peer = Arc::new(RwLock::new(peer));

        // Add to all indexes
        self.peers_by_addr.write().await.insert(addr, peer.clone());
        self.peers_by_id.write().await.insert(client_id.clone(), peer.clone());
        self.peers_by_session.write().await.insert(session_id, peer.clone());

        // Add to or create room
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .entry(room_name.clone())
            .or_insert_with(|| Arc::new(RwLock::new(Room::new(room_name))));
        room.write().await.add_peer(peer.clone(), is_host);

        peer
    }

    /// Get peer by address
    pub async fn get_peer_by_addr(&self, addr: &SocketAddr) -> Option<Arc<RwLock<Peer>>> {
        self.peers_by_addr.read().await.get(addr).cloned()
    }

    /// Get peer by client ID
    pub async fn get_peer_by_id(&self, client_id: &str) -> Option<Arc<RwLock<Peer>>> {
        self.peers_by_id.read().await.get(client_id).cloned()
    }

    /// Get peer by session ID
    pub async fn get_peer_by_session(&self, session_id: &str) -> Option<Arc<RwLock<Peer>>> {
        self.peers_by_session.read().await.get(session_id).cloned()
    }

    /// Get room
    pub async fn get_room(&self, name: &str) -> Option<Arc<RwLock<Room>>> {
        self.rooms.read().await.get(name).cloned()
    }

    /// Remove peer
    pub async fn remove_peer(&self, addr: &SocketAddr) {
        let peer = {
            let mut peers = self.peers_by_addr.write().await;
            peers.remove(addr)
        };

        if let Some(peer) = peer {
            let p = peer.read().await;
            let client_id = p.client_id.clone();
            let session_id = p.session_id.clone();
            let room_name = p.room.clone();
            drop(p);

            self.peers_by_id.write().await.remove(&client_id);
            self.peers_by_session.write().await.remove(&session_id);

            // Remove from room
            if let Some(room) = self.get_room(&room_name).await {
                let mut room = room.write().await;
                room.remove_peer(&client_id);
                
                // Clean up empty rooms
                if room.is_empty() {
                    drop(room);
                    self.rooms.write().await.remove(&room_name);
                }
            }
        }
    }

    /// Clean up timed out peers
    pub async fn cleanup_timed_out(&self) -> Vec<SocketAddr> {
        let mut timed_out = Vec::new();

        // Find timed out peers
        {
            let peers = self.peers_by_addr.read().await;
            for (addr, peer) in peers.iter() {
                if peer.read().await.is_timed_out() {
                    timed_out.push(*addr);
                }
            }
        }

        // Remove them
        for addr in &timed_out {
            self.remove_peer(addr).await;
        }

        timed_out
    }

    /// Get statistics
    pub async fn get_stats(&self) -> SessionStats {
        let peers = self.peers_by_addr.read().await.len();
        let rooms = self.rooms.read().await.len();

        let mut total_sent = 0u64;
        let mut total_received = 0u64;

        for peer in self.peers_by_addr.read().await.values() {
            let p = peer.read().await;
            total_sent += p.bytes_sent;
            total_received += p.bytes_received;
        }

        SessionStats {
            active_peers: peers,
            active_rooms: rooms,
            total_bytes_sent: total_sent,
            total_bytes_received: total_received,
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub active_peers: usize,
    pub active_rooms: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}
