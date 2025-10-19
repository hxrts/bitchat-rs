//! NetworkRouter - Centralized packet routing for the BitChat simulator
//!
//! This module implements the formal "Network Router" role, acting as the network itself
//! to route packets between peers with configurable network conditions.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use rand::{Rng, SeedableRng};
use bitchat_core::PeerId;

// ----------------------------------------------------------------------------
// Network Conditions Configuration
// ----------------------------------------------------------------------------

/// Network condition profiles for realistic simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkProfile {
    /// Perfect network conditions - no latency, loss, or reordering
    Perfect,
    /// Slow WiFi with typical home broadband characteristics
    SlowWifi {
        latency_ms: u32,
        jitter_ms: u32,
    },
    /// Unreliable 3G mobile connection
    Unreliable3G {
        packet_loss: f32,
        reordering_chance: f32,
        latency_ms: u32,
        jitter_ms: u32,
    },
    /// High latency satellite connection
    Satellite {
        latency_ms: u32,
        jitter_ms: u32,
        occasional_outages: bool,
    },
    /// Mesh network with peer-to-peer routing
    MeshNetwork {
        hop_latency_ms: u32,
        max_hops: u8,
        partition_chance: f32,
    },
    /// Custom network conditions
    Custom {
        latency_range_ms: (u32, u32),
        packet_loss: f32,
        reordering_chance: f32,
        duplication_chance: f32,
        corruption_chance: f32,
    },
}

impl Default for NetworkProfile {
    fn default() -> Self {
        NetworkProfile::Perfect
    }
}

impl NetworkProfile {
    /// Get typical latency for this profile
    #[allow(dead_code)]
    pub fn base_latency_ms(&self) -> u32 {
        match self {
            NetworkProfile::Perfect => 0,
            NetworkProfile::SlowWifi { latency_ms, .. } => *latency_ms,
            NetworkProfile::Unreliable3G { latency_ms, .. } => *latency_ms,
            NetworkProfile::Satellite { latency_ms, .. } => *latency_ms,
            NetworkProfile::MeshNetwork { hop_latency_ms, max_hops, .. } => 
                hop_latency_ms * (*max_hops as u32),
            NetworkProfile::Custom { latency_range_ms, .. } => latency_range_ms.0,
        }
    }
}

/// Configuration for the network router
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NetworkRouterConfig {
    /// Default network profile
    pub profile: NetworkProfile,
    /// Whether to enable detailed packet logging
    pub enable_packet_logging: bool,
    /// Maximum packet queue size per peer
    pub max_queue_size: usize,
    /// Network topology (for future mesh simulation)
    pub topology: NetworkTopology,
}

impl Default for NetworkRouterConfig {
    fn default() -> Self {
        Self {
            profile: NetworkProfile::Perfect,
            enable_packet_logging: false,
            max_queue_size: 1000,
            topology: NetworkTopology::FullyConnected,
        }
    }
}

/// Network topology configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NetworkTopology {
    /// All peers can directly communicate
    FullyConnected,
    /// Linear chain of peers
    Linear,
    /// Star topology with one central peer
    Star { center: PeerId },
    /// Custom topology defined by adjacency list
    Custom { adjacency: HashMap<PeerId, Vec<PeerId>> },
}

// ----------------------------------------------------------------------------
// Packet Handling
// ----------------------------------------------------------------------------

/// A packet being routed through the network
#[derive(Debug, Clone)]
pub struct NetworkPacket {
    /// Source peer ID
    pub from: PeerId,
    /// Destination peer ID  
    pub to: PeerId,
    /// Packet payload
    pub data: Vec<u8>,
    /// Original timestamp when packet was sent
    pub sent_at: Instant,
    /// When this packet should be delivered (after applying latency)
    pub deliver_at: Instant,
    /// Unique sequence number for tracking
    pub sequence: u64,
    /// Number of hops (for mesh routing)
    pub hop_count: u8,
}

/// Handle for a mock transport to interact with the network router
#[allow(dead_code)]
pub struct MockNetworkHandle {
    /// Channel to send outgoing packets to the router
    pub outgoing_tx: mpsc::UnboundedSender<NetworkPacket>,
    /// Channel to receive incoming packets from the router
    pub incoming_rx: mpsc::UnboundedReceiver<NetworkPacket>,
    /// Peer ID associated with this handle
    pub peer_id: PeerId,
}

