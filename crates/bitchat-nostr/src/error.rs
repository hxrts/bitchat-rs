//! Error types for Nostr transport

use bitchat_core::{internal::TransportError, BitchatError, PeerId};
use thiserror::Error;

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors specific to the Nostr transport
#[derive(Error, Debug)]
pub enum NostrTransportError {
    #[error("Failed to connect to relay: {relay} - {source}")]
    RelayConnectionFailed {
        relay: String,
        #[source]
        source: nostr_sdk::client::Error,
    },

    #[error("Failed to send event: {0}")]
    EventSendFailed(#[from] nostr_sdk::client::Error),

    #[error("Failed to serialize message: {0}")]
    SerializationFailed(#[from] bincode::Error),

    #[error("Key operation failed: {0}")]
    KeyOperationFailed(String),

    #[error("Failed to deserialize message: {0}")]
    DeserializationFailed(String),

    #[error("Invalid relay URL: {url}")]
    InvalidRelayUrl { url: String },

    #[error("Client not initialized")]
    ClientNotInitialized,

    #[error("Message too large: {size} bytes (max: {max_size})")]
    MessageTooLarge { size: usize, max_size: usize },

    #[error("Failed to create encrypted message: {0}")]
    EncryptionFailed(String),

    #[error("Receive channel closed")]
    ReceiveChannelClosed,

    #[error("Unknown peer: {peer_id}")]
    UnknownPeer { peer_id: PeerId },
}

#[cfg(not(target_arch = "wasm32"))]
impl From<nostr_sdk::key::Error> for NostrTransportError {
    fn from(err: nostr_sdk::key::Error) -> Self {
        NostrTransportError::KeyOperationFailed(err.to_string())
    }
}

impl From<NostrTransportError> for BitchatError {
    fn from(err: NostrTransportError) -> Self {
        BitchatError::Transport(TransportError::ReceiveFailed {
            reason: err.to_string(),
        })
    }
}
