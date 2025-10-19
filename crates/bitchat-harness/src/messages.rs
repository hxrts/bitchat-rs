//! Canonical message schema and normalization helpers.
//!
//! This module defines the canonical types for inter-task communication and
//! provides normalization helpers for transports to convert raw payloads into
//! the schema.

extern crate alloc;
use alloc::{string::String, vec::Vec};

pub use bitchat_core::channel::{
    AppEvent, ChannelTransportType, Command, ConnectionStatus, Effect, Event, TransportStatus,
};

use bitchat_core::internal::{MessageId, PacketError};
use bitchat_core::{BitchatError, BitchatResult, PeerId};

// ----------------------------------------------------------------------------
// Transport Message Normalization
// ----------------------------------------------------------------------------

/// Raw inbound message from a transport before normalization
#[derive(Debug, Clone)]
pub struct RawInboundMessage {
    pub sender: Option<PeerId>,
    pub content: Vec<u8>,
    pub transport: ChannelTransportType,
    pub metadata: TransportMetadata,
}

/// Raw outbound message to be sent via transport
#[derive(Debug, Clone)]
pub struct RawOutboundMessage {
    pub recipient: PeerId,
    pub content: Vec<u8>,
    pub transport: ChannelTransportType,
    pub metadata: TransportMetadata,
}

/// Transport-specific metadata
#[derive(Debug, Clone, Default)]
pub struct TransportMetadata {
    pub message_id: Option<String>,
    pub timestamp: Option<u64>,
    pub sequence: Option<u64>,
    pub signal_strength: Option<i8>,
    pub retry_count: Option<u32>,
    /// Transport-specific hash for deduplication
    pub transport_hash: Option<Vec<u8>>,
}

/// Normalized inbound message
#[derive(Debug)]
pub struct InboundMessage {
    pub event: Event,
    /// Canonical hash for deduplication across transports
    pub canonical_hash: Vec<u8>,
}

/// Normalized outbound message  
#[derive(Debug)]
pub struct OutboundMessage {
    pub effect: Effect,
    /// Canonical hash for tracking
    pub canonical_hash: Vec<u8>,
}

impl InboundMessage {
    /// Convert raw transport message to canonical Event
    pub fn from_transport(raw: RawInboundMessage) -> BitchatResult<Self> {
        let sender = raw
            .sender
            .ok_or(BitchatError::InvalidPacket(PacketError::InvalidSenderId))?;

        // Create canonical hash from content + sender + transport
        let canonical_hash = Self::compute_canonical_hash(&raw.content, &sender, &raw.transport);

        // Convert raw content to string (assuming UTF-8 for now)
        let content = String::from_utf8(raw.content).map_err(|_| {
            BitchatError::InvalidPacket(PacketError::Generic {
                message: "Invalid UTF-8 in message content".to_string(),
            })
        })?;

        let event = Event::MessageReceived {
            from: sender,
            content,
            transport: raw.transport,
            message_id: raw.metadata.message_id.and_then(|id| {
                // Try to parse message ID from hex string
                MessageId::from_hex(&id).ok()
            }),
            recipient: None, // Will be filled by core logic
            timestamp: raw.metadata.timestamp,
            sequence: raw.metadata.sequence,
        };

        Ok(Self {
            event,
            canonical_hash,
        })
    }

    /// Compute canonical hash for deduplication
    fn compute_canonical_hash(
        content: &[u8],
        sender: &PeerId,
        transport: &ChannelTransportType,
    ) -> Vec<u8> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.update(sender.as_bytes());
        hasher.update([*transport as u8]);
        hasher.finalize().to_vec()
    }
}

impl OutboundMessage {
    /// Convert Effect to raw transport message
    pub fn to_transport(effect: Effect) -> BitchatResult<Self> {
        let canonical_hash = match &effect {
            Effect::SendPacket {
                peer_id,
                data,
                transport,
            } => Self::compute_canonical_hash(data, peer_id, transport),
            _ => {
                return Err(BitchatError::InvalidPacket(PacketError::Generic {
                    message: "Effect type not convertible to transport message".to_string(),
                }));
            }
        };

        Ok(Self {
            effect,
            canonical_hash,
        })
    }

