//! Transport abstraction for BitChat protocol
//!
//! This module provides a unified interface for different transport mechanisms
//! (BLE, Nostr, etc.) used by the BitChat protocol, enabling clean separation
//! between protocol logic and transport implementation.

use alloc::{boxed::Box, string::String, vec::Vec};
use smallvec::SmallVec;
use async_trait::async_trait;

use crate::packet::BitchatPacket;
use crate::types::PeerId;
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Transport Trait
// ----------------------------------------------------------------------------

/// Unified transport interface for BitChat communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a packet to a specific peer
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> Result<()>;

    /// Broadcast a packet to all reachable peers
    async fn broadcast(&mut self, packet: BitchatPacket) -> Result<()>;

    /// Receive the next packet from any peer
    async fn receive(&mut self) -> Result<(PeerId, BitchatPacket)>;

    /// Get list of currently discoverable peers (optimized for small collections)
    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]>;

    /// Start the transport (begin scanning, advertising, etc.)
    async fn start(&mut self) -> Result<()>;

    /// Stop the transport and clean up resources
    async fn stop(&mut self) -> Result<()>;

    /// Check if transport is currently active
    fn is_active(&self) -> bool;

    /// Get transport-specific metadata/capabilities
    fn capabilities(&self) -> TransportCapabilities;
}

// ----------------------------------------------------------------------------
// Transport Capabilities
// ----------------------------------------------------------------------------

/// Describes the capabilities and characteristics of a transport
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    /// Transport type identifier
    pub transport_type: TransportType,
    /// Maximum packet size supported
    pub max_packet_size: usize,
    /// Whether transport supports peer discovery
    pub supports_discovery: bool,
    /// Whether transport supports broadcasting
    pub supports_broadcast: bool,
    /// Whether transport requires internet connectivity
    pub requires_internet: bool,
    /// Typical latency characteristics
    pub latency_class: LatencyClass,
    /// Reliability characteristics
    pub reliability_class: ReliabilityClass,
}

/// Transport type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransportType {
    /// Bluetooth Low Energy
    Ble,
    /// Nostr over WebSocket
    Nostr,
    /// Local network (for testing)
    Local,
    /// Custom transport implementation
    Custom(&'static str),
}

/// Latency characteristics of a transport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyClass {
    /// Very low latency (< 10ms typical)
    VeryLow,
    /// Low latency (< 100ms typical)
    Low,
    /// Medium latency (< 1s typical)
    Medium,
    /// High latency (> 1s typical)
    High,
}

/// Reliability characteristics of a transport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReliabilityClass {
    /// Very reliable (> 99% delivery rate)
    VeryHigh,
    /// Reliable (> 95% delivery rate)
    High,
    /// Moderately reliable (> 80% delivery rate)
    Medium,
    /// Unreliable (< 80% delivery rate)
    Low,
}

// ----------------------------------------------------------------------------
// Transport Events
// ----------------------------------------------------------------------------

/// Events that can be emitted by transports
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// New peer discovered
    PeerDiscovered {
        peer_id: PeerId,
        transport_type: TransportType,
    },
    /// Peer became unreachable
    PeerLost {
        peer_id: PeerId,
        transport_type: TransportType,
    },
    /// Transport started successfully
    Started { transport_type: TransportType },
    /// Transport stopped
    Stopped { transport_type: TransportType },
    /// Transport error occurred
    Error {
        transport_type: TransportType,
        error: String,
    },
}

/// Trait for handling transport events
pub trait TransportEventHandler: Send + Sync {
    /// Handle a transport event
    fn handle_event(&mut self, event: TransportEvent);
}

// ----------------------------------------------------------------------------
// Transport Manager
// ----------------------------------------------------------------------------

/// Manages multiple transports with intelligent routing
pub struct TransportManager {
    /// Active transports by type
    transports: Vec<Box<dyn Transport>>,
    /// Event handler for transport events
    event_handler: Option<Box<dyn TransportEventHandler>>,
    /// Transport selection policy
    selection_policy: TransportSelectionPolicy,
}

/// Policy for selecting which transport to use
#[derive(Debug, Clone)]
pub enum TransportSelectionPolicy {
    /// Always use the first available transport
    FirstAvailable,
    /// Prefer transports in the given order
    PreferenceOrder(Vec<TransportType>),
    /// Use the transport with lowest latency
    LowestLatency,
    /// Use the most reliable transport
    HighestReliability,
    /// Custom selection function
    Custom,
}