impl MockNetworkHandle {
    /// Send a packet to another peer through the network
    #[allow(dead_code)]
    pub async fn send_packet(&self, to: PeerId, data: Vec<u8>) -> Result<(), String> {
        let packet = NetworkPacket {
            from: self.peer_id,
            to,
            data,
            sent_at: Instant::now(),
            deliver_at: Instant::now(), // Router will update this
            sequence: 0, // Router will assign this
            hop_count: 0,
        };

        self.outgoing_tx.send(packet)
            .map_err(|_| "Network router disconnected".to_string())
    }

    /// Receive the next packet addressed to this peer
    #[allow(dead_code)]
    pub async fn receive_packet(&mut self) -> Option<NetworkPacket> {
        self.incoming_rx.recv().await
    }

    /// Try to receive a packet without blocking
    #[allow(dead_code)]
    pub fn try_receive_packet(&mut self) -> Option<NetworkPacket> {
        self.incoming_rx.try_recv().ok()
    }
}

// ----------------------------------------------------------------------------
// Network Router Implementation
// ----------------------------------------------------------------------------

/// Central network router that simulates network behavior
pub struct NetworkRouter {
    /// Configuration
    config: NetworkRouterConfig,
    /// Packet sequence counter
    next_sequence: u64,
    /// Pending packets waiting to be delivered
    pending_packets: Vec<NetworkPacket>,
    /// Channels for each peer
    peer_channels: HashMap<PeerId, mpsc::UnboundedSender<NetworkPacket>>,
    /// Channel to receive outgoing packets from all peers
    outgoing_rx: mpsc::UnboundedReceiver<NetworkPacket>,
    /// Statistics
    stats: NetworkStats,
    /// Random number generator for network effects
    rng: rand::rngs::StdRng,
}

/// Network statistics
#[derive(Debug, Default, Clone)]
pub struct NetworkStats {
    pub packets_routed: u64,
    pub packets_dropped: u64,
    pub packets_reordered: u64,
    pub packets_duplicated: u64,
    pub total_latency_ms: u64,
    pub peak_queue_size: usize,
}

impl NetworkRouter {
    /// Create a new network router
    pub fn new(config: NetworkRouterConfig) -> (Self, mpsc::UnboundedSender<NetworkPacket>) {
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
        
        let router = Self {
            config,
            next_sequence: 1,
            pending_packets: Vec::new(),
            peer_channels: HashMap::new(),
            outgoing_rx,
            stats: NetworkStats::default(),
            rng: rand::rngs::StdRng::from_entropy(),
        };

        (router, outgoing_tx)
    }

    /// Register a new peer with the network router
    #[allow(dead_code)]
    pub fn add_peer(&mut self, peer_id: PeerId) -> MockNetworkHandle {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let (outgoing_tx, _outgoing_rx) = mpsc::unbounded_channel();

        self.peer_channels.insert(peer_id, incoming_tx);

        // We need to forward packets from this peer's outgoing channel to the main router
        // This would typically be done by spawning a task, but for now we'll use a simpler approach
        
        MockNetworkHandle {
            outgoing_tx,
            incoming_rx,
            peer_id,
        }
    }

    /// Remove a peer from the network
    #[allow(dead_code)]
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peer_channels.remove(peer_id);
        
        // Remove any pending packets for this peer
        self.pending_packets.retain(|packet| packet.to != *peer_id && packet.from != *peer_id);
        
