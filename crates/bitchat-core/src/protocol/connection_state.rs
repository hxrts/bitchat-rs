//! Linear Connection State Machine
//!
//! Provides type-safe connection lifecycle management that eliminates invalid
//! state transitions through linear type enforcement.

use crate::channel::ChannelTransportType;
use crate::{channel::Effect, types::Timestamp, PeerId};
use serde::{Deserialize, Serialize};
cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::fmt;
    } else {
        use core::fmt;
        use alloc::string::{String, ToString};
        use alloc::vec::{Vec, vec};
        use alloc::format;
        use alloc::boxed::Box;
    }
}

// ----------------------------------------------------------------------------
// Connection State Types
// ----------------------------------------------------------------------------

/// Linear connection state that must be consumed to transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionState {
    /// No connection to peer
    Disconnected(DisconnectedState),
    /// Discovering peer via transports
    Discovering(DiscoveringState),
    /// Attempting to establish connection
    Connecting(ConnectingState),
    /// Actively connected with established session
    Connected(ConnectedState),
    /// Connection failed with error details
    Failed(FailedState),
}

/// State when no connection exists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectedState {
    pub peer_id: PeerId,
    pub last_seen: Option<Timestamp>,
    pub failed_attempts: u32,
}

/// State when discovering peer via transports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveringState {
    pub peer_id: PeerId,
    pub discovery_started: Timestamp,
    pub discovered_transports: Vec<ChannelTransportType>,
    pub discovery_timeout: Option<Timestamp>,
}

/// State when attempting to establish connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectingState {
    pub peer_id: PeerId,
    pub transport: ChannelTransportType,
    pub connection_started: Timestamp,
    pub connection_timeout: Timestamp,
    pub session_params: Option<SessionParams>,
}

/// State when connection is established
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedState {
    pub peer_id: PeerId,
    pub transport: ChannelTransportType,
    pub connected_since: Timestamp,
    pub session_id: String,
    pub last_activity: Timestamp,
    pub message_count: u64,
}

/// State when connection failed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedState {
    pub peer_id: PeerId,
    pub transport: Option<ChannelTransportType>,
    pub failed_at: Timestamp,
    pub error_reason: String,
    pub retry_after: Option<Timestamp>,
}

/// Session parameters for connection establishment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionParams {
    pub protocol_version: u32,
    pub encryption_key: Vec<u8>,
    pub timeout_seconds: u64,
}

// ----------------------------------------------------------------------------
// State Transition Events
// ----------------------------------------------------------------------------

/// Events that trigger state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionEvent {
    /// Start discovering peer via transports
    StartDiscovery { timeout_seconds: Option<u64> },
    /// Peer discovered via specific transport
    PeerDiscovered {
        transport: ChannelTransportType,
        signal_strength: Option<i8>,
    },
    /// Initiate connection to discovered peer
    InitiateConnection {
        transport: ChannelTransportType,
        session_params: SessionParams,
    },
    /// Connection established successfully
    ConnectionEstablished { session_id: String },
    /// Connection failed with error
    ConnectionFailed { reason: String },
    /// Connection lost during operation
    ConnectionLost { reason: String },
    /// Activity detected on connection
    ActivityDetected,
    /// Timeout occurred
    Timeout,
    /// Manual disconnection requested
    Disconnect,
    /// Retry connection after failure
    Retry,
}

// ----------------------------------------------------------------------------
// State Transition Results
// ----------------------------------------------------------------------------

/// Result of a state transition
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// New connection state
    pub new_state: ConnectionState,
    /// Effects to execute as result of transition
    pub effects: Vec<Effect>,
    /// Audit trail entry
    pub audit_entry: AuditEntry,
}

/// Audit trail entry for state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: Timestamp,
    pub peer_id: PeerId,
    pub from_state: String,
    pub to_state: String,
    pub event: String,
    pub effects_count: usize,
}

// ----------------------------------------------------------------------------
// State Machine Implementation
// ----------------------------------------------------------------------------

impl ConnectionState {
    /// Create initial disconnected state for a peer
    pub fn new_disconnected(peer_id: PeerId) -> Self {
        ConnectionState::Disconnected(DisconnectedState {
            peer_id,
            last_seen: None,
            failed_attempts: 0,
        })
    }

