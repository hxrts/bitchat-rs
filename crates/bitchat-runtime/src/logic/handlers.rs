//! Core Logic Command and Event Handlers
//!
//! Contains all the command and event handling logic for the Core Logic task.

use bitchat_core::{
    PeerId, BitchatResult,
    Effect, AppEvent, ConnectionStatus, ChannelTransportType,
    internal::{
        ConnectionState, ConnectionEvent, StateTransition,
        ContentAddressedMessage, MessageId
    }
};
use super::state::{CoreState, SystemTimeSource};
use bitchat_core::internal::TimeSource;

#[cfg(feature = "std")]
use tracing::{warn, error, debug};
#[cfg(not(feature = "std"))]
use log::{info, warn, error, debug};

/// Command and event handlers for the Core Logic task
pub struct CommandHandlers;

impl CommandHandlers {
    /// Handle send message command
    pub async fn handle_send_message(
        state: &mut CoreState,
        recipient: PeerId,
        content: String,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        // Check if we can send to this peer
        let connection = state.connections.get(&recipient);
        if let Some(conn) = connection {
            if !conn.can_send_messages() {
                return Ok((
                    Vec::new(),
                    vec![AppEvent::SystemError {
                        error: format!("Cannot send to peer {} - not connected", recipient),
                    }],
                ));
            }
        } else {
            return Ok((
                Vec::new(),
                vec![AppEvent::SystemError {
                    error: format!("Peer {} not found", recipient),
                }],
            ));
        }

        // Create content-addressed message with proper protocol layering
        // Note: In a full implementation, this would go through Noise Protocol Framework
        // and proper packet fragmentation instead of raw content transmission
        state.message_sequence += 1;
        let message = ContentAddressedMessage::new(
            state.peer_id,
            Some(recipient),
            content.clone(),
            state.message_sequence,
        );

        // Store message
        state.message_store.store_message(message.clone())?;
        state.stats.messages_sent += 1;

        // TODO: Replace with proper Noise Protocol framing and fragmentation
        // This simplified approach violates the protocol layering promise
        let packet_data = format!("{}:{}", state.peer_id, content).into_bytes();

        // Determine transport from connection state
        let transport = if let Some(ConnectionState::Connected(conn)) = state.connections.get(&recipient) {
            conn.transport
        } else {
            ChannelTransportType::Ble // Default fallback
        };

        let effects = vec![Effect::SendPacket {
            peer_id: recipient,
            data: packet_data,
            transport,
        }];

        let app_events = vec![AppEvent::MessageSent {
            to: recipient,
            content,
            timestamp: message.timestamp,
        }];

        Ok((effects, app_events))
    }

    /// Handle connect to peer command
    pub async fn handle_connect_to_peer(
        state: &mut CoreState,
        peer_id: PeerId,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        // Get or create connection state
        let connection = state.connections
            .remove(&peer_id)
            .unwrap_or_else(|| ConnectionState::new_disconnected(peer_id));

        // Start discovery
        match connection.transition(ConnectionEvent::StartDiscovery { timeout_seconds: Some(60) }) {
            Ok(transition) => {
                Self::apply_state_transition(state, transition).await;
                Ok((Vec::new(), Vec::new())) // Effects handled by transition
            }
            Err(e) => {
                error!("Failed to start discovery for peer {}: {}", peer_id, e);
                Ok((Vec::new(), vec![AppEvent::SystemError {
                    error: format!("Failed to connect to peer: {}", e),
                }]))
            }
        }
    }

    /// Handle start discovery command
    pub async fn handle_start_discovery() -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        let effects = vec![
            Effect::StartTransportDiscovery { transport: ChannelTransportType::Ble },
            Effect::StartTransportDiscovery { transport: ChannelTransportType::Nostr },
        ];

        let app_events = vec![AppEvent::DiscoveryStateChanged {
            active: true,
            transport: None,
        }];