impl TransportManager {
    /// Create a new transport manager
    pub fn new() -> Self {
        Self {
            transports: Vec::new(),
            event_handler: None,
            selection_policy: TransportSelectionPolicy::FirstAvailable,
        }
    }

    /// Add a transport to the manager
    pub fn add_transport(&mut self, transport: Box<dyn Transport>) {
        self.transports.push(transport);
    }

    /// Set the event handler
    pub fn set_event_handler(&mut self, handler: Box<dyn TransportEventHandler>) {
        self.event_handler = Some(handler);
    }

    /// Set the transport selection policy
    pub fn set_selection_policy(&mut self, policy: TransportSelectionPolicy) {
        self.selection_policy = policy;
    }

    /// Start all transports
    pub async fn start_all(&mut self) -> Result<()> {
        for transport in &mut self.transports {
            transport.start().await?;
            if let Some(handler) = &mut self.event_handler {
                handler.handle_event(TransportEvent::Started {
                    transport_type: transport.capabilities().transport_type,
                });
            }
        }
        Ok(())
    }

    /// Stop all transports
    pub async fn stop_all(&mut self) -> Result<()> {
        for transport in &mut self.transports {
            if transport.is_active() {
                transport.stop().await?;
                if let Some(handler) = &mut self.event_handler {
                    handler.handle_event(TransportEvent::Stopped {
                        transport_type: transport.capabilities().transport_type,
                    });
                }
            }
        }
        Ok(())
    }

    /// Send a packet to a specific peer using the best available transport
    pub async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> Result<()> {
        let transport = self.select_transport_for_peer(&peer_id)?;
        transport.send_to(peer_id, packet).await
    }