    /// Get peer ID for any state
    pub fn peer_id(&self) -> PeerId {
        match self {
            ConnectionState::Disconnected(s) => s.peer_id,
            ConnectionState::Discovering(s) => s.peer_id,
            ConnectionState::Connecting(s) => s.peer_id,
            ConnectionState::Connected(s) => s.peer_id,
            ConnectionState::Failed(s) => s.peer_id,
        }
    }

    /// Get current state name for logging/audit
    pub fn state_name(&self) -> &'static str {
        match self {
            ConnectionState::Disconnected(_) => "Disconnected",
            ConnectionState::Discovering(_) => "Discovering",
            ConnectionState::Connecting(_) => "Connecting",
            ConnectionState::Connected(_) => "Connected",
            ConnectionState::Failed(_) => "Failed",
        }
    }

    /// Process an event and transition to new state (consumes self)
    pub fn transition(
        self,
        event: ConnectionEvent,
    ) -> Result<StateTransition, StateTransitionError> {
        let peer_id = self.peer_id();
        let from_state = self.state_name().to_string();
        let event_name = format!("{:?}", event);

        let (new_state, effects) = match (self, event) {
            // From Disconnected
            (
                ConnectionState::Disconnected(state),
                ConnectionEvent::StartDiscovery { timeout_seconds },
            ) => {
                let timeout = timeout_seconds.map(|secs| current_timestamp() + secs);
                let new_state = ConnectionState::Discovering(DiscoveringState {
                    peer_id: state.peer_id,
                    discovery_started: current_timestamp(),
                    discovered_transports: Vec::new(),
                    discovery_timeout: timeout,
                });
                let effects = vec![
                    Effect::StartTransportDiscovery {
                        transport: ChannelTransportType::Ble,
                    },
                    Effect::StartTransportDiscovery {
                        transport: ChannelTransportType::Nostr,
                    },
                ];
                (new_state, effects)
            }

            // From Discovering
            (
                ConnectionState::Discovering(mut state),
                ConnectionEvent::PeerDiscovered { transport, .. },
            ) => {
                if !state.discovered_transports.contains(&transport) {
                    state.discovered_transports.push(transport);
                }
                (ConnectionState::Discovering(state), Vec::new())
            }

            (
                ConnectionState::Discovering(state),
                ConnectionEvent::InitiateConnection {
                    transport,
                    session_params,
                },
            ) => {
                let timeout = current_timestamp() + session_params.timeout_seconds;
                let new_state = ConnectionState::Connecting(ConnectingState {
                    peer_id: state.peer_id,
                    transport,
                    connection_started: current_timestamp(),
                    connection_timeout: timeout,
                    session_params: Some(session_params),
                });
                let effects = vec![Effect::InitiateConnection {
                    peer_id: state.peer_id,
                    transport,
                }];
                (new_state, effects)
            }

            (ConnectionState::Discovering(_), ConnectionEvent::Timeout) => {
                let new_state = ConnectionState::Failed(FailedState {
                    peer_id,
                    transport: None,
                    failed_at: current_timestamp(),
                    error_reason: "Discovery timeout".to_string(),
                    retry_after: Some(current_timestamp() + 60), // Retry after 1 minute
                });
                (new_state, Vec::new())
            }

            // From Connecting
            (
                ConnectionState::Connecting(state),
                ConnectionEvent::ConnectionEstablished { session_id },
            ) => {
                let new_state = ConnectionState::Connected(ConnectedState {
                    peer_id: state.peer_id,
                    transport: state.transport,
                    connected_since: current_timestamp(),
                    session_id,
                    last_activity: current_timestamp(),
                    message_count: 0,
                });
                (new_state, Vec::new())
            }

            (ConnectionState::Connecting(state), ConnectionEvent::ConnectionFailed { reason }) => {
                let new_state = ConnectionState::Failed(FailedState {
                    peer_id: state.peer_id,
                    transport: Some(state.transport),
                    failed_at: current_timestamp(),
                    error_reason: reason,
                    retry_after: Some(current_timestamp() + 30), // Retry after 30 seconds
                });
                (new_state, Vec::new())
            }

            (ConnectionState::Connecting(_), ConnectionEvent::Timeout) => {
                let new_state = ConnectionState::Failed(FailedState {
                    peer_id,
                    transport: None,
                    failed_at: current_timestamp(),
                    error_reason: "Connection timeout".to_string(),
                    retry_after: Some(current_timestamp() + 60),
                });
                (new_state, Vec::new())
            }

            // From Connected
            (ConnectionState::Connected(mut state), ConnectionEvent::ActivityDetected) => {
                state.last_activity = current_timestamp();
                state.message_count += 1;
                (ConnectionState::Connected(state), Vec::new())
            }

            (ConnectionState::Connected(state), ConnectionEvent::ConnectionLost { reason }) => {
                let new_state = ConnectionState::Failed(FailedState {
                    peer_id: state.peer_id,
                    transport: Some(state.transport),
                    failed_at: current_timestamp(),
                    error_reason: reason,
                    retry_after: Some(current_timestamp() + 10), // Quick retry for lost connections
                });
                (new_state, Vec::new())
            }

            (ConnectionState::Connected(_), ConnectionEvent::Disconnect) => {
                let new_state = ConnectionState::Disconnected(DisconnectedState {
                    peer_id,
                    last_seen: Some(current_timestamp()),
                    failed_attempts: 0,
                });
                (new_state, Vec::new())
            }

            // From Failed
            (ConnectionState::Failed(state), ConnectionEvent::Retry) => {
                let new_state = ConnectionState::Disconnected(DisconnectedState {
                    peer_id: state.peer_id,
                    last_seen: None,
                    failed_attempts: 0, // Reset attempt counter on manual retry
                });
                (new_state, Vec::new())
            }

            // Universal transitions
            (_, ConnectionEvent::Disconnect) => {
                let new_state = ConnectionState::Disconnected(DisconnectedState {
                    peer_id,
                    last_seen: Some(current_timestamp()),
                    failed_attempts: 0,
                });
                (new_state, Vec::new())
            }

            // Invalid transitions
            (_state, event) => {
                return Err(StateTransitionError::InvalidTransition {
                    from_state: from_state.clone(),
                    event: event_name.clone(),
                    reason: format!("Event {:?} not valid for state {}", event, from_state),
                });
            }
        };

        let to_state = new_state.state_name().to_string();
        let audit_entry = AuditEntry {
            timestamp: current_timestamp(),
            peer_id,
            from_state,
            to_state,
            event: event_name,
            effects_count: effects.len(),
        };

        Ok(StateTransition {
            new_state,
            effects,
            audit_entry,
        })
    }

    /// Check if state allows message sending
    pub fn can_send_messages(&self) -> bool {
        matches!(self, ConnectionState::Connected(_))
    }

    /// Check if state requires timeout handling
    pub fn has_timeout(&self) -> Option<Timestamp> {
        match self {
            ConnectionState::Discovering(s) => s.discovery_timeout,
            ConnectionState::Connecting(s) => Some(s.connection_timeout),
            _ => None,
        }
    }

    /// Get connection quality score (0-100)
    pub fn quality_score(&self) -> u8 {
        match self {
            ConnectionState::Connected(state) => {
                let age = current_timestamp() - state.last_activity;
                if age < 10 {
                    100
                } else if age < 60 {
                    80
                } else {
                    60
                }
            }
            ConnectionState::Connecting(_) => 30,
            ConnectionState::Discovering(_) => 20,
            ConnectionState::Failed(_) => 0,
            ConnectionState::Disconnected(_) => 0,
        }
    }
}

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors that can occur during state transitions
#[derive(Debug, Clone)]
pub enum StateTransitionError {
    /// Invalid state transition attempted
    InvalidTransition {
        from_state: String,
        event: String,
        reason: String,
    },
    /// State corruption detected
    StateCorruption { peer_id: PeerId, details: String },
}