    /// Compute canonical hash for tracking
    fn compute_canonical_hash(
        content: &[u8],
        recipient: &PeerId,
        transport: &ChannelTransportType,
    ) -> Vec<u8> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.update(recipient.as_bytes());
        hasher.update([*transport as u8]);
        hasher.finalize().to_vec()
    }

    /// Extract raw message data for transport
    pub fn extract_raw(&self) -> BitchatResult<RawOutboundMessage> {
        match &self.effect {
            Effect::SendPacket {
                peer_id,
                data,
                transport,
            } => Ok(RawOutboundMessage {
                recipient: *peer_id,
                content: data.clone(),
                transport: *transport,
                metadata: TransportMetadata::default(),
            }),
            _ => Err(BitchatError::InvalidPacket(PacketError::Generic {
                message: "Effect not convertible to raw outbound message".to_string(),
            })),
        }
    }
}

/// Validation helpers
impl RawInboundMessage {
    /// Validate that required metadata is present
    pub fn validate(&self) -> BitchatResult<()> {
        if self.sender.is_none() {
            return Err(BitchatError::InvalidPacket(PacketError::InvalidSenderId));
        }

        if self.content.is_empty() {
            return Err(BitchatError::InvalidPacket(PacketError::Generic {
                message: "Empty content in raw inbound message".to_string(),
            }));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id() -> PeerId {
        PeerId::new([1, 2, 3, 4, 5, 6, 7, 8])
    }

    fn create_test_raw_inbound() -> RawInboundMessage {
        RawInboundMessage {
            sender: Some(create_test_peer_id()),
            content: b"Hello, World!".to_vec(),
            transport: ChannelTransportType::Ble,
            metadata: TransportMetadata {
                message_id: Some("deadbeef".to_string()),
                timestamp: Some(1234567890),
                sequence: Some(42),
                signal_strength: Some(-60),
                retry_count: Some(0),
                transport_hash: Some(vec![0xde, 0xad, 0xbe, 0xef]),
            },
        }
    }

    #[test]
    fn test_inbound_message_from_transport_success() {
        let raw = create_test_raw_inbound();
        let result = InboundMessage::from_transport(raw);

        assert!(result.is_ok());
        let inbound = result.unwrap();

        match inbound.event {
            Event::MessageReceived {
                from,
                content,
                transport,
                ..
            } => {
                assert_eq!(from, create_test_peer_id());
                assert_eq!(content, "Hello, World!");
                assert_eq!(transport, ChannelTransportType::Ble);
            }
            _ => panic!("Expected MessageReceived event"),
        }

        // Canonical hash should be deterministic
        assert!(!inbound.canonical_hash.is_empty());
        assert_eq!(inbound.canonical_hash.len(), 32); // SHA256 output
    }

    #[test]
    fn test_inbound_message_missing_sender() {
        let mut raw = create_test_raw_inbound();
        raw.sender = None;

        let result = InboundMessage::from_transport(raw);
        assert!(result.is_err());

        match result.unwrap_err() {
            BitchatError::InvalidPacket(PacketError::InvalidSenderId) => (),
            e => panic!("Expected InvalidSenderId error, got: {:?}", e),
        }
    }

    #[test]
    fn test_inbound_message_invalid_utf8() {
        let mut raw = create_test_raw_inbound();
        raw.content = vec![0xFF, 0xFE, 0xFD]; // Invalid UTF-8

        let result = InboundMessage::from_transport(raw);
        assert!(result.is_err());

        match result.unwrap_err() {
            BitchatError::InvalidPacket(PacketError::Generic { message }) => {
                assert!(message.contains("Invalid UTF-8"));
            }
            e => panic!("Expected Generic PacketError, got: {:?}", e),
        }
    }

    #[test]
    fn test_canonical_hash_deterministic() {
        let raw1 = create_test_raw_inbound();
        let raw2 = create_test_raw_inbound();

        let inbound1 = InboundMessage::from_transport(raw1).unwrap();
        let inbound2 = InboundMessage::from_transport(raw2).unwrap();

        assert_eq!(inbound1.canonical_hash, inbound2.canonical_hash);
    }

    #[test]
    fn test_canonical_hash_different_for_different_content() {
        let raw1 = create_test_raw_inbound();
        let mut raw2 = create_test_raw_inbound();
        raw2.content = b"Different content".to_vec();

        let inbound1 = InboundMessage::from_transport(raw1).unwrap();
        let inbound2 = InboundMessage::from_transport(raw2).unwrap();

        assert_ne!(inbound1.canonical_hash, inbound2.canonical_hash);
    }

    #[test]
    fn test_outbound_message_to_transport() {
        let peer_id = create_test_peer_id();
        let data = b"Test message".to_vec();
        let transport = ChannelTransportType::Nostr;

        let effect = Effect::SendPacket {
            peer_id,
            data: data.clone(),
            transport,
        };

        let result = OutboundMessage::to_transport(effect);
        assert!(result.is_ok());

        let outbound = result.unwrap();
        assert!(!outbound.canonical_hash.is_empty());
        assert_eq!(outbound.canonical_hash.len(), 32);

        // Test extraction
        let raw = outbound.extract_raw().unwrap();
        assert_eq!(raw.recipient, peer_id);
        assert_eq!(raw.content, data);
        assert_eq!(raw.transport, transport);
    }

    #[test]
    fn test_outbound_message_unsupported_effect() {
        let effect = Effect::StartListening {
            transport: ChannelTransportType::Ble,
        };

        let result = OutboundMessage::to_transport(effect);
        assert!(result.is_err());

        match result.unwrap_err() {
            BitchatError::InvalidPacket(PacketError::Generic { message }) => {
                assert!(message.contains("not convertible"));
            }
            e => panic!("Expected Generic PacketError, got: {:?}", e),
        }
    }

    #[test]
    fn test_raw_inbound_validation_success() {
        let raw = create_test_raw_inbound();
        assert!(raw.validate().is_ok());
    }

    #[test]
    fn test_raw_inbound_validation_missing_sender() {
        let mut raw = create_test_raw_inbound();
        raw.sender = None;

        let result = raw.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            BitchatError::InvalidPacket(PacketError::InvalidSenderId) => (),
            e => panic!("Expected InvalidSenderId, got: {:?}", e),
        }
    }

    #[test]
    fn test_raw_inbound_validation_empty_content() {
        let mut raw = create_test_raw_inbound();
        raw.content = Vec::new();

        let result = raw.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            BitchatError::InvalidPacket(PacketError::Generic { message }) => {
                assert!(message.contains("Empty content"));
            }
            e => panic!("Expected Generic PacketError, got: {:?}", e),
        }
    }

    #[test]
    fn test_message_id_parsing() {
        let mut raw = create_test_raw_inbound();
        // Use a valid 32-byte hex string for MessageId
        raw.metadata.message_id =
            Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string());

        let inbound = InboundMessage::from_transport(raw).unwrap();

        match inbound.event {
            Event::MessageReceived {
                message_id: Some(_),
                ..
            } => (),
            Event::MessageReceived {
                message_id: None, ..
            } => panic!("Expected valid message ID"),
            _ => panic!("Expected MessageReceived event"),
        }
    }

    #[test]
    fn test_message_id_parsing_invalid() {
        let mut raw = create_test_raw_inbound();
        raw.metadata.message_id = Some("invalid_hex".to_string());

        let inbound = InboundMessage::from_transport(raw).unwrap();

        match inbound.event {
            Event::MessageReceived {
                message_id: None, ..
            } => (),
            Event::MessageReceived {
                message_id: Some(_),
                ..
            } => panic!("Expected None for invalid message ID"),
            _ => panic!("Expected MessageReceived event"),
        }
    }
}
