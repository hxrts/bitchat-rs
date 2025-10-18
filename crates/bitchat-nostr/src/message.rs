//! BitChat message format for Nostr events

use bitchat_core::{BitchatPacket, PeerId};
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
    /// Serialized BitChat packet (base64 encoded)
    pub packet_data: String,
    /// Message timestamp
    pub timestamp: u64,
}

impl BitchatNostrMessage {
    /// Create a new BitChat Nostr message
    pub fn new(
        sender_peer_id: PeerId,
        recipient_peer_id: Option<PeerId>,
        packet: &BitchatPacket,
    ) -> Result<Self, NostrTransportError> {
        let packet_bytes = bincode::serialize(packet)?;
        let packet_data = general_purpose::STANDARD.encode(packet_bytes);

        Ok(Self {
            sender_peer_id,
            recipient_peer_id,
            packet_data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    /// Extract BitChat packet from Nostr message
    pub fn to_packet(&self) -> Result<BitchatPacket, NostrTransportError> {
        let packet_bytes = general_purpose::STANDARD
            .decode(&self.packet_data)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;
        Ok(bincode::deserialize(&packet_bytes)?)
    }
}