impl fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateTransitionError::InvalidTransition {
                from_state,
                event,
                reason,
            } => {
                write!(
                    f,
                    "Invalid transition from {} on event {}: {}",
                    from_state, event, reason
                )
            }
            StateTransitionError::StateCorruption { peer_id, details } => {
                write!(f, "State corruption for peer {}: {}", peer_id, details)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StateTransitionError {}

// ----------------------------------------------------------------------------
// Utility Functions
// ----------------------------------------------------------------------------

/// Get current timestamp in milliseconds since UNIX epoch
fn current_timestamp() -> Timestamp {
    cfg_if::cfg_if! {
        if #[cfg(any(feature = "std", feature = "wasm"))] {
            Timestamp::now()
        } else {
            Timestamp::new(0) // Fallback when no time features enabled
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_initial_state() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        assert_eq!(state.peer_id(), peer_id);
        assert_eq!(state.state_name(), "Disconnected");
        assert!(!state.can_send_messages());
        assert_eq!(state.quality_score(), 0);
    }

    #[test]
    fn test_discovery_transition() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        let transition = state
            .transition(ConnectionEvent::StartDiscovery {
                timeout_seconds: Some(60),
            })
            .unwrap();

        assert_eq!(transition.new_state.state_name(), "Discovering");
        assert_eq!(transition.effects.len(), 2); // Start discovery for both transports
        assert_eq!(transition.audit_entry.from_state, "Disconnected");
        assert_eq!(transition.audit_entry.to_state, "Discovering");
    }

    #[test]
    fn test_connection_flow() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        // Start discovery
        let transition = state
            .transition(ConnectionEvent::StartDiscovery {
                timeout_seconds: None,
            })
            .unwrap();
        let state = transition.new_state;

        // Peer discovered
        let transition = state
            .transition(ConnectionEvent::PeerDiscovered {
                transport: ChannelTransportType::Ble,
                signal_strength: Some(-50),
            })
            .unwrap();
        let state = transition.new_state;

        // Initiate connection
        let transition = state
            .transition(ConnectionEvent::InitiateConnection {
                transport: ChannelTransportType::Ble,
                session_params: create_test_session_params(),
            })
            .unwrap();
        let state = transition.new_state;

        assert_eq!(state.state_name(), "Connecting");
        assert_eq!(transition.effects.len(), 1); // InitiateConnection effect

        // Connection established
        let transition = state
            .transition(ConnectionEvent::ConnectionEstablished {
                session_id: "test-session".to_string(),
            })
            .unwrap();
        let state = transition.new_state;

        assert_eq!(state.state_name(), "Connected");
        assert!(state.can_send_messages());
        assert!(state.quality_score() > 0);
    }

    #[test]
    fn test_invalid_transition() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        // Try to establish connection without discovering first
        let result = state.transition(ConnectionEvent::ConnectionEstablished {
            session_id: "test".to_string(),
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            StateTransitionError::InvalidTransition { from_state, .. } => {
                assert_eq!(from_state, "Disconnected");
            }
            _ => panic!("Expected InvalidTransition error"),
        }
    }

    #[test]
    fn test_connection_failure_and_retry() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        // Go through discovery to connecting
        let state = state
            .transition(ConnectionEvent::StartDiscovery {
                timeout_seconds: None,
            })
            .unwrap()
            .new_state;
        let state = state
            .transition(ConnectionEvent::InitiateConnection {
                transport: ChannelTransportType::Ble,
                session_params: create_test_session_params(),
            })
            .unwrap()
            .new_state;

        // Connection fails
        let transition = state
            .transition(ConnectionEvent::ConnectionFailed {
                reason: "Network error".to_string(),
            })
            .unwrap();
        let state = transition.new_state;

        assert_eq!(state.state_name(), "Failed");

        // Retry
        let transition = state.transition(ConnectionEvent::Retry).unwrap();
        let state = transition.new_state;

        assert_eq!(state.state_name(), "Disconnected");
    }

    #[test]
    fn test_universal_disconnect() {
        let peer_id = create_test_peer_id(1);
        let state = ConnectionState::new_disconnected(peer_id);

        // From any state, disconnect should work
        let state = state
            .transition(ConnectionEvent::StartDiscovery {
                timeout_seconds: None,
            })
            .unwrap()
            .new_state;
        let transition = state.transition(ConnectionEvent::Disconnect).unwrap();

        assert_eq!(transition.new_state.state_name(), "Disconnected");
    }
}
