//! Connection state management for BitChat runtime
//!
//! This module contains the ConnectionManager that manages peer connection states
//! and handles connection lifecycle transitions.

use std::collections::HashMap;
use bitchat_core::{
    PeerId, ChannelTransportType,
    internal::{
        ConnectionState, ConnectionEvent, StateTransition, StateTransitionError,
        AuditEntry, TimeSource
    }
};

// ----------------------------------------------------------------------------
// Connection Manager
// ----------------------------------------------------------------------------

/// Manages connection states for all peers
#[derive(Debug)]
pub struct ConnectionManager<T: TimeSource> {
    /// Active peer connections
    connections: HashMap<PeerId, ConnectionState>,
    /// Connection audit trail
    audit_trail: Vec<AuditEntry>,
    /// Time source for generating timestamps
    time_source: T,
    /// Statistics
    stats: ConnectionStats,
}

impl<T: TimeSource> ConnectionManager<T> {
    /// Create a new connection manager
    pub fn new(time_source: T) -> Self {
        Self {
            connections: HashMap::new(),
            audit_trail: Vec::new(),
            time_source,
            stats: ConnectionStats::default(),
        }
    }

    /// Get connection state for a peer
    pub fn get_connection(&self, peer_id: &PeerId) -> Option<&ConnectionState> {
        self.connections.get(peer_id)
    }

    /// Get mutable connection state for a peer
    pub fn get_connection_mut(&mut self, peer_id: &PeerId) -> Option<&mut ConnectionState> {
        self.connections.get_mut(peer_id)
    }

    /// Get all connections
    pub fn get_all_connections(&self) -> &HashMap<PeerId, ConnectionState> {
        &self.connections
    }

    /// Get connections for a specific transport
    pub fn get_connections_by_transport(&self, transport: ChannelTransportType) -> Vec<(&PeerId, &ConnectionState)> {
        self.connections
            .iter()
            .filter(|(_, state)| {
                match state {
                    ConnectionState::Connecting(s) => s.transport == transport,
                    ConnectionState::Connected(s) => s.transport == transport,
                    ConnectionState::Failed(s) => s.transport == Some(transport),
                    _ => false,
                }
            })
            .collect()
    }

    /// Initialize connection for a peer (if not already exists)
    pub fn initialize_peer(&mut self, peer_id: PeerId) -> &ConnectionState {
        self.connections.entry(peer_id).or_insert_with(|| {
            self.stats.peers_initialized += 1;
            ConnectionState::new_disconnected(peer_id)
        })
    }

    /// Process a connection event and update state
    pub fn process_connection_event(
        &mut self, 
        peer_id: PeerId, 
        event: ConnectionEvent
    ) -> Result<StateTransition, StateTransitionError> {
        // Ensure peer is initialized
        self.initialize_peer(peer_id);
        
        // Take the current state to consume it
        let current_state = self.connections.remove(&peer_id)
            .expect("Peer must exist after initialization");
        
        // Process the transition
        let transition = current_state.transition(event)?;
        
        // Store the new state
        self.connections.insert(peer_id, transition.new_state.clone());
        
        // Record audit entry
        self.audit_trail.push(transition.audit_entry.clone());
        self.stats.state_transitions += 1;
        
        // Cleanup old audit entries (keep last 1000)
        if self.audit_trail.len() > 1000 {
            self.audit_trail.remove(0);
        }

        Ok(transition)
    }

    /// Remove a peer's connection
    pub fn remove_peer(&mut self, peer_id: &PeerId) -> Option<ConnectionState> {
        let removed = self.connections.remove(peer_id);
        if removed.is_some() {
            self.stats.peers_removed += 1;
        }
        removed
    }

    /// Get count of peers in each state
    pub fn get_state_counts(&self) -> StateDistribution {
        let mut distribution = StateDistribution::default();
        
        for state in self.connections.values() {
            match state {
                ConnectionState::Disconnected(_) => distribution.disconnected += 1,
                ConnectionState::Discovering(_) => distribution.discovering += 1,
                ConnectionState::Connecting(_) => distribution.connecting += 1,
                ConnectionState::Connected(_) => distribution.connected += 1,
                ConnectionState::Failed(_) => distribution.failed += 1,
            }
        }
        
        distribution
    }

    /// Get peers that have timed out
    pub fn get_timed_out_peers(&self) -> Vec<(PeerId, ConnectionEvent)> {
        let current_time = self.time_source.now();
        let mut timed_out = Vec::new();

        for (peer_id, state) in &self.connections {
            if let Some(timeout) = state.has_timeout() {
                if current_time.as_millis() >= timeout.as_millis() {
                    timed_out.push((*peer_id, ConnectionEvent::Timeout));
                }
            }
        }

        timed_out
    }

