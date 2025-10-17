//! BitChat packet and message structures
//!
//! This module defines the binary wire format for BitChat protocol messages,
//! following the specification from the BitChat whitepaper.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{PeerId, Timestamp, Ttl};

// ----------------------------------------------------------------------------
// Message Types
// ----------------------------------------------------------------------------

/// Message types for BitChat packets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    /// Regular chat message
    Message = 0x01,
    /// Delivery acknowledgment
    DeliveryAck = 0x02,
    /// Read receipt
    ReadReceipt = 0x03,
    /// Noise handshake initiation
    NoiseHandshakeInit = 0x10,
    /// Noise handshake response
    NoiseHandshakeResponse = 0x11,
    /// Noise handshake finalization
    NoiseHandshakeFinalize = 0x12,
    /// Identity announcement
    Announce = 0x20,
    /// Request sync from peers
    RequestSync = 0x30,
    /// Fragment start
    FragmentStart = 0x40,
    /// Fragment continuation
    FragmentContinue = 0x41,
    /// Fragment end
    FragmentEnd = 0x42,
}

impl MessageType {
    /// Convert from u8, returning None for unknown values
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Message),
            0x02 => Some(Self::DeliveryAck),
            0x03 => Some(Self::ReadReceipt),
            0x10 => Some(Self::NoiseHandshakeInit),
            0x11 => Some(Self::NoiseHandshakeResponse),
            0x12 => Some(Self::NoiseHandshakeFinalize),
            0x20 => Some(Self::Announce),
            0x30 => Some(Self::RequestSync),
            0x40 => Some(Self::FragmentStart),
            0x41 => Some(Self::FragmentContinue),
            0x42 => Some(Self::FragmentEnd),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// Packet Flags
// ----------------------------------------------------------------------------

/// Flags for optional packet fields
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PacketFlags {
    pub has_recipient: bool,
    pub has_signature: bool,
    pub is_compressed: bool,
}

impl PacketFlags {
    /// Convert flags to a single byte
    pub fn to_byte(self) -> u8 {
        let mut flags = 0u8;
        if self.has_recipient { flags |= 0x01; }
        if self.has_signature { flags |= 0x02; }
        if self.is_compressed { flags |= 0x04; }
        flags
    }

    /// Create flags from a byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            has_recipient: (byte & 0x01) != 0,
            has_signature: (byte & 0x02) != 0,
            is_compressed: (byte & 0x04) != 0,
        }
    }
}

// ----------------------------------------------------------------------------
// BitChat Packet
// ----------------------------------------------------------------------------

/// Main BitChat packet structure
///
/// This represents the complete packet format as transmitted over the wire.
/// The binary layout follows the specification exactly for cross-platform compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatPacket {
    /// Protocol version (currently 1)
    pub version: u8,
    /// Message type
    pub message_type: MessageType,
    /// Time-to-live for routing
    pub ttl: Ttl,
    /// Packet creation timestamp
    pub timestamp: Timestamp,
    /// Optional field flags
    pub flags: PacketFlags,
    /// Sender's peer ID
    pub sender_id: PeerId,
    /// Optional recipient ID (None for broadcast)
    pub recipient_id: Option<PeerId>,
    /// Packet payload
    pub payload: Vec<u8>,
    /// Optional Ed25519 signature
    #[serde(with = "signature_serde")]
    pub signature: Option<[u8; 64]>,
}

impl BitchatPacket {
    /// Current protocol version
    pub const CURRENT_VERSION: u8 = 1;

    /// Create a new packet with required fields
    pub fn new(
        message_type: MessageType,
        sender_id: PeerId,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            message_type,
            ttl: Ttl::default(),
            #[cfg(feature = "std")]
            timestamp: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            timestamp: Timestamp::new(0),
            flags: PacketFlags::default(),
            sender_id,
            recipient_id: None,
            payload,
            signature: None,
        }
    }

    /// Set the recipient (for private messages)
    pub fn with_recipient(mut self, recipient_id: PeerId) -> Self {
        self.recipient_id = Some(recipient_id);
        self.flags.has_recipient = true;
        self
    }

    /// Add a signature to the packet
    pub fn with_signature(mut self, signature: [u8; 64]) -> Self {
        self.signature = Some(signature);
        self.flags.has_signature = true;
        self
    }

    /// Check if this is a broadcast message
    pub fn is_broadcast(&self) -> bool {
        self.recipient_id.is_none() || 
        self.recipient_id == Some(PeerId::BROADCAST)
    }

    /// Get the payload length
    pub fn payload_len(&self) -> u16 {
        self.payload.len() as u16
    }
}

// ----------------------------------------------------------------------------
// BitChat Message
// ----------------------------------------------------------------------------

/// Application-level message content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatMessage {
    /// Message flags
    pub flags: MessageFlags,
    /// Message creation timestamp
    pub timestamp: Timestamp,
    /// Unique message identifier
    pub id: Uuid,
    /// Sender's nickname
    pub sender: String,
    /// Message content
    pub content: String,
    /// Original sender for relayed messages
    pub original_sender: Option<String>,
    /// Recipient nickname for private messages
    pub recipient_nickname: Option<String>,
}

