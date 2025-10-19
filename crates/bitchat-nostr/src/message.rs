//! BitChat message format for Nostr events

use bitchat_core::PeerId;
use nostr_sdk::base64::{engine::general_purpose, Engine as _};
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::NostrTransportError;

// ----------------------------------------------------------------------------
// BitChat Nostr Event Format
// ----------------------------------------------------------------------------

/// Custom Nostr event kind for BitChat protocol
pub const BITCHAT_KIND: Kind = Kind::Custom(30420);

/// BitChat message wrapper for Nostr events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatNostrMessage {
    /// BitChat peer ID of sender
    pub sender_peer_id: PeerId,
    /// BitChat peer ID of recipient (None for broadcast)
    pub recipient_peer_id: Option<PeerId>,
    /// Raw message data (base64 encoded)
    pub data: String,
    /// Message timestamp
    pub timestamp: u64,
}

impl BitchatNostrMessage {
    /// Create a new BitChat Nostr message from raw data
    pub fn new(sender_peer_id: PeerId, recipient_peer_id: Option<PeerId>, data: Vec<u8>) -> Self {
        let data_base64 = general_purpose::STANDARD.encode(data);

        Self {
            sender_peer_id,
            recipient_peer_id,
            data: data_base64,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Extract raw data from Nostr message
    pub fn to_data(&self) -> Result<Vec<u8>, NostrTransportError> {
        general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))
    }

    /// Serialize message to bincode + base64 for Nostr event content
    pub fn to_nostr_content(&self) -> Result<String, NostrTransportError> {
        let message_bytes = bincode::serialize(self)?;
        Ok(general_purpose::STANDARD.encode(message_bytes))
    }

    /// Deserialize message from bincode + base64 Nostr event content
    pub fn from_nostr_content(content: &str) -> Result<Self, NostrTransportError> {
        let message_bytes = general_purpose::STANDARD
            .decode(content)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;
        Ok(bincode::deserialize(&message_bytes)?)
    }

    /// Check if this message is intended for a specific peer
    pub fn is_for_peer(&self, peer_id: &PeerId) -> bool {
        match &self.recipient_peer_id {
            Some(recipient) => recipient == peer_id,
            None => true, // Broadcast message
        }
    }

    /// Check if this is a broadcast message
    pub fn is_broadcast(&self) -> bool {
        self.recipient_peer_id.is_none()
    }
}