        info!("Removed peer {} from network router", peer_id);
    }

    /// Start the network router main loop
    pub async fn run(&mut self) -> Result<(), String> {
        info!("Starting network router with profile: {:?}", self.config.profile);
        
        let mut delivery_interval = interval(Duration::from_millis(1));
        let mut stats_interval = interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                // Process incoming packets from peers
                packet = self.outgoing_rx.recv() => {
                    match packet {
                        Some(mut packet) => {
                            packet.sequence = self.next_sequence;
                            self.next_sequence += 1;
                            
                            if let Err(e) = self.process_outgoing_packet(packet).await {
                                error!("Failed to process outgoing packet: {}", e);
                            }
                        }
                        None => {
                            info!("All peer channels closed, shutting down network router");
                            break;
                        }
                    }
                }

                // Check for packets ready to be delivered
                _ = delivery_interval.tick() => {
                    self.deliver_ready_packets().await;
                }

                // Log statistics periodically
                _ = stats_interval.tick() => {
                    self.log_statistics();
                }
            }
        }

        info!("Network router stopped. Final stats: {:?}", self.stats);
        Ok(())
    }

    /// Process an outgoing packet from a peer
    async fn process_outgoing_packet(&mut self, mut packet: NetworkPacket) -> Result<(), String> {
        if self.config.enable_packet_logging {
            debug!("Processing packet {} from {} to {}, {} bytes", 
                   packet.sequence, packet.from, packet.to, packet.data.len());
        }

        // Check if destination peer exists
        if !self.peer_channels.contains_key(&packet.to) {
            warn!("Dropping packet to unknown peer: {}", packet.to);
            self.stats.packets_dropped += 1;
            return Ok(());
        }

        // Apply network conditions based on profile
        match &self.config.profile {
            NetworkProfile::Perfect => {
                // No delay, deliver immediately
                packet.deliver_at = Instant::now();
            }
            
            NetworkProfile::SlowWifi { latency_ms, jitter_ms } => {
                let jitter = self.rng.gen_range(0..=*jitter_ms);
                let total_latency = latency_ms + jitter;
                packet.deliver_at = Instant::now() + Duration::from_millis(total_latency as u64);
            }
            
            NetworkProfile::Unreliable3G { packet_loss, reordering_chance, latency_ms, jitter_ms } => {
                // Check for packet loss
                if self.rng.gen::<f32>() < *packet_loss {
                    if self.config.enable_packet_logging {
                        debug!("Dropping packet {} due to packet loss", packet.sequence);
                    }
                    self.stats.packets_dropped += 1;
                    return Ok(());
                }

                // Apply latency with jitter
                let jitter = self.rng.gen_range(0..=*jitter_ms);
                let total_latency = latency_ms + jitter;
                packet.deliver_at = Instant::now() + Duration::from_millis(total_latency as u64);

                // Check for reordering
                if self.rng.gen::<f32>() < *reordering_chance {
                    // Add extra delay for reordering
                    let extra_delay = self.rng.gen_range(10..100);
                    packet.deliver_at += Duration::from_millis(extra_delay);
                    self.stats.packets_reordered += 1;
                }
            }

            NetworkProfile::Satellite { latency_ms, jitter_ms, occasional_outages } => {
                // Check for outages
                if *occasional_outages && self.rng.gen::<f32>() < 0.01 { // 1% chance of outage
                    if self.config.enable_packet_logging {
                        debug!("Dropping packet {} due to satellite outage", packet.sequence);
                    }
                    self.stats.packets_dropped += 1;
                    return Ok(());
                }

                let jitter = self.rng.gen_range(0..=*jitter_ms);
                let total_latency = latency_ms + jitter;
                packet.deliver_at = Instant::now() + Duration::from_millis(total_latency as u64);
            }

            NetworkProfile::MeshNetwork { hop_latency_ms, max_hops, partition_chance } => {
                // Simulate mesh routing
                packet.hop_count += 1;
                
                if packet.hop_count > *max_hops {
                    warn!("Dropping packet {} - exceeded max hops", packet.sequence);
                    self.stats.packets_dropped += 1;
                    return Ok(());
                }

                // Check for network partition
                if self.rng.gen::<f32>() < *partition_chance {
                    warn!("Dropping packet {} due to network partition", packet.sequence);
                    self.stats.packets_dropped += 1;
                    return Ok(());
                }

                let latency = hop_latency_ms * (packet.hop_count as u32);
                packet.deliver_at = Instant::now() + Duration::from_millis(latency as u64);
            }

            NetworkProfile::Custom { 
                latency_range_ms, 
                packet_loss, 
                reordering_chance,
                duplication_chance,
                corruption_chance 
            } => {
                // Packet loss
                if self.rng.gen::<f32>() < *packet_loss {
                    self.stats.packets_dropped += 1;
                    return Ok(());
                }

                // Packet corruption (simulate with invalid data)
                if self.rng.gen::<f32>() < *corruption_chance && !packet.data.is_empty() {
                    let corrupt_idx = self.rng.gen_range(0..packet.data.len());
                    packet.data[corrupt_idx] = self.rng.gen();
                }

                // Duplication
                if self.rng.gen::<f32>() < *duplication_chance {
                    let mut duplicate = packet.clone();
                    duplicate.sequence = self.next_sequence;
                    self.next_sequence += 1;
                    self.pending_packets.push(duplicate);
                    self.stats.packets_duplicated += 1;
                }

                // Latency with range
                let latency = self.rng.gen_range(latency_range_ms.0..=latency_range_ms.1);
                packet.deliver_at = Instant::now() + Duration::from_millis(latency as u64);

                // Reordering
                if self.rng.gen::<f32>() < *reordering_chance {
                    let extra_delay = self.rng.gen_range(10..100);
                    packet.deliver_at += Duration::from_millis(extra_delay);
                    self.stats.packets_reordered += 1;
                }
            }
        }

        // Add to pending packets
        self.pending_packets.push(packet);
        self.stats.packets_routed += 1;

        // Update peak queue size
        if self.pending_packets.len() > self.stats.peak_queue_size {
            self.stats.peak_queue_size = self.pending_packets.len();
        }

        Ok(())
    }

    /// Deliver packets that are ready
    async fn deliver_ready_packets(&mut self) {
        let now = Instant::now();
        let mut delivered_indices = Vec::new();

        for (i, packet) in self.pending_packets.iter().enumerate() {
            if packet.deliver_at <= now {
                if let Some(channel) = self.peer_channels.get(&packet.to) {
                    if channel.send(packet.clone()).is_err() {
                        warn!("Failed to deliver packet to peer {} - channel closed", packet.to);
                        // Remove the peer if channel is closed
                        self.peer_channels.remove(&packet.to);
                    } else {
                        if self.config.enable_packet_logging {
                            debug!("Delivered packet {} to peer {}", packet.sequence, packet.to);
                        }
                        
                        // Update latency stats
                        let latency = now.duration_since(packet.sent_at).as_millis() as u64;
                        self.stats.total_latency_ms += latency;
                    }
                } else {
                    warn!("Attempted to deliver packet to unknown peer: {}", packet.to);
                }
                delivered_indices.push(i);
            }
        }

        // Remove delivered packets (in reverse order to maintain indices)
        for &i in delivered_indices.iter().rev() {
            self.pending_packets.remove(i);
        }
    }

    /// Log network statistics
    fn log_statistics(&self) {
        let avg_latency = if self.stats.packets_routed > 0 {
            self.stats.total_latency_ms / self.stats.packets_routed
        } else {
            0
        };

        info!(
            "Network stats: routed={}, dropped={}, reordered={}, duplicated={}, avg_latency={}ms, peak_queue={}",
            self.stats.packets_routed,
            self.stats.packets_dropped,
            self.stats.packets_reordered,
            self.stats.packets_duplicated,
            avg_latency,
            self.stats.peak_queue_size
        );
    }

    /// Get current network statistics
    #[allow(dead_code)]
    pub fn get_stats(&self) -> &NetworkStats {
        &self.stats
    }

    /// Update network profile dynamically
    pub fn set_profile(&mut self, profile: NetworkProfile) {
        info!("Changing network profile to: {:?}", profile);
        self.config.profile = profile;
    }

    /// Simulate network partition between specific peers
    pub fn partition_peers(&mut self, peer1: PeerId, peer2: PeerId) {
        warn!("Simulating network partition between {} and {}", peer1, peer2);
        
        // Remove any pending packets between these peers
        self.pending_packets.retain(|packet| {
            !((packet.from == peer1 && packet.to == peer2) || 
              (packet.from == peer2 && packet.to == peer1))
        });
        
        // In a full implementation, we would track partitioned peers
        // and prevent future packet delivery between them
    }

    /// Heal network partition between specific peers  
    pub fn heal_partition(&mut self, peer1: PeerId, peer2: PeerId) {
        info!("Healing network partition between {} and {}", peer1, peer2);
        // In a full implementation, we would restore connectivity
    }
}