    /// Get peers that can send messages
    pub fn get_message_ready_peers(&self) -> Vec<PeerId> {
        self.connections
            .iter()
            .filter_map(|(peer_id, state)| {
                if state.can_send_messages() {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get connection quality scores for all peers
    pub fn get_quality_scores(&self) -> HashMap<PeerId, u8> {
        self.connections
            .iter()
            .map(|(peer_id, state)| (*peer_id, state.quality_score()))
            .collect()
    }

    /// Get recent audit entries
    pub fn get_recent_audit_entries(&self, limit: usize) -> &[AuditEntry] {
        let start = if self.audit_trail.len() > limit {
            self.audit_trail.len() - limit
        } else {
            0
        };
        &self.audit_trail[start..]
    }

    /// Get connection statistics
    pub fn get_statistics(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Clear disconnected and failed peers older than specified age
    pub fn cleanup_old_peers(&mut self, max_age_seconds: u64) {
        let current_time = self.time_source.now();
        let cutoff_time = current_time.as_millis().saturating_sub(max_age_seconds * 1000);

        let mut to_remove = Vec::new();
        
        for (peer_id, state) in &self.connections {
            let should_remove = match state {
                ConnectionState::Disconnected(s) => {
                    if let Some(last_seen) = s.last_seen {
                        last_seen.as_millis() < cutoff_time
                    } else {
                        false // Keep peers we've never seen
                    }
                },
                ConnectionState::Failed(s) => {
                    s.failed_at.as_millis() < cutoff_time
                },
                _ => false, // Keep active connections
            };
            
            if should_remove {
                to_remove.push(*peer_id);
            }
        }

        for peer_id in to_remove {
            self.remove_peer(&peer_id);
            self.stats.peers_cleaned_up += 1;
        }
    }
}

// ----------------------------------------------------------------------------
// Supporting Types
// ----------------------------------------------------------------------------

/// Statistics for connection management
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Number of peers initialized
    pub peers_initialized: u64,
    /// Number of peers removed
    pub peers_removed: u64,
    /// Number of state transitions processed
    pub state_transitions: u64,
    /// Number of peers cleaned up due to age
    pub peers_cleaned_up: u64,
}

/// Distribution of peers across connection states
#[derive(Debug, Clone, Default)]
pub struct StateDistribution {
    pub disconnected: usize,
    pub discovering: usize,
    pub connecting: usize,
    pub connected: usize,
    pub failed: usize,
}

impl StateDistribution {
    /// Get total number of peers
    pub fn total(&self) -> usize {
        self.disconnected + self.discovering + self.connecting + self.connected + self.failed
    }

    /// Get percentage of peers in connected state
    pub fn connected_percentage(&self) -> f32 {
        if self.total() == 0 {
            0.0
        } else {
            self.connected as f32 / self.total() as f32 * 100.0
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::internal::SessionParams;
    use bitchat_core::SystemTimeSource;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    fn create_test_session_params() -> SessionParams {
        SessionParams {
            protocol_version: 1,
            encryption_key: vec![1, 2, 3, 4],
            timeout_seconds: 30,
        }
    }

    #[test]
    fn test_connection_manager_initialization() {
        let time_source = SystemTimeSource;
        let mut manager = ConnectionManager::new(time_source);
        let peer_id = create_test_peer_id(1);

        let state = manager.initialize_peer(peer_id);
        assert_eq!(state.peer_id(), peer_id);
        assert_eq!(state.state_name(), "Disconnected");
        assert_eq!(manager.get_statistics().peers_initialized, 1);
    }

    #[test]
    fn test_connection_state_transition() {
        let time_source = SystemTimeSource;
        let mut manager = ConnectionManager::new(time_source);
        let peer_id = create_test_peer_id(1);

        // Initialize peer
        manager.initialize_peer(peer_id);

        // Start discovery
        let transition = manager.process_connection_event(
            peer_id, 
            ConnectionEvent::StartDiscovery { timeout_seconds: Some(60) }
        ).unwrap();

        assert_eq!(transition.new_state.state_name(), "Discovering");
        assert!(!transition.effects.is_empty());
        assert_eq!(manager.get_statistics().state_transitions, 1);
    }

    #[test]
    fn test_state_distribution() {
        let time_source = SystemTimeSource;
        let mut manager = ConnectionManager::new(time_source);

        // Add peers in different states
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);
        
        manager.initialize_peer(peer1);
        manager.initialize_peer(peer2);
        
        manager.process_connection_event(
            peer1, 
            ConnectionEvent::StartDiscovery { timeout_seconds: Some(60) }
        ).unwrap();

        let distribution = manager.get_state_counts();
        assert_eq!(distribution.total(), 2);
        assert_eq!(distribution.disconnected, 1);
        assert_eq!(distribution.discovering, 1);
    }

    #[test]
    fn test_connection_removal() {
        let time_source = SystemTimeSource;
        let mut manager = ConnectionManager::new(time_source);
        let peer_id = create_test_peer_id(1);

        manager.initialize_peer(peer_id);
        assert!(manager.get_connection(&peer_id).is_some());

        let removed = manager.remove_peer(&peer_id);
        assert!(removed.is_some());
        assert!(manager.get_connection(&peer_id).is_none());
        assert_eq!(manager.get_statistics().peers_removed, 1);
    }
}