/// Flags for BitChat messages
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MessageFlags {
    pub is_relay: bool,
    pub is_private: bool,
    pub has_original_sender: bool,
}

impl MessageFlags {
    /// Convert flags to a single byte
    pub fn to_byte(self) -> u8 {
        let mut flags = 0u8;
        if self.is_relay { flags |= 0x01; }
        if self.is_private { flags |= 0x02; }
        if self.has_original_sender { flags |= 0x04; }
        flags
    }

    /// Create flags from a byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            is_relay: (byte & 0x01) != 0,
            is_private: (byte & 0x02) != 0,
            has_original_sender: (byte & 0x04) != 0,
        }
    }
}

impl BitchatMessage {
    /// Create a new message
    pub fn new(sender: String, content: String) -> Self {
        Self {
            flags: MessageFlags::default(),
            #[cfg(feature = "std")]
            timestamp: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            timestamp: Timestamp::new(0),
            id: Uuid::new_v4(),
            sender,
            content,
            original_sender: None,
            recipient_nickname: None,
        }
    }

    /// Mark as a private message
    pub fn as_private(mut self, recipient_nickname: String) -> Self {
        self.flags.is_private = true;
        self.recipient_nickname = Some(recipient_nickname);
        self
    }

    /// Mark as a relayed message
    pub fn as_relay(mut self, original_sender: String) -> Self {
        self.flags.is_relay = true;
        self.flags.has_original_sender = true;
        self.original_sender = Some(original_sender);
        self
    }
}

// ----------------------------------------------------------------------------
// Delivery Acknowledgment
// ----------------------------------------------------------------------------

/// Delivery acknowledgment payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAck {
    /// ID of the acknowledged message
    pub message_id: Uuid,
    /// Timestamp of acknowledgment
    pub timestamp: Timestamp,
}

impl DeliveryAck {
    /// Create a new delivery acknowledgment
    pub fn new(message_id: Uuid) -> Self {
        Self {
            message_id,
            #[cfg(feature = "std")]
            timestamp: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            timestamp: Timestamp::new(0),
        }
    }
}

// ----------------------------------------------------------------------------
// Read Receipt
// ----------------------------------------------------------------------------

/// Read receipt payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceipt {
    /// ID of the read message
    pub message_id: Uuid,
    /// Timestamp when message was read
    pub timestamp: Timestamp,
}

impl ReadReceipt {
    /// Create a new read receipt
    pub fn new(message_id: Uuid) -> Self {
        Self {
            message_id,
            #[cfg(feature = "std")]
            timestamp: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            timestamp: Timestamp::new(0),
        }
    }
}

// ----------------------------------------------------------------------------
// Serde Helpers
// ----------------------------------------------------------------------------

/// Custom serde module for 64-byte signatures
mod signature_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(signature: &Option<[u8; 64]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match signature {
            Some(sig) => sig.as_slice().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 64]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<Vec<u8>> = Option::deserialize(deserializer)?;
        match opt {
            Some(vec) => {
                if vec.len() == 64 {
                    let mut array = [0u8; 64];
                    array.copy_from_slice(&vec);
                    Ok(Some(array))
                } else {
                    Err(serde::de::Error::custom("signature must be 64 bytes"))
                }
            }
            None => Ok(None),
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::from_u8(0x01), Some(MessageType::Message));
        assert_eq!(MessageType::from_u8(0x02), Some(MessageType::DeliveryAck));
        assert_eq!(MessageType::from_u8(0xFF), None);
    }

    #[test]
    fn test_packet_flags() {
        let flags = PacketFlags {
            has_recipient: true,
            has_signature: false,
            is_compressed: true,
        };
        
        let byte = flags.to_byte();
        assert_eq!(byte, 0x05); // 0x01 | 0x04
        
        let parsed = PacketFlags::from_byte(byte);
        assert_eq!(parsed.has_recipient, true);
        assert_eq!(parsed.has_signature, false);
        assert_eq!(parsed.is_compressed, true);
    }

    #[test]
    fn test_packet_creation() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test payload".to_vec();
        
        let packet = BitchatPacket::new(
            MessageType::Message,
            sender,
            payload.clone(),
        );
        
        assert_eq!(packet.version, BitchatPacket::CURRENT_VERSION);
        assert_eq!(packet.message_type, MessageType::Message);
        assert_eq!(packet.sender_id, sender);
        assert_eq!(packet.payload, payload);
        assert!(packet.is_broadcast());
    }

    #[test]
    fn test_message_creation() {
        let msg = BitchatMessage::new(
            "alice".to_string(),
            "Hello, world!".to_string(),
        );
        
        assert_eq!(msg.sender, "alice");
        assert_eq!(msg.content, "Hello, world!");
        assert!(!msg.flags.is_private);
        assert!(!msg.flags.is_relay);
    }
}