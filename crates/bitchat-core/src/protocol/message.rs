//! Application layer message structures
//!
//! This module defines the BitchatMessage format and Noise payload types
//! for application-level content within BitChat packets.

use alloc::{string::String, vec::Vec};
use core::convert::TryInto;
use serde::{Deserialize, Serialize};

use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Noise Payload Types (Encrypted Layer)
// ----------------------------------------------------------------------------

/// Message types for encrypted payloads within Noise transport
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NoisePayloadType {
    /// Private chat message content
    PrivateMessage = 0x01,
    /// Message read confirmation
    ReadReceipt = 0x02,
    /// Message delivery confirmation
    Delivered = 0x03,
    /// QR verification challenge
    VerifyChallenge = 0x10,
    /// QR verification response
    VerifyResponse = 0x11,
    /// File transfer initiation
    FileOffer = 0x20,
    /// File transfer acceptance
    FileAccept = 0x21,
    /// File transfer chunk
    FileChunk = 0x22,
    /// File transfer completion
    FileComplete = 0x23,
    /// Group creation
    GroupCreate = 0x30,
    /// Group member invite
    GroupInvite = 0x31,
    /// Group member join
    GroupJoin = 0x32,
    /// Group member leave
    GroupLeave = 0x33,
    /// Group message
    GroupMessage = 0x34,
    /// Group metadata update
    GroupUpdate = 0x35,
    /// Group member kick/remove
    GroupKick = 0x36,
    /// Device announcement for multi-device sync
    DeviceAnnouncement = 0x40,
    /// Session synchronization request
    SessionSyncRequest = 0x41,
    /// Session synchronization response
    SessionSyncResponse = 0x42,
    /// Device heartbeat
    DeviceHeartbeat = 0x43,
    /// Version hello with capability announcement
    VersionHello = 0x50,
    /// Version acknowledgment with negotiated capabilities
    VersionAck = 0x51,
    /// Capability negotiation rejection
    CapabilityRejection = 0x52,
}

impl NoisePayloadType {
    /// Convert from raw byte value
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            // Core message types (canonical implementation)
            0x01 => Ok(NoisePayloadType::PrivateMessage),
            0x02 => Ok(NoisePayloadType::ReadReceipt),
            0x03 => Ok(NoisePayloadType::Delivered),
            0x10 => Ok(NoisePayloadType::VerifyChallenge),
            0x11 => Ok(NoisePayloadType::VerifyResponse),
            
            // Experimental message types (conditionally available)
            #[cfg(feature = "experimental")]
            0x20 => Ok(NoisePayloadType::FileOffer),
            #[cfg(feature = "experimental")]
            0x21 => Ok(NoisePayloadType::FileAccept),
            #[cfg(feature = "experimental")]
            0x22 => Ok(NoisePayloadType::FileChunk),
            #[cfg(feature = "experimental")]
            0x23 => Ok(NoisePayloadType::FileComplete),
            #[cfg(feature = "experimental")]
            0x30 => Ok(NoisePayloadType::GroupCreate),
            #[cfg(feature = "experimental")]
            0x31 => Ok(NoisePayloadType::GroupInvite),
            #[cfg(feature = "experimental")]
            0x32 => Ok(NoisePayloadType::GroupJoin),
            #[cfg(feature = "experimental")]
            0x33 => Ok(NoisePayloadType::GroupLeave),
            #[cfg(feature = "experimental")]
            0x34 => Ok(NoisePayloadType::GroupMessage),
            #[cfg(feature = "experimental")]
            0x35 => Ok(NoisePayloadType::GroupUpdate),
            #[cfg(feature = "experimental")]
            0x36 => Ok(NoisePayloadType::GroupKick),
            #[cfg(feature = "experimental")]
            0x40 => Ok(NoisePayloadType::DeviceAnnouncement),
            #[cfg(feature = "experimental")]
            0x41 => Ok(NoisePayloadType::SessionSyncRequest),
            #[cfg(feature = "experimental")]
            0x42 => Ok(NoisePayloadType::SessionSyncResponse),
            #[cfg(feature = "experimental")]
            0x43 => Ok(NoisePayloadType::DeviceHeartbeat),
            #[cfg(feature = "experimental")]
            0x50 => Ok(NoisePayloadType::VersionHello),
            #[cfg(feature = "experimental")]
            0x51 => Ok(NoisePayloadType::VersionAck),
            #[cfg(feature = "experimental")]
            0x52 => Ok(NoisePayloadType::CapabilityRejection),
            