// ----------------------------------------------------------------------------
// Presets for Common Network Conditions
// ----------------------------------------------------------------------------

impl NetworkProfile {
    /// Perfect LAN conditions
    #[allow(dead_code)]
    pub fn perfect() -> Self {
        NetworkProfile::Perfect
    }

    /// Typical home WiFi
    #[allow(dead_code)]
    pub fn home_wifi() -> Self {
        NetworkProfile::SlowWifi {
            latency_ms: 5,
            jitter_ms: 2,
        }
    }

    /// Coffee shop WiFi
    #[allow(dead_code)]
    pub fn coffee_wifi() -> Self {
        NetworkProfile::SlowWifi {
            latency_ms: 20,
            jitter_ms: 10,
        }
    }

    /// Mobile 4G connection
    #[allow(dead_code)]
    pub fn mobile_4g() -> Self {
        NetworkProfile::Unreliable3G {
            packet_loss: 0.001, // 0.1%
            reordering_chance: 0.01, // 1%
            latency_ms: 50,
            jitter_ms: 20,
        }
    }

    /// Poor mobile connection
    #[allow(dead_code)]
    pub fn poor_mobile() -> Self {
        NetworkProfile::Unreliable3G {
            packet_loss: 0.05, // 5%
            reordering_chance: 0.1, // 10%
            latency_ms: 200,
            jitter_ms: 100,
        }
    }

