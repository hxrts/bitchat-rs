//! Basic Transport Failover Implementation
//!
//! This module implements the Basic Transport Failover feature from the roadmap,
//! providing essential hybrid transport capability with BLE as primary and Nostr as fallback.
//! 
//! Design is based on the canonical MessageRouter implementation from the Swift/iOS BitChat,
//! adapted for the Rust CSP-based architecture.

use hashbrown::HashMap;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use core::time::Duration;
use serde::{Deserialize, Serialize};

use crate::types::{PeerId, Timestamp};

/// Transport type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    Ble,
    Nostr,
}

/// Basic transport routing strategies
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasicRoutingStrategy {
    /// Always try primary transport first, fallback to secondary (canonical behavior)
    PreferPrimary,
    /// Round-robin between available transports
    LoadBalance,
    /// Send via all available transports for redundancy
    BroadcastAll,
}

/// Transport health and availability status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStatus {
    /// Transport type
    pub transport_type: TransportType,
    /// Whether transport is currently available
    pub is_available: bool,
    /// Timestamp of last successful operation
    pub last_success: Option<Timestamp>,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Last failure timestamp
    pub last_failure: Option<Timestamp>,
    /// Average latency in milliseconds (if measured)
    pub avg_latency_ms: Option<u64>,
}

impl TransportStatus {
    /// Create a new transport status
    pub fn new(transport_type: TransportType) -> Self {
        Self {
            transport_type,
            is_available: false,
            last_success: None,
            consecutive_failures: 0,
            last_failure: None,
            avg_latency_ms: None,
        }
    }
    
    /// Record a successful operation
    pub fn record_success(&mut self, latency_ms: Option<u64>) {
        self.is_available = true;
        self.last_success = Some(Timestamp::now());
        self.consecutive_failures = 0;
        self.avg_latency_ms = latency_ms;
    }
    
    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.last_failure = Some(Timestamp::now());
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        
        // Only mark as unavailable after multiple consecutive failures
        if self.consecutive_failures >= 3 {
            self.is_available = false;
        }
    }
    
    /// Check if transport should be considered healthy
    pub fn is_healthy(&self) -> bool {
        self.is_available && self.consecutive_failures < 3
    }
}

/// Configuration for basic transport failover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Primary transport (BLE by default, canonical behavior)
    pub primary_transport: TransportType,
    /// Fallback transport (Nostr by default, canonical behavior)
    pub fallback_transport: TransportType,
    /// Routing strategy
    pub routing_strategy: BasicRoutingStrategy,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum failures before marking transport unhealthy
    pub max_consecutive_failures: u32,
    /// Timeout for transport operations
    pub operation_timeout: Duration,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            primary_transport: TransportType::Ble,      // Canonical: BLE first
            fallback_transport: TransportType::Nostr,   // Canonical: Nostr fallback
            routing_strategy: BasicRoutingStrategy::PreferPrimary, // Canonical behavior
            health_check_interval: Duration::from_secs(30),
            max_consecutive_failures: 3,
            operation_timeout: Duration::from_secs(10),
        }
    }
}

/// Peer reachability information for transport selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReachability {
    /// Peer ID
    pub peer_id: PeerId,
    /// Whether peer is reachable via BLE mesh
    pub ble_reachable: bool,
    /// Whether peer has Nostr public key (mutual favorite)
    pub nostr_available: bool,
    /// Last time peer was seen on BLE
    pub last_ble_seen: Option<Timestamp>,
    /// Last time peer interacted via Nostr
    pub last_nostr_seen: Option<Timestamp>,
}

impl PeerReachability {
    /// Create new peer reachability info
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            ble_reachable: false,
            nostr_available: false,
            last_ble_seen: None,
            last_nostr_seen: None,
        }
    }
    
    /// Update BLE reachability
    pub fn update_ble_reachability(&mut self, reachable: bool) {
        self.ble_reachable = reachable;
        if reachable {
            self.last_ble_seen = Some(Timestamp::now());
        }
    }
    
    /// Update Nostr availability
    pub fn update_nostr_availability(&mut self, available: bool) {
        self.nostr_available = available;
        if available {
            self.last_nostr_seen = Some(Timestamp::now());
        }
    }
    
    /// Get preferred transport for this peer (canonical logic)
    pub fn preferred_transport(&self, config: &FailoverConfig) -> Option<TransportType> {
        match config.routing_strategy {
            BasicRoutingStrategy::PreferPrimary => {
                // Canonical: BLE first if reachable, Nostr if available
                if self.ble_reachable && config.primary_transport == TransportType::Ble {
                    Some(TransportType::Ble)
                } else if self.nostr_available {
                    Some(TransportType::Nostr)
                } else {
                    None
                }
            }
            BasicRoutingStrategy::LoadBalance => {
                // Simple load balancing based on last seen times
                match (self.ble_reachable, self.nostr_available) {
                    (true, true) => {
                        // Choose based on recency or alternate
                        if let (Some(ble_time), Some(nostr_time)) = (&self.last_ble_seen, &self.last_nostr_seen) {
                            if ble_time > nostr_time {
                                Some(TransportType::Ble)
                            } else {
                                Some(TransportType::Nostr)
                            }
                        } else {
                            Some(config.primary_transport)
                        }
                    }
                    (true, false) => Some(TransportType::Ble),
                    (false, true) => Some(TransportType::Nostr),
                    (false, false) => None,
                }
            }
            BasicRoutingStrategy::BroadcastAll => {
                // For broadcast, we'll need to send via all available transports
                // Return primary for now, caller should check all transports
                if self.ble_reachable {
                    Some(TransportType::Ble)
                } else if self.nostr_available {
                    Some(TransportType::Nostr)
                } else {
                    None
                }
            }
        }
    }
}

/// Message type for transport routing decisions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageContext {
    /// Private direct message
    Private { recipient: PeerId },
    /// Public message to mesh channel
    PublicMesh,
    /// Public message to location/geohash channel
    PublicLocation,
    /// Read receipt
    ReadReceipt { recipient: PeerId },
    /// Delivery acknowledgment
    DeliveryAck { recipient: PeerId },
    /// Favorite notification
    FavoriteNotification { recipient: PeerId },
}

/// Transport selection result
#[derive(Debug, Clone)]
pub enum TransportSelection {
    /// Use specific transport
    UseTransport(TransportType),
    /// Use all available transports (for broadcast)
    UseAll(Vec<TransportType>),
    /// Queue message (no transports available)
    Queue,
    /// Message cannot be sent
    CannotSend { reason: String },
}

/// Basic Transport Manager - implements the canonical MessageRouter pattern
/// 
/// This provides the core failover logic inspired by the Swift MessageRouter:
/// - BLE-first for private messages when peer is reachable
/// - Nostr fallback when BLE unavailable but Nostr key exists
/// - Message queuing when no transports available
/// - Health monitoring for transport availability
pub struct BasicTransportManager {
    /// Configuration
    config: FailoverConfig,
    /// Transport status tracking
    transport_status: HashMap<TransportType, TransportStatus>,
    /// Peer reachability information
    peer_reachability: HashMap<PeerId, PeerReachability>,
    /// Last health check timestamp
    last_health_check: Option<Timestamp>,
}

impl BasicTransportManager {
    /// Create a new transport manager
    pub fn new(config: FailoverConfig) -> Self {
        let mut transport_status = HashMap::new();
        transport_status.insert(TransportType::Ble, TransportStatus::new(TransportType::Ble));
        transport_status.insert(TransportType::Nostr, TransportStatus::new(TransportType::Nostr));
        
        Self {
            config,
            transport_status,
            peer_reachability: HashMap::new(),
            last_health_check: None,
        }
    }
    
    /// Create with default configuration (canonical: BLE primary, Nostr fallback)
    pub fn new_canonical() -> Self {
        Self::new(FailoverConfig::default())
    }
    
    /// Update transport availability status
    pub fn update_transport_status(&mut self, transport_type: TransportType, available: bool, latency_ms: Option<u64>) {
        if let Some(status) = self.transport_status.get_mut(&transport_type) {
            if available {
                status.record_success(latency_ms);
            } else {
                status.record_failure();
            }
        }
    }
    
    /// Update peer reachability information
    pub fn update_peer_reachability(&mut self, peer_id: PeerId, ble_reachable: bool, nostr_available: bool) {
        let reachability = self.peer_reachability.entry(peer_id).or_insert_with(|| PeerReachability::new(peer_id));
        reachability.update_ble_reachability(ble_reachable);
        reachability.update_nostr_availability(nostr_available);
    }
    
    /// Select transport for message (canonical MessageRouter logic)
    pub fn select_transport(&self, message_context: &MessageContext) -> TransportSelection {
        match message_context {
            MessageContext::Private { recipient } 
            | MessageContext::ReadReceipt { recipient }
            | MessageContext::DeliveryAck { recipient }
            | MessageContext::FavoriteNotification { recipient } => {
                self.select_transport_for_peer(*recipient)
            }
            MessageContext::PublicMesh => {
                // Mesh messages always go via BLE (canonical behavior)
                if self.is_transport_healthy(TransportType::Ble) {
                    TransportSelection::UseTransport(TransportType::Ble)
                } else {
                    TransportSelection::Queue
                }
            }
            MessageContext::PublicLocation => {
                // Location messages go via Nostr (canonical behavior)
                if self.is_transport_healthy(TransportType::Nostr) {
                    TransportSelection::UseTransport(TransportType::Nostr)
                } else {
                    TransportSelection::Queue
                }
            }
        }
    }
    
    /// Select transport for a specific peer (canonical private message logic)
    fn select_transport_for_peer(&self, peer_id: PeerId) -> TransportSelection {
        let reachability = self.peer_reachability.get(&peer_id);
        
        match self.config.routing_strategy {
            BasicRoutingStrategy::PreferPrimary => {
                // Canonical logic: BLE first if reachable, Nostr fallback if available
                if let Some(reach) = reachability {
                    if reach.ble_reachable && self.is_transport_healthy(TransportType::Ble) {
                        TransportSelection::UseTransport(TransportType::Ble)
                    } else if reach.nostr_available && self.is_transport_healthy(TransportType::Nostr) {
                        TransportSelection::UseTransport(TransportType::Nostr)
                    } else {
                        TransportSelection::Queue
                    }
                } else {
                    // No reachability info - try primary transport
                    if self.is_transport_healthy(self.config.primary_transport) {
                        TransportSelection::UseTransport(self.config.primary_transport)
                    } else {
                        TransportSelection::Queue
                    }
                }
            }
            BasicRoutingStrategy::LoadBalance => {
                if let Some(reach) = reachability {
                    if let Some(transport) = reach.preferred_transport(&self.config) {
                        if self.is_transport_healthy(transport) {
                            TransportSelection::UseTransport(transport)
                        } else {
                            // Try the other transport
                            let other = match transport {
                                TransportType::Ble => TransportType::Nostr,
                                TransportType::Nostr => TransportType::Ble,
                            };
                            if self.is_transport_healthy(other) {
                                TransportSelection::UseTransport(other)
                            } else {
                                TransportSelection::Queue
                            }
                        }
                    } else {
                        TransportSelection::CannotSend { 
                            reason: "Peer not reachable via any transport".to_string() 
                        }
                    }
                } else {
                    TransportSelection::Queue
                }
            }
            BasicRoutingStrategy::BroadcastAll => {
                let mut available_transports = Vec::new();
                
                if let Some(reach) = reachability {
                    if reach.ble_reachable && self.is_transport_healthy(TransportType::Ble) {
                        available_transports.push(TransportType::Ble);
                    }
                    if reach.nostr_available && self.is_transport_healthy(TransportType::Nostr) {
                        available_transports.push(TransportType::Nostr);
                    }
                }
                
                if available_transports.is_empty() {
                    TransportSelection::Queue
                } else {
                    TransportSelection::UseAll(available_transports)
                }
            }
        }
    }
    
    /// Check if transport is healthy
    pub fn is_transport_healthy(&self, transport_type: TransportType) -> bool {
        self.transport_status
            .get(&transport_type)
            .map(|status| status.is_healthy())
            .unwrap_or(false)
    }
    
    /// Get transport status
    pub fn get_transport_status(&self, transport_type: TransportType) -> Option<&TransportStatus> {
        self.transport_status.get(&transport_type)
    }
    
    /// Get all transport statuses
    pub fn get_all_transport_status(&self) -> &HashMap<TransportType, TransportStatus> {
        &self.transport_status
    }
    
    /// Get peer reachability info
    pub fn get_peer_reachability(&self, peer_id: PeerId) -> Option<&PeerReachability> {
        self.peer_reachability.get(&peer_id)
    }
    