            _ => Err(BitchatError::invalid_packet("Unknown noise payload type")),
        }
    }

    /// Check if a message type is supported in current feature configuration
    pub fn is_supported(value: u8) -> bool {
        match value {
            // Core types always supported
            0x01..=0x03 | 0x10..=0x11 => true,
            
            // Experimental types conditionally supported
            #[cfg(feature = "experimental")]
            0x20..=0x23 | 0x30..=0x36 | 0x40..=0x43 | 0x50..=0x52 => true,
            #[cfg(not(feature = "experimental"))]
            0x20..=0x23 | 0x30..=0x36 | 0x40..=0x43 | 0x50..=0x52 => false,
            
            _ => false,
        }
    }

    /// Convert to raw byte value
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// ----------------------------------------------------------------------------
// Message Flags
// ----------------------------------------------------------------------------

/// Flags for optional BitchatMessage fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageFlags(u8);

impl MessageFlags {
    /// No optional fields present
    pub const NONE: Self = Self(0x00);

    /// Message is a relay (forwarded message)
    pub const IS_RELAY: Self = Self(0x01);

    /// Message is private (deprecated, use packet-level recipient)
    pub const IS_PRIVATE: Self = Self(0x02);

    /// Original sender field is present
    pub const HAS_ORIGINAL_SENDER: Self = Self(0x04);

    /// Recipient nickname field is present
    pub const HAS_RECIPIENT_NICKNAME: Self = Self(0x08);

    /// Sender peer ID field is present
    pub const HAS_SENDER_PEER_ID: Self = Self(0x10);

    /// Mentions array is present
    pub const HAS_MENTIONS: Self = Self(0x20);

    /// Create flags from raw byte
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Get raw byte value
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Check if message is a relay
    pub const fn is_relay(self) -> bool {
        (self.0 & Self::IS_RELAY.0) != 0
    }

    /// Check if message is private (deprecated)
    pub const fn is_private(self) -> bool {
        (self.0 & Self::IS_PRIVATE.0) != 0
    }

    /// Check if original sender is present
    pub const fn has_original_sender(self) -> bool {
        (self.0 & Self::HAS_ORIGINAL_SENDER.0) != 0
    }

    /// Check if recipient nickname is present
    pub const fn has_recipient_nickname(self) -> bool {
        (self.0 & Self::HAS_RECIPIENT_NICKNAME.0) != 0
    }

    /// Check if sender peer ID is present
    pub const fn has_sender_peer_id(self) -> bool {
        (self.0 & Self::HAS_SENDER_PEER_ID.0) != 0
    }

    /// Check if mentions are present
    pub const fn has_mentions(self) -> bool {
        (self.0 & Self::HAS_MENTIONS.0) != 0
    }

    /// Set relay flag
    pub fn with_relay(mut self) -> Self {
        self.0 |= Self::IS_RELAY.0;
        self
    }

    /// Set original sender flag
    pub fn with_original_sender(mut self) -> Self {
        self.0 |= Self::HAS_ORIGINAL_SENDER.0;
        self
    }

    /// Set recipient nickname flag
    pub fn with_recipient_nickname(mut self) -> Self {
        self.0 |= Self::HAS_RECIPIENT_NICKNAME.0;
        self
    }

    /// Set sender peer ID flag
    pub fn with_sender_peer_id(mut self) -> Self {
        self.0 |= Self::HAS_SENDER_PEER_ID.0;
        self
    }

    /// Set mentions flag
    pub fn with_mentions(mut self) -> Self {
        self.0 |= Self::HAS_MENTIONS.0;
        self
    }
}

// ----------------------------------------------------------------------------
// BitChat Message
// ----------------------------------------------------------------------------

/// Application layer message content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitchatMessage {
    /// Message flags
    pub flags: MessageFlags,
    /// Message creation timestamp
    pub timestamp: Timestamp,
    /// Unique message identifier
    pub id: String,
    /// Sender's display nickname
    pub sender: String,
    /// Message content
    pub content: String,
    /// Original sender (for relayed messages)
    pub original_sender: Option<String>,
    /// Recipient nickname (for private messages)
    pub recipient_nickname: Option<String>,
    /// Sender's peer ID
    pub sender_peer_id: Option<PeerId>,
    /// List of mentioned users
    pub mentions: Option<Vec<String>>,
}

impl BitchatMessage {
    /// Create a new message
    pub fn new(id: String, sender: String, content: String) -> Self {
        Self {
            flags: MessageFlags::NONE,
            timestamp: Timestamp::now(),
            id,
            sender,
            content,
            original_sender: None,
            recipient_nickname: None,
            sender_peer_id: None,
            mentions: None,
        }
    }