    /// Satellite internet
    #[allow(dead_code)]
    pub fn satellite() -> Self {
        NetworkProfile::Satellite {
            latency_ms: 600,
            jitter_ms: 50,
            occasional_outages: true,
        }
    }

    /// Mesh network simulation
    #[allow(dead_code)]
    pub fn mesh_network() -> Self {
        NetworkProfile::MeshNetwork {
            hop_latency_ms: 10,
            max_hops: 5,
            partition_chance: 0.001, // 0.1%
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_perfect_network() {
        let config = NetworkRouterConfig {
            profile: NetworkProfile::perfect(),
            enable_packet_logging: true,
            ..Default::default()
        };

        let (mut router, outgoing_tx) = NetworkRouter::new(config);
        
        let peer1 = PeerId::new([1; 8]);
        let peer2 = PeerId::new([2; 8]);
        
        let mut handle1 = router.add_peer(peer1);
        let mut handle2 = router.add_peer(peer2);

        // Start router in background
        let router_handle = tokio::spawn(async move {
            router.run().await
        });

        // Send packet from peer1 to peer2
        handle1.send_packet(peer2, b"hello".to_vec()).await.unwrap();

        // Peer2 should receive it immediately
        let received = tokio::time::timeout(Duration::from_millis(100), handle2.receive_packet()).await;
        assert!(received.is_ok());
        
        let packet = received.unwrap().unwrap();
        assert_eq!(packet.from, peer1);
        assert_eq!(packet.data, b"hello");

        // Clean shutdown
        drop(outgoing_tx);
        drop(handle1);
        drop(handle2);
        
        let _ = router_handle.await;
    }

    #[tokio::test] 
    async fn test_lossy_network() {
        let config = NetworkRouterConfig {
            profile: NetworkProfile::Unreliable3G {
                packet_loss: 0.5, // 50% loss for testing
                reordering_chance: 0.0,
                latency_ms: 10,
                jitter_ms: 5,
            },
            enable_packet_logging: true,
            ..Default::default()
        };

        let (mut router, _outgoing_tx) = NetworkRouter::new(config);
        
        let peer1 = PeerId::new([1; 8]);
        let peer2 = PeerId::new([2; 8]);
        
        let mut handle1 = router.add_peer(peer1);
        let mut _handle2 = router.add_peer(peer2);

        // Start router in background  
        let router_handle = tokio::spawn(async move {
            router.run().await
        });

        // Send many packets, expect some to be lost
        for i in 0..100 {
            let _ = handle1.send_packet(peer2, format!("packet{}", i).into_bytes()).await;
        }

        // Give time for processing
        sleep(Duration::from_millis(100)).await;

        // Check router stats
        // Note: In a real test we'd need access to router stats
        
        drop(handle1);
        let _ = router_handle.await;
    }
}