    /// Get all available transports
    pub fn get_available_transports(&self) -> Vec<TransportType> {
        self.transport_status
            .iter()
            .filter_map(|(transport_type, status)| {
                if status.is_healthy() {
                    Some(*transport_type)
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Perform health check on all transports
    pub fn health_check(&mut self) -> bool {
        self.last_health_check = Some(Timestamp::now());
        
        // For now, just report if any transport is healthy
        // In a full implementation, this would trigger actual health checks
        self.transport_status.values().any(|status| status.is_healthy())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: FailoverConfig) {
        self.config = config;
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &FailoverConfig {
        &self.config
    }
}

impl Default for BasicTransportManager {
    fn default() -> Self {
        Self::new_canonical()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_transport_status_lifecycle() {
        let mut status = TransportStatus::new(TransportType::Ble);
        
        // Initially unhealthy
        assert!(!status.is_healthy());
        assert_eq!(status.consecutive_failures, 0);
        
        // Record success
        status.record_success(Some(50));
        assert!(status.is_healthy());
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.avg_latency_ms, Some(50));
        
        // Record failures
        status.record_failure();
        assert!(status.is_healthy()); // Still healthy after 1 failure
        status.record_failure();
        assert!(status.is_healthy()); // Still healthy after 2 failures
        status.record_failure();
        assert!(!status.is_healthy()); // Unhealthy after 3 failures
    }

    #[test]
    fn test_canonical_transport_selection() {
        let mut manager = BasicTransportManager::new_canonical();
        let peer_id = create_test_peer_id(1);
        
        // Set up healthy transports
        manager.update_transport_status(TransportType::Ble, true, Some(25));
        manager.update_transport_status(TransportType::Nostr, true, Some(100));
        
        // Test private message with BLE reachable peer (canonical: BLE first)
        manager.update_peer_reachability(peer_id, true, true);
        let selection = manager.select_transport(&MessageContext::Private { recipient: peer_id });
        match selection {
            TransportSelection::UseTransport(TransportType::Ble) => {}, // Expected
            _ => panic!("Expected BLE transport for reachable peer"),
        }
        
        // Test private message with only Nostr available (canonical: Nostr fallback)
        manager.update_peer_reachability(peer_id, false, true);
        let selection = manager.select_transport(&MessageContext::Private { recipient: peer_id });
        match selection {
            TransportSelection::UseTransport(TransportType::Nostr) => {}, // Expected
            _ => panic!("Expected Nostr transport when BLE unavailable"),
        }
        
        // Test mesh message (canonical: always BLE)
        let selection = manager.select_transport(&MessageContext::PublicMesh);
        match selection {
            TransportSelection::UseTransport(TransportType::Ble) => {}, // Expected
            _ => panic!("Expected BLE transport for mesh messages"),
        }
        
        // Test location message (canonical: always Nostr)
        let selection = manager.select_transport(&MessageContext::PublicLocation);
        match selection {
            TransportSelection::UseTransport(TransportType::Nostr) => {}, // Expected
            _ => panic!("Expected Nostr transport for location messages"),
        }
    }

    #[test]
    fn test_peer_reachability() {
        let peer_id = create_test_peer_id(1);
        let mut reachability = PeerReachability::new(peer_id);
        let config = FailoverConfig::default();
        
        // Initially no transport available
        assert_eq!(reachability.preferred_transport(&config), None);
        
        // BLE becomes reachable
        reachability.update_ble_reachability(true);
        assert_eq!(reachability.preferred_transport(&config), Some(TransportType::Ble));
        
        // Nostr becomes available
        reachability.update_nostr_availability(true);
        // Should still prefer BLE (primary transport)
        assert_eq!(reachability.preferred_transport(&config), Some(TransportType::Ble));
        
        // BLE becomes unreachable
        reachability.update_ble_reachability(false);
        // Should fallback to Nostr
        assert_eq!(reachability.preferred_transport(&config), Some(TransportType::Nostr));
    }

    #[test]
    fn test_broadcast_strategy() {
        let mut config = FailoverConfig::default();
        config.routing_strategy = BasicRoutingStrategy::BroadcastAll;
        let mut manager = BasicTransportManager::new(config);
        let peer_id = create_test_peer_id(1);
        
        // Set up healthy transports
        manager.update_transport_status(TransportType::Ble, true, Some(25));
        manager.update_transport_status(TransportType::Nostr, true, Some(100));
        
        // Both transports available for peer
        manager.update_peer_reachability(peer_id, true, true);
        let selection = manager.select_transport(&MessageContext::Private { recipient: peer_id });
        match selection {
            TransportSelection::UseAll(transports) => {
                assert_eq!(transports.len(), 2);
                assert!(transports.contains(&TransportType::Ble));
                assert!(transports.contains(&TransportType::Nostr));
            }
            _ => panic!("Expected UseAll for broadcast strategy"),
        }
    }
}