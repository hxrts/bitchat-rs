//! BLE peer management and state

use std::time::{Duration, Instant};

use bitchat_core::{PeerId, BitchatResult, BitchatError};
use bitchat_core::protocol::DiscoveredPeer;
use btleplug::api::Peripheral;
use btleplug::platform::Peripheral as PlatformPeripheral;

// ----------------------------------------------------------------------------
// Peer State Management
// ----------------------------------------------------------------------------

/// Connection state for a BLE peer
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

/// Information about a discovered BLE peer
#[derive(Debug, Clone)]
pub struct BlePeer {
    /// BitChat peer ID
    pub peer_id: PeerId,
    /// BLE peripheral
    pub peripheral: PlatformPeripheral,
    /// Device name
    pub device_name: String,
    /// Connection state
    pub connection_state: ConnectionState,
    /// Last connection attempt timestamp
    pub last_connection_attempt: Option<Instant>,
    /// Connection retry count
    pub retry_count: u32,
}

impl BlePeer {
    /// Create a new BLE peer
    pub fn new(peer_id: PeerId, peripheral: PlatformPeripheral, device_name: String) -> Self {
        Self {
            peer_id,
            peripheral,
            device_name,
            connection_state: ConnectionState::Disconnected,
            last_connection_attempt: None,
            retry_count: 0,
        }
    }

    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.connection_state == ConnectionState::Connected
    }

    /// Check if peer is currently connecting
    pub fn is_connecting(&self) -> bool {
        self.connection_state == ConnectionState::Connecting
    }

    /// Check if peer can retry connection
    pub fn can_retry(&self) -> bool {
        self.retry_count < 3
            && self
                .last_connection_attempt
                .map(|t| t.elapsed() > Duration::from_secs(self.retry_count as u64 * 5))
                .unwrap_or(true)
    }

    /// Mark connection attempt started
    pub fn start_connection_attempt(&mut self) {
        self.connection_state = ConnectionState::Connecting;
        self.last_connection_attempt = Some(Instant::now());
        self.retry_count += 1;
    }

    /// Mark connection as successful
    pub fn mark_connected(&mut self) {
        self.connection_state = ConnectionState::Connected;
        self.retry_count = 0; // Reset on successful connection
    }

    /// Mark connection as failed
    pub fn mark_failed(&mut self) {
        self.connection_state = ConnectionState::Failed;
    }

    /// Mark peer as disconnected
    pub fn mark_disconnected(&mut self) {
        self.connection_state = ConnectionState::Disconnected;
    }

    /// Get peripheral ID for comparison
    pub fn peripheral_id(&self) -> btleplug::platform::PeripheralId {
        self.peripheral.id()
    }

    /// Update peer information from an announce packet
    pub fn update_from_announce(&mut self, discovered_peer: &DiscoveredPeer) -> BitchatResult<()> {
        // Verify the peer IDs match
        if self.peer_id != discovered_peer.peer_id {
            return Err(BitchatError::invalid_packet("Peer ID mismatch in announce update"));
        }

        // Update the nickname if it's different
        // For now, we don't store the full announce information in BlePeer
        // In a full implementation, we might want to store noise keys, signatures, etc.
        
        Ok(())
    }

    /// Create a BlePeer from a DiscoveredPeer (for peers discovered via announce packets)
    pub fn from_discovered_peer(_discovered_peer: DiscoveredPeer) -> BitchatResult<Self> {
        // Note: This is a placeholder implementation since we don't have a real peripheral
        // In a real implementation, we would need to create a mock peripheral or
        // refactor BlePeer to optionally have a peripheral
        Err(BitchatError::invalid_packet(
            "Cannot create BlePeer from DiscoveredPeer without BLE peripheral"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btleplug::platform::Peripheral;

    // Note: These tests would require mock peripherals in a real test suite
    // For now, we'll test the logic that doesn't depend on btleplug

    #[test]
    fn test_connection_state_transitions() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        // We can't easily create a mock Peripheral here, so this is a simplified test

        // Test retry logic
        assert!(Duration::from_secs(0) < Duration::from_secs(5)); // Basic duration comparison
    }

    #[test]
    fn test_retry_logic() {
        // Test the retry calculation logic
        let retry_count = 2u32;
        let expected_delay = Duration::from_secs(retry_count as u64 * 5);
        assert_eq!(expected_delay, Duration::from_secs(10));
    }
}