    /// Set as relay message with original sender
    pub fn with_relay(mut self, original_sender: String) -> Self {
        self.flags = self.flags.with_relay().with_original_sender();
        self.original_sender = Some(original_sender);
        self
    }

    /// Set recipient nickname
    pub fn with_recipient_nickname(mut self, nickname: String) -> Self {
        self.flags = self.flags.with_recipient_nickname();
        self.recipient_nickname = Some(nickname);
        self
    }

    /// Set sender peer ID
    pub fn with_sender_peer_id(mut self, peer_id: PeerId) -> Self {
        self.flags = self.flags.with_sender_peer_id();
        self.sender_peer_id = Some(peer_id);
        self
    }

    /// Set mentions
    pub fn with_mentions(mut self, mentions: Vec<String>) -> Self {
        if !mentions.is_empty() {
            self.flags = self.flags.with_mentions();
            self.mentions = Some(mentions);
        }
        self
    }

    /// Validate message structure
    pub fn validate(&self) -> Result<()> {
        // Check flag consistency
        if self.flags.has_original_sender() && self.original_sender.is_none() {
            return Err(BitchatError::invalid_packet(
                "Original sender flag set but no original sender",
            ));
        }

        if self.flags.has_recipient_nickname() && self.recipient_nickname.is_none() {
            return Err(BitchatError::invalid_packet(
                "Recipient nickname flag set but no recipient nickname",
            ));
        }

        if self.flags.has_sender_peer_id() && self.sender_peer_id.is_none() {
            return Err(BitchatError::invalid_packet(
                "Sender peer ID flag set but no sender peer ID",
            ));
        }

        if self.flags.has_mentions()
            && (self.mentions.is_none() || self.mentions.as_ref().unwrap().is_empty())
        {
            return Err(BitchatError::invalid_packet(
                "Mentions flag set but no mentions",
            ));
        }

        // Check field lengths
        if self.id.len() > 255 {
            return Err(BitchatError::invalid_packet("Message ID too long"));
        }

        if self.sender.len() > 255 {
            return Err(BitchatError::invalid_packet("Sender nickname too long"));
        }

        if self.content.len() > u16::MAX as usize {
            return Err(BitchatError::invalid_packet("Message content too long"));
        }

        if let Some(ref original_sender) = self.original_sender {
            if original_sender.len() > 255 {
                return Err(BitchatError::invalid_packet(
                    "Original sender nickname too long",
                ));
            }
        }

        if let Some(ref recipient_nickname) = self.recipient_nickname {
            if recipient_nickname.len() > 255 {
                return Err(BitchatError::invalid_packet("Recipient nickname too long"));
            }
        }

        Ok(())
    }

    /// Serialize message to binary format
    pub fn to_binary(&self) -> Result<Vec<u8>> {
        self.validate()?;

        let mut bytes = Vec::new();

        // Flags (1 byte)
        bytes.push(self.flags.as_u8());

        // Timestamp (8 bytes, big-endian)
        bytes.extend_from_slice(&self.timestamp.as_millis().to_be_bytes());

        // ID (1 byte length + data)
        let id_bytes = self.id.as_bytes();
        bytes.push(id_bytes.len() as u8);
        bytes.extend_from_slice(id_bytes);

        // Sender (1 byte length + data)
        let sender_bytes = self.sender.as_bytes();
        bytes.push(sender_bytes.len() as u8);
        bytes.extend_from_slice(sender_bytes);

        // Content (2 bytes length + data)
        let content_bytes = self.content.as_bytes();
        bytes.extend_from_slice(&(content_bytes.len() as u16).to_be_bytes());
        bytes.extend_from_slice(content_bytes);

        // Optional fields based on flags
        if self.flags.has_original_sender() {
            if let Some(ref original_sender) = self.original_sender {
                let original_sender_bytes = original_sender.as_bytes();
                bytes.push(original_sender_bytes.len() as u8);
                bytes.extend_from_slice(original_sender_bytes);
            }
        }

        if self.flags.has_recipient_nickname() {
            if let Some(ref recipient_nickname) = self.recipient_nickname {
                let recipient_nickname_bytes = recipient_nickname.as_bytes();
                bytes.push(recipient_nickname_bytes.len() as u8);
                bytes.extend_from_slice(recipient_nickname_bytes);
            }
        }

        if self.flags.has_sender_peer_id() {
            if let Some(sender_peer_id) = self.sender_peer_id {
                bytes.extend_from_slice(sender_peer_id.as_bytes());
            }
        }

        if self.flags.has_mentions() {
            if let Some(ref mentions) = self.mentions {
                bytes.push(mentions.len() as u8);
                for mention in mentions {
                    let mention_bytes = mention.as_bytes();
                    bytes.push(mention_bytes.len() as u8);
                    bytes.extend_from_slice(mention_bytes);
                }
            }
        }

        Ok(bytes)
    }