        Ok((effects, app_events))
    }

    /// Handle stop discovery command
    pub async fn handle_stop_discovery() -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        let effects = vec![
            Effect::StopTransportDiscovery { transport: ChannelTransportType::Ble },
            Effect::StopTransportDiscovery { transport: ChannelTransportType::Nostr },
        ];

        let app_events = vec![AppEvent::DiscoveryStateChanged {
            active: false,
            transport: None,
        }];

        Ok((effects, app_events))
    }

    /// Handle disconnect from peer command
    pub async fn handle_disconnect_from_peer(
        state: &mut CoreState,
        peer_id: PeerId,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        if let Some(connection) = state.connections.remove(&peer_id) {
            match connection.transition(ConnectionEvent::Disconnect) {
                Ok(transition) => {
                    Self::apply_state_transition(state, transition).await;
                }
                Err(e) => {
                    error!("Error disconnecting from peer {}: {}", peer_id, e);
                }
            }
        }

        let app_events = vec![AppEvent::PeerStatusChanged {
            peer_id,
            status: ConnectionStatus::Disconnected,
            transport: None,
        }];

        Ok((Vec::new(), app_events))
    }

    /// Handle shutdown command
    pub async fn handle_shutdown() -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        let effects = vec![
            Effect::StopListening { transport: ChannelTransportType::Ble },
            Effect::StopListening { transport: ChannelTransportType::Nostr },
        ];

        Ok((effects, Vec::new()))
    }

    /// Handle peer discovered event
    pub async fn handle_peer_discovered(
        state: &mut CoreState,
        peer_id: PeerId,
        transport: ChannelTransportType,
        signal_strength: Option<i8>,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        // Get or create connection state
        let connection = state.connections
            .remove(&peer_id)
            .unwrap_or_else(|| ConnectionState::new_disconnected(peer_id));

        // Process peer discovery
        match connection.transition(ConnectionEvent::PeerDiscovered { transport, signal_strength }) {
            Ok(transition) => {
                Self::apply_state_transition(state, transition).await;
            }
            Err(e) => {
                debug!("Peer discovery transition failed: {}", e);
                // Create new disconnected state since transition failed
                let new_connection = ConnectionState::new_disconnected(peer_id);
                state.connections.insert(peer_id, new_connection);
            }
        }

        let app_events = vec![AppEvent::PeerStatusChanged {
            peer_id,
            status: ConnectionStatus::Discovering,
            transport: Some(transport),
        }];

        Ok((Vec::new(), app_events))
    }

    /// Handle message received event
    pub async fn handle_message_received(
        state: &mut CoreState,
        from: PeerId,
        content: String,
        _transport: ChannelTransportType,
        message_id: Option<MessageId>,
        recipient: Option<PeerId>,
        timestamp: Option<u64>,
        sequence: Option<u64>,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        // Update connection activity
        if let Some(connection) = state.connections.remove(&from) {
            match connection.transition(ConnectionEvent::ActivityDetected) {
                Ok(transition) => {
                    Self::apply_state_transition(state, transition).await;
                }
                Err(e) => {
                    debug!("Activity transition failed: {}", e);
                    // Create new disconnected state since transition failed
                    let new_connection = ConnectionState::new_disconnected(from);
                    state.connections.insert(from, new_connection);
                }
            }
        }

        let resolved_timestamp = timestamp.unwrap_or_else(|| SystemTimeSource.now().as_millis());
        let resolved_recipient = recipient.unwrap_or(state.peer_id);
        let resolved_sequence = match sequence {
            Some(seq) => seq,
            None => {
                state.message_sequence = state.message_sequence.wrapping_add(1);
                state.message_sequence
            }
        };

        let message = ContentAddressedMessage::from_metadata(
            from,
            Some(resolved_recipient),
            content.clone(),
            resolved_sequence,
            resolved_timestamp,
            message_id,
        )?;

        state.message_store.store_message(message.clone())?;
        state.stats.messages_received += 1;

        let app_events = vec![AppEvent::MessageReceived {
            from,
            content,
            timestamp: message.timestamp,
        }];

        Ok((Vec::new(), app_events))
    }

    /// Handle connection established event
    pub async fn handle_connection_established(
        state: &mut CoreState,
        peer_id: PeerId,
        transport: ChannelTransportType,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        if let Some(connection) = state.connections.remove(&peer_id) {
            match connection.transition(ConnectionEvent::ConnectionEstablished {
                session_id: format!("session-{}-{}", peer_id, transport),
            }) {
                Ok(transition) => {
                    Self::apply_state_transition(state, transition).await;
                }
                Err(e) => {
                    error!("Connection established transition failed: {}", e);
                    // Create new disconnected state since transition failed
                    let new_connection = ConnectionState::new_disconnected(peer_id);
                    state.connections.insert(peer_id, new_connection);
                }
            }
        }

        let app_events = vec![AppEvent::PeerStatusChanged {
            peer_id,
            status: ConnectionStatus::Connected,
            transport: Some(transport),
        }];

        Ok((Vec::new(), app_events))
    }

    /// Handle connection lost event
    pub async fn handle_connection_lost(
        state: &mut CoreState,
        peer_id: PeerId,
        transport: ChannelTransportType,
        reason: String,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        if let Some(connection) = state.connections.remove(&peer_id) {
            match connection.transition(ConnectionEvent::ConnectionLost { reason: reason.clone() }) {
                Ok(transition) => {
                    Self::apply_state_transition(state, transition).await;
                }
                Err(e) => {
                    error!("Connection lost transition failed: {}", e);
                    // Create new disconnected state since transition failed
                    let new_connection = ConnectionState::new_disconnected(peer_id);
                    state.connections.insert(peer_id, new_connection);
                }
            }
        }

        let app_events = vec![AppEvent::PeerStatusChanged {
            peer_id,
            status: ConnectionStatus::Error,
            transport: Some(transport),
        }];

        Ok((Vec::new(), app_events))
    }

    /// Handle transport error event
    pub async fn handle_transport_error(
        transport: ChannelTransportType,
        error: String,
    ) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        warn!("Transport {} error: {}", transport, error);

        let app_events = vec![AppEvent::SystemError { error }];

        Ok((Vec::new(), app_events))
    }

    /// Apply a state transition and update internal state
    pub async fn apply_state_transition(state: &mut CoreState, transition: StateTransition) {
        let peer_id = transition.new_state.peer_id();
        
        // Update connection state
        state.connections.insert(peer_id, transition.new_state);
        
        // Record audit entry
        state.audit_trail.push(transition.audit_entry);
        state.stats.state_transitions += 1;
        
        // Effects are returned to be handled by the caller
    }
}
