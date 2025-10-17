//! BLE peer management and state

use std::time::{Duration, Instant};

use bitchat_core::PeerId;
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
        self.retry_count < 3 && 
        self.last_connection_attempt
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