    /// Deserialize message from binary format
    pub fn from_binary(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 11 {
            return Err(BitchatError::invalid_packet("Message too short"));
        }

        let mut offset = 0;

        // Parse flags
        let flags = MessageFlags::new(bytes[offset]);
        offset += 1;

        // Parse timestamp
        let timestamp_bytes: [u8; 8] = bytes[offset..offset + 8]
            .try_into()
            .map_err(|_| BitchatError::invalid_packet("Invalid timestamp"))?;
        let timestamp = Timestamp::new(u64::from_be_bytes(timestamp_bytes));
        offset += 8;

        // Parse ID
        let id_length = bytes[offset] as usize;
        offset += 1;
        if bytes.len() < offset + id_length {
            return Err(BitchatError::invalid_packet("Message too short for ID"));
        }
        let id = String::from_utf8(bytes[offset..offset + id_length].to_vec())
            .map_err(|_| BitchatError::invalid_packet("Invalid ID encoding"))?;
        offset += id_length;

        // Parse sender
        if bytes.len() < offset + 1 {
            return Err(BitchatError::invalid_packet("Message too short for sender"));
        }
        let sender_length = bytes[offset] as usize;
        offset += 1;
        if bytes.len() < offset + sender_length {
            return Err(BitchatError::invalid_packet("Message too short for sender"));
        }
        let sender = String::from_utf8(bytes[offset..offset + sender_length].to_vec())
            .map_err(|_| BitchatError::invalid_packet("Invalid sender encoding"))?;
        offset += sender_length;

        // Parse content
        if bytes.len() < offset + 2 {
            return Err(BitchatError::invalid_packet(
                "Message too short for content length",
            ));
        }
        let content_length_bytes: [u8; 2] = bytes[offset..offset + 2]
            .try_into()
            .map_err(|_| BitchatError::invalid_packet("Invalid content length"))?;
        let content_length = u16::from_be_bytes(content_length_bytes) as usize;
        offset += 2;
        if bytes.len() < offset + content_length {
            return Err(BitchatError::invalid_packet(
                "Message too short for content",
            ));
        }
        let content = String::from_utf8(bytes[offset..offset + content_length].to_vec())
            .map_err(|_| BitchatError::invalid_packet("Invalid content encoding"))?;
        offset += content_length;

        // Parse optional fields
        let original_sender = if flags.has_original_sender() {
            if bytes.len() < offset + 1 {
                return Err(BitchatError::invalid_packet(
                    "Message too short for original sender",
                ));
            }
            let original_sender_length = bytes[offset] as usize;
            offset += 1;
            if bytes.len() < offset + original_sender_length {
                return Err(BitchatError::invalid_packet(
                    "Message too short for original sender",
                ));
            }
            let original_sender =
                String::from_utf8(bytes[offset..offset + original_sender_length].to_vec())
                    .map_err(|_| {
                        BitchatError::invalid_packet("Invalid original sender encoding")
                    })?;
            offset += original_sender_length;
            Some(original_sender)
        } else {
            None
        };

        let recipient_nickname = if flags.has_recipient_nickname() {
            if bytes.len() < offset + 1 {
                return Err(BitchatError::invalid_packet(
                    "Message too short for recipient nickname",
                ));
            }
            let recipient_nickname_length = bytes[offset] as usize;
            offset += 1;
            if bytes.len() < offset + recipient_nickname_length {
                return Err(BitchatError::invalid_packet(
                    "Message too short for recipient nickname",
                ));
            }
            let recipient_nickname =
                String::from_utf8(bytes[offset..offset + recipient_nickname_length].to_vec())
                    .map_err(|_| {
                        BitchatError::invalid_packet("Invalid recipient nickname encoding")
                    })?;
            offset += recipient_nickname_length;
            Some(recipient_nickname)
        } else {
            None
        };

        let sender_peer_id = if flags.has_sender_peer_id() {
            if bytes.len() < offset + 8 {
                return Err(BitchatError::invalid_packet(
                    "Message too short for sender peer ID",
                ));
            }
            let peer_id_bytes: [u8; 8] = bytes[offset..offset + 8]
                .try_into()
                .map_err(|_| BitchatError::invalid_packet("Invalid sender peer ID"))?;
            offset += 8;
            Some(PeerId::new(peer_id_bytes))
        } else {
            None
        };

        let mentions = if flags.has_mentions() {
            if bytes.len() < offset + 1 {
                return Err(BitchatError::invalid_packet(
                    "Message too short for mentions count",
                ));
            }
            let mentions_count = bytes[offset] as usize;
            offset += 1;
            let mut mentions = Vec::with_capacity(mentions_count);
            for _ in 0..mentions_count {
                if bytes.len() < offset + 1 {
                    return Err(BitchatError::invalid_packet(
                        "Message too short for mention",
                    ));
                }
                let mention_length = bytes[offset] as usize;
                offset += 1;
                if bytes.len() < offset + mention_length {
                    return Err(BitchatError::invalid_packet(
                        "Message too short for mention",
                    ));
                }
                let mention = String::from_utf8(bytes[offset..offset + mention_length].to_vec())
                    .map_err(|_| BitchatError::invalid_packet("Invalid mention encoding"))?;
                offset += mention_length;
                mentions.push(mention);
            }
            Some(mentions)
        } else {
            None
        };

        let message = Self {
            flags,
            timestamp,
            id,
            sender,
            content,
            original_sender,
            recipient_nickname,
            sender_peer_id,
            mentions,
        };

        message.validate()?;
        Ok(message)
    }
}

