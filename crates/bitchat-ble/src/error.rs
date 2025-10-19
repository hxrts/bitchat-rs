//! Error types for BLE transport

use bitchat_core::internal::TransportError;
use bitchat_core::BitchatError;
use thiserror::Error;

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors specific to the BLE transport
#[derive(Error, Debug)]
pub enum BleTransportError {
    #[error("Failed to connect to peer: {0}")]
    ConnectionFailed(String),

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Connection already in progress")]
    ConnectionInProgress,

    #[error("Too many failed connection attempts")]
    TooManyRetries,

    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },

    #[error("Peer not connected")]
    PeerNotConnected,

    #[error("Peer not discovered")]
    PeerNotDiscovered,

    #[error("Failed to discover services: {0}")]
    ServiceDiscoveryFailed(String),

    #[error("Characteristic not found: {characteristic}")]
    CharacteristicNotFound { characteristic: String },

    #[error("Failed to subscribe to notifications: {0}")]
    SubscriptionFailed(String),

    #[error("Failed to write to characteristic: {0}")]
    WriteFailed(String),

    #[error("Failed to get BLE events: {0}")]
    EventStreamFailed(String),

    #[error("Failed to get notifications stream: {0}")]
    NotificationStreamFailed(String),

    #[error("Packet too large: {size} bytes (max: {max_size})")]
    PacketTooLarge { size: usize, max_size: usize },

    #[error("Receive channel closed")]
    ReceiveChannelClosed,

    #[error("BLE adapter not available")]
    AdapterNotAvailable,

    #[error("Transport error: {0}")]
    Transport(String),
}

impl From<BleTransportError> for BitchatError {
    fn from(err: BleTransportError) -> Self {
        BitchatError::Transport(TransportError::InvalidConfiguration {
            reason: err.to_string(),
        })
    }
}