    /// Broadcast a packet on all active transports
    pub async fn broadcast_all(&mut self, packet: BitchatPacket) -> Result<()> {
        let mut errors = Vec::new();

        for transport in &mut self.transports {
            if transport.is_active() {
                if let Err(e) = transport.broadcast(packet.clone()).await {
                    errors.push(e);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(BitchatError::InvalidPacket(
                "Broadcast failed on all transports".into(),
            ))
        }
    }

    /// Get all discovered peers across all transports
    pub fn all_discovered_peers(&self) -> SmallVec<[(PeerId, TransportType); 16]> {
        let mut peers = SmallVec::new();

        for transport in &self.transports {
            let transport_type = transport.capabilities().transport_type;
            for peer_id in transport.discovered_peers() {
                peers.push((peer_id, transport_type));
            }
        }

        peers
    }

    /// Get active transport count
    pub fn active_transport_count(&self) -> usize {
        self.transports.iter().filter(|t| t.is_active()).count()
    }

    /// Select the best transport for a specific peer
    fn select_transport_for_peer(&mut self, peer_id: &PeerId) -> Result<&mut Box<dyn Transport>> {
        // Find transports that can reach this peer
        let mut available_transports = Vec::new();

        for (i, transport) in self.transports.iter().enumerate() {
            if transport.is_active() && transport.discovered_peers().contains(peer_id) {
                available_transports.push(i);
            }
        }

        if available_transports.is_empty() {
            return Err(BitchatError::InvalidPacket(
                "No transport can reach peer".into(),
            ));
        }

        // Select transport based on policy
        let selected_index = match &self.selection_policy {
            TransportSelectionPolicy::FirstAvailable => available_transports[0],
            TransportSelectionPolicy::PreferenceOrder(order) => {
                // Find the first transport type in preference order that's available
                for preferred_type in order {
                    for &i in &available_transports {
                        if self.transports[i].capabilities().transport_type == *preferred_type {
                            return Ok(&mut self.transports[i]);
                        }
                    }
                }
                available_transports[0] // Fallback to first available
            }
            TransportSelectionPolicy::LowestLatency => {
                // Find transport with lowest latency
                let mut best_index = available_transports[0];
                let mut best_latency = self.transports[best_index].capabilities().latency_class;

                for &i in &available_transports[1..] {
                    let latency = self.transports[i].capabilities().latency_class;
                    if (latency as u8) < (best_latency as u8) {
                        best_index = i;
                        best_latency = latency;
                    }
                }
                best_index
            }
            TransportSelectionPolicy::HighestReliability => {
                // Find transport with highest reliability
                let mut best_index = available_transports[0];
                let mut best_reliability =
                    self.transports[best_index].capabilities().reliability_class;

                for &i in &available_transports[1..] {
                    let reliability = self.transports[i].capabilities().reliability_class;
                    if (reliability as u8) > (best_reliability as u8) {
                        best_index = i;
                        best_reliability = reliability;
                    }
                }
                best_index
            }
            TransportSelectionPolicy::Custom => {
                // For now, just use first available - can be extended later
                available_transports[0]
            }
        };

        Ok(&mut self.transports[selected_index])
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------------
// Mock Transport (for testing)
// ----------------------------------------------------------------------------

/// Mock transport implementation for testing
#[cfg(test)]
pub struct MockTransport {
    /// Whether the transport is active
    active: bool,
    /// Discovered peers
    peers: SmallVec<[PeerId; 8]>,
    /// Sent packets (for verification)
    sent_packets: Vec<(Option<PeerId>, BitchatPacket)>,
    /// Packets to be received
    receive_queue: Vec<(PeerId, BitchatPacket)>,
    /// Transport capabilities
    capabilities: TransportCapabilities,
}

#[cfg(test)]
impl MockTransport {
    /// Create a new mock transport
    pub fn new(transport_type: TransportType) -> Self {
        Self {
            active: false,
            peers: SmallVec::new(),
            sent_packets: Vec::new(),
            receive_queue: Vec::new(),
            capabilities: TransportCapabilities {
                transport_type,
                max_packet_size: 1024,
                supports_discovery: true,
                supports_broadcast: true,
                requires_internet: false,
                latency_class: LatencyClass::Low,
                reliability_class: ReliabilityClass::High,
            },
        }
    }

    /// Add a peer to the discovered peers list
    pub fn add_peer(&mut self, peer_id: PeerId) {
        if !self.peers.contains(&peer_id) {
            self.peers.push(peer_id);
        }
    }

    /// Queue a packet for reception
    pub fn queue_receive(&mut self, from: PeerId, packet: BitchatPacket) {
        self.receive_queue.push((from, packet));
    }

    /// Get sent packets (for verification)
    pub fn sent_packets(&self) -> &[(Option<PeerId>, BitchatPacket)] {
        &self.sent_packets
    }
}

#[cfg(test)]
#[async_trait]
impl Transport for MockTransport {
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> Result<()> {
        self.sent_packets.push((Some(peer_id), packet));
        Ok(())
    }

    async fn broadcast(&mut self, packet: BitchatPacket) -> Result<()> {
        self.sent_packets.push((None, packet));
        Ok(())
    }

    async fn receive(&mut self) -> Result<(PeerId, BitchatPacket)> {
        if let Some((peer_id, packet)) = self.receive_queue.pop() {
            Ok((peer_id, packet))
        } else {
            Err(BitchatError::InvalidPacket("No packets to receive".into()))
        }
    }

    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        self.peers.clone()
    }

    async fn start(&mut self) -> Result<()> {
        self.active = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.active = false;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn capabilities(&self) -> TransportCapabilities {
        self.capabilities.clone()
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::MessageType;

    #[tokio::test]
    async fn test_mock_transport() {
        let mut transport = MockTransport::new(TransportType::Local);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let packet = BitchatPacket::new(MessageType::Message, peer_id, b"test".to_vec());

        assert!(!transport.is_active());

        transport.start().await.unwrap();
        assert!(transport.is_active());

        transport.add_peer(peer_id);
        let expected: SmallVec<[PeerId; 8]> = smallvec::smallvec![peer_id];
        assert_eq!(transport.discovered_peers(), expected);

        transport.send_to(peer_id, packet.clone()).await.unwrap();
        assert_eq!(transport.sent_packets().len(), 1);

        transport.broadcast(packet).await.unwrap();
        assert_eq!(transport.sent_packets().len(), 2);

        transport.stop().await.unwrap();
        assert!(!transport.is_active());
    }

    #[tokio::test]
    async fn test_transport_manager() {
        let mut manager = TransportManager::new();

        let transport1 = Box::new(MockTransport::new(TransportType::Ble));
        let transport2 = Box::new(MockTransport::new(TransportType::Nostr));

        manager.add_transport(transport1);
        manager.add_transport(transport2);

        manager.start_all().await.unwrap();
        assert_eq!(manager.active_transport_count(), 2);

        manager.stop_all().await.unwrap();
        assert_eq!(manager.active_transport_count(), 0);
    }

    #[test]
    fn test_transport_capabilities() {
        let caps = TransportCapabilities {
            transport_type: TransportType::Ble,
            max_packet_size: 1024,
            supports_discovery: true,
            supports_broadcast: true,
            requires_internet: false,
            latency_class: LatencyClass::Low,
            reliability_class: ReliabilityClass::High,
        };

        assert_eq!(caps.transport_type, TransportType::Ble);
        assert!(caps.supports_discovery);
        assert!(caps.supports_broadcast);
        assert!(!caps.requires_internet);
    }
}