// ----------------------------------------------------------------------------
// Noise Payload Wrapper
// ----------------------------------------------------------------------------

/// Wrapper for encrypted payloads in Noise transport
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoisePayload {
    /// Payload type
    pub payload_type: NoisePayloadType,
    /// Payload data
    pub data: Vec<u8>,
}

impl NoisePayload {
    /// Create a new noise payload
    pub fn new(payload_type: NoisePayloadType, data: Vec<u8>) -> Self {
        Self { payload_type, data }
    }

    /// Serialize to binary format
    pub fn to_binary(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.payload_type.as_u8());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserialize from binary format
    pub fn from_binary(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(BitchatError::invalid_packet("Empty noise payload"));
        }

        let payload_type = NoisePayloadType::from_u8(bytes[0])?;
        let data = bytes[1..].to_vec();

        Ok(Self { payload_type, data })
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_payload_type_conversion() {
        assert_eq!(
            NoisePayloadType::from_u8(0x01).unwrap(),
            NoisePayloadType::PrivateMessage
        );
        assert_eq!(NoisePayloadType::ReadReceipt.as_u8(), 0x02);
        assert!(NoisePayloadType::from_u8(0xFF).is_err());
    }

    #[test]
    fn test_message_flags() {
        let flags = MessageFlags::NONE.with_relay().with_original_sender();

        assert!(flags.is_relay());
        assert!(flags.has_original_sender());
        assert!(!flags.has_mentions());
        assert_eq!(flags.as_u8(), 0x05);
    }

    #[test]
    fn test_bitchat_message_creation() {
        let message = BitchatMessage::new(
            "msg123".to_string(),
            "Alice".to_string(),
            "Hello, Bob!".to_string(),
        );

        assert_eq!(message.id, "msg123");
        assert_eq!(message.sender, "Alice");
        assert_eq!(message.content, "Hello, Bob!");
        assert!(!message.flags.is_relay());

        message.validate().unwrap();
    }

    #[test]
    fn test_bitchat_message_with_relay() {
        let message = BitchatMessage::new(
            "msg123".to_string(),
            "Bob".to_string(),
            "Hello, Charlie!".to_string(),
        )
        .with_relay("Alice".to_string());

        assert!(message.flags.is_relay());
        assert!(message.flags.has_original_sender());
        assert_eq!(message.original_sender, Some("Alice".to_string()));

        message.validate().unwrap();
    }

    #[test]
    fn test_bitchat_message_binary_roundtrip() {
        let message = BitchatMessage::new(
            "msg123".to_string(),
            "Alice".to_string(),
            "Hello, BitChat!".to_string(),
        )
        .with_mentions(vec!["@Bob".to_string(), "@Charlie".to_string()]);

        let binary = message.to_binary().unwrap();
        let parsed = BitchatMessage::from_binary(&binary).unwrap();

        assert_eq!(message, parsed);
    }

    #[test]
    fn test_noise_payload_roundtrip() {
        let payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            b"encrypted content".to_vec(),
        );

        let binary = payload.to_binary();
        let parsed = NoisePayload::from_binary(&binary).unwrap();

        assert_eq!(payload, parsed);
    }
}
