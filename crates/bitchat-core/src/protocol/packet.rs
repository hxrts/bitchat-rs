//! BitChat packet format and binary wire protocol
//!
//! This module implements the core BitchatPacket binary format as specified
//! in the protocol documentation, providing exact wire format compatibility.

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::protocol::crypto::IdentityKeyPair;
use crate::types::{PeerId, Timestamp, Ttl};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Protocol Constants
// ----------------------------------------------------------------------------

/// Protocol version 1 (13-byte header)
pub const PROTOCOL_VERSION_1: u8 = 1;

/// Protocol version 2 (15-byte header)  
pub const PROTOCOL_VERSION_2: u8 = 2;

/// Current protocol version
pub const CURRENT_PROTOCOL_VERSION: u8 = PROTOCOL_VERSION_1;

/// Fixed header size for version 1  
pub const HEADER_SIZE_V1: usize = 13;

/// Fixed header size for version 2
pub const HEADER_SIZE_V2: usize = 15;

/// Maximum payload size for version 1 (255 bytes)
pub const MAX_PAYLOAD_SIZE_V1: usize = 255;

/// Maximum payload size for version 2 (~4 GiB)
pub const MAX_PAYLOAD_SIZE_V2: usize = u32::MAX as usize;

// ----------------------------------------------------------------------------
// Message Types
// ----------------------------------------------------------------------------

/// Message types for the wire protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    /// Peer presence broadcast
    Announce = 0x01,
    /// Public chat message
    Message = 0x02,
    /// Graceful peer departure
    Leave = 0x03,
    /// Noise XX handshake (single type, not split)
    NoiseHandshake = 0x10,
    /// Container for all encrypted payloads
    NoiseEncrypted = 0x11,
    /// Large message fragmentation
    Fragment = 0x20,
    /// Mesh state synchronization request
    RequestSync = 0x21,
    /// File transfer protocol
    FileTransfer = 0x22,
    /// Protocol version negotiation
    VersionHello = 0x30,
    /// Version acknowledgment
    VersionAck = 0x31,
}

impl MessageType {
    /// Convert from raw byte value
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(MessageType::Announce),
            0x02 => Ok(MessageType::Message),
            0x03 => Ok(MessageType::Leave),
            0x10 => Ok(MessageType::NoiseHandshake),
            0x11 => Ok(MessageType::NoiseEncrypted),
            0x20 => Ok(MessageType::Fragment),
            0x21 => Ok(MessageType::RequestSync),
            0x22 => Ok(MessageType::FileTransfer),
            0x30 => Ok(MessageType::VersionHello),
            0x31 => Ok(MessageType::VersionAck),
            _ => Err(BitchatError::invalid_packet("Unknown message type")),
        }
    }

    /// Convert to raw byte value
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// ----------------------------------------------------------------------------
// Packet Flags
// ----------------------------------------------------------------------------

/// Flags controlling optional packet fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketFlags(u8);

impl PacketFlags {
    /// No optional fields present
    pub const NONE: Self = Self(0x00);

    /// Recipient ID field is present
    pub const HAS_RECIPIENT: Self = Self(0x01);

    /// Signature field is present
    pub const HAS_SIGNATURE: Self = Self(0x02);

    /// Payload is compressed with zlib
    pub const IS_COMPRESSED: Self = Self(0x04);

    /// Route field is present (reserved for future use)
    pub const HAS_ROUTE: Self = Self(0x08);

    /// Create flags from raw byte
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Get raw byte value
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Check if recipient ID is present
    pub const fn has_recipient(self) -> bool {
        (self.0 & Self::HAS_RECIPIENT.0) != 0
    }

    /// Check if signature is present
    pub const fn has_signature(self) -> bool {
        (self.0 & Self::HAS_SIGNATURE.0) != 0
    }

    /// Check if payload is compressed
    pub const fn is_compressed(self) -> bool {
        (self.0 & Self::IS_COMPRESSED.0) != 0
    }

    /// Check if route is present
    pub const fn has_route(self) -> bool {
        (self.0 & Self::HAS_ROUTE.0) != 0
    }

    /// Set recipient flag
    pub fn with_recipient(mut self) -> Self {
        self.0 |= Self::HAS_RECIPIENT.0;
        self
    }

    /// Set signature flag
    pub fn with_signature(mut self) -> Self {
        self.0 |= Self::HAS_SIGNATURE.0;
        self
    }

    /// Set compression flag
    pub fn with_compression(mut self) -> Self {
        self.0 |= Self::IS_COMPRESSED.0;
        self
    }

    /// Set route flag
    pub fn with_route(mut self) -> Self {
        self.0 |= Self::HAS_ROUTE.0;
        self
    }
}

// ----------------------------------------------------------------------------
// Packet Header
// ----------------------------------------------------------------------------

/// Binary packet header
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketHeader {
    /// Protocol version
    pub version: u8,
    /// Message type
    pub message_type: MessageType,
    /// Time-to-live for routing
    pub ttl: Ttl,
    /// Creation timestamp
    pub timestamp: Timestamp,
    /// Optional field flags
    pub flags: PacketFlags,
    /// Payload length in bytes
    pub payload_length: u32,
}

impl PacketHeader {
    /// Create a new packet header
    pub fn new(
        message_type: MessageType,
        ttl: Ttl,
        timestamp: Timestamp,
        flags: PacketFlags,
        payload_length: u32,
    ) -> Self {
        Self {
            version: CURRENT_PROTOCOL_VERSION,
            message_type,
            ttl,
            timestamp,
            flags,
            payload_length,
        }
    }

    /// Get the expected header size for this version
    pub fn header_size(&self) -> usize {
        match self.version {
            PROTOCOL_VERSION_1 => HEADER_SIZE_V1,
            PROTOCOL_VERSION_2 => HEADER_SIZE_V2,
            _ => HEADER_SIZE_V1, // Default to v1 for unknown versions
        }
    }

    /// Get maximum payload size for this version
    pub fn max_payload_size(&self) -> usize {
        match self.version {
            PROTOCOL_VERSION_1 => MAX_PAYLOAD_SIZE_V1,
            PROTOCOL_VERSION_2 => MAX_PAYLOAD_SIZE_V2,
            _ => MAX_PAYLOAD_SIZE_V1,
        }
    }

    /// Validate header fields
    pub fn validate(&self) -> Result<()> {
        // Check protocol version
        if self.version != PROTOCOL_VERSION_1 && self.version != PROTOCOL_VERSION_2 {
            return Err(BitchatError::invalid_packet("Unsupported protocol version"));
        }

        // Check payload length limits
        let max_size = self.max_payload_size();
        if self.payload_length as usize > max_size {
            return Err(BitchatError::invalid_packet("Payload too large"));
        }

        // Version 1 has stricter payload length limit
        if self.version == PROTOCOL_VERSION_1 && self.payload_length > 255 {
            return Err(BitchatError::invalid_packet("Payload too large for v1"));
        }

        Ok(())
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;

        let mut bytes = Vec::with_capacity(self.header_size());

        // Version (1 byte)
        bytes.push(self.version);

        // Type (1 byte)
        bytes.push(self.message_type.as_u8());

        // TTL (1 byte)
        bytes.push(self.ttl.value());

        // Timestamp (8 bytes, big-endian)
        bytes.extend_from_slice(&self.timestamp.as_millis().to_be_bytes());

        // Flags (1 byte)
        bytes.push(self.flags.as_u8());

        // Payload length (1 byte for v1, 4 bytes for v2, big-endian)
        match self.version {
            PROTOCOL_VERSION_1 => {
                if self.payload_length > 255 {
                    return Err(BitchatError::invalid_packet(
                        "Payload too large for v1 (max 255 bytes)",
                    ));
                }
                bytes.push(self.payload_length as u8);
            }
            PROTOCOL_VERSION_2 => {
                bytes.extend_from_slice(&self.payload_length.to_be_bytes());
            }
            _ => return Err(BitchatError::invalid_packet("Unsupported protocol version")),
        }

        Ok(bytes)
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(BitchatError::invalid_packet("Empty packet"));
        }

        // Read version to determine header size
        let version = bytes[0];
        let expected_size = match version {
            PROTOCOL_VERSION_1 => HEADER_SIZE_V1,
            PROTOCOL_VERSION_2 => HEADER_SIZE_V2,
            _ => return Err(BitchatError::invalid_packet("Unsupported protocol version")),
        };

        if bytes.len() < expected_size {
            return Err(BitchatError::invalid_packet("Packet too short"));
        }

        // Parse fixed fields
        let message_type = MessageType::from_u8(bytes[1])?;
        let ttl = Ttl::new(bytes[2]);

        // Parse timestamp (8 bytes, big-endian)
        let timestamp_bytes: [u8; 8] = bytes[3..11]
            .try_into()
            .map_err(|_| BitchatError::invalid_packet("Invalid timestamp"))?;
        let timestamp = Timestamp::new(u64::from_be_bytes(timestamp_bytes));

        let flags = PacketFlags::new(bytes[11]);

        // Parse payload length based on version
        let payload_length = match version {
            PROTOCOL_VERSION_1 => bytes[12] as u32,
            PROTOCOL_VERSION_2 => {
                let length_bytes: [u8; 4] = bytes[12..16]
                    .try_into()
                    .map_err(|_| BitchatError::invalid_packet("Invalid payload length"))?;
                u32::from_be_bytes(length_bytes)
            }
            _ => return Err(BitchatError::invalid_packet("Unsupported protocol version")),
        };

        let header = Self {
            version,
            message_type,
            ttl,
            timestamp,
            flags,
            payload_length,
        };

        header.validate()?;
        Ok(header)
    }
}

// ----------------------------------------------------------------------------
// BitChat Packet
// ----------------------------------------------------------------------------

/// Complete BitChat packet with header and variable fields
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitchatPacket {
    /// Packet header
    pub header: PacketHeader,
    /// Sender peer ID (always present)
    pub sender_id: PeerId,
    /// Optional recipient peer ID
    pub recipient_id: Option<PeerId>,
    /// Optional route (reserved for future use)
    pub route: Option<Vec<u8>>,
    /// Packet payload
    pub payload: Vec<u8>,
    /// Optional Ed25519 signature
    #[serde(with = "signature_serde")]
    pub signature: Option<[u8; 64]>,
}

impl BitchatPacket {
    /// Create a simple new packet (legacy method)
    pub fn new_simple(message_type: MessageType, sender_id: PeerId, payload: Vec<u8>) -> Self {
        let flags = PacketFlags::NONE;
        let header = PacketHeader::new(
            message_type,
            Ttl::DEFAULT,
            Timestamp::now(),
            flags,
            payload.len() as u32,
        );

        Self {
            header,
            sender_id,
            recipient_id: None,
            route: None,
            payload,
            signature: None,
        }
    }

    /// Set recipient (makes this a private message)
    pub fn with_recipient(mut self, recipient_id: PeerId) -> Self {
        self.recipient_id = Some(recipient_id);
        self.header.flags = self.header.flags.with_recipient();
        self
    }

    /// Set signature
    pub fn with_signature(mut self, signature: [u8; 64]) -> Self {
        self.signature = Some(signature);
        self.header.flags = self.header.flags.with_signature();
        self
    }

    /// Set TTL for routing
    pub fn with_ttl(mut self, ttl: Ttl) -> Self {
        self.header.ttl = ttl;
        self
    }

    /// Check if this is a broadcast message
    pub fn is_broadcast(&self) -> bool {
        self.recipient_id.is_none() || self.recipient_id == Some(PeerId::BROADCAST)
    }

    /// Check if this is a private message
    pub fn is_private(&self) -> bool {
        !self.is_broadcast()
    }

    /// Validate packet structure
    pub fn validate(&self) -> Result<()> {
        // Validate header
        self.header.validate()?;

        // Check payload length consistency
        if self.payload.len() != self.header.payload_length as usize {
            return Err(BitchatError::invalid_packet("Payload length mismatch"));
        }

        // Validate flags consistency
        if self.header.flags.has_recipient() && self.recipient_id.is_none() {
            return Err(BitchatError::invalid_packet(
                "Recipient flag set but no recipient",
            ));
        }

        if !self.header.flags.has_recipient() && self.recipient_id.is_some() {
            return Err(BitchatError::invalid_packet(
                "Recipient present but flag not set",
            ));
        }

        if self.header.flags.has_signature() && self.signature.is_none() {
            return Err(BitchatError::invalid_packet(
                "Signature flag set but no signature",
            ));
        }

        if !self.header.flags.has_signature() && self.signature.is_some() {
            return Err(BitchatError::invalid_packet(
                "Signature present but flag not set",
            ));
        }

        Ok(())
    }

    /// Create a new packet with explicit flags and optional recipient
    pub fn new(
        message_type: MessageType,
        sender_id: PeerId,
        recipient_id: Option<PeerId>,
        timestamp: Timestamp,
        payload: Vec<u8>,
        flags: PacketFlags,
    ) -> Result<Self> {
        let mut final_flags = flags;
        
        // Ensure flags are consistent with optional fields
        if recipient_id.is_some() {
            final_flags = final_flags.with_recipient();
        }

        let header = PacketHeader::new(
            message_type,
            Ttl::DEFAULT,
            timestamp,
            final_flags,
            payload.len() as u32,
        );

        let packet = Self {
            header,
            sender_id,
            recipient_id,
            route: None,
            payload,
            signature: None,
        };

        Ok(packet)
    }

    /// Sign the packet using an Ed25519 identity keypair
    pub fn sign(&mut self, identity_keypair: &IdentityKeyPair) -> Result<()> {
        // Create canonical bytes for signing (excluding signature and TTL)
        let canonical_bytes = self.canonical_bytes_for_signing()?;
        
        // Sign the canonical bytes
        let signature = identity_keypair.sign(&canonical_bytes);
        
        // Store signature and update flags
        self.signature = Some(signature);
        self.header.flags = self.header.flags.with_signature();
        
        Ok(())
    }

    /// Verify the packet's signature using an Ed25519 public key
    pub fn verify_signature(&self, public_key: &[u8; 32]) -> Result<()> {
        let signature = self.signature.ok_or_else(|| {
            BitchatError::invalid_packet("No signature present for verification")
        })?;

        // Recreate canonical bytes (excluding signature and TTL)
        let canonical_bytes = self.canonical_bytes_for_signing()?;
        
        // Verify the signature
        IdentityKeyPair::verify(public_key, &canonical_bytes, &signature)?;
        
        Ok(())
    }

    /// Create canonical bytes for signing/verification
    /// This excludes the signature field and TTL to allow for relay operations
    fn canonical_bytes_for_signing(&self) -> Result<Vec<u8>> {
        
        let mut hasher = Sha256::new();
        
        // Include context string
        hasher.update(b"bitchat-packet-v1");
        
        // Include packet fields (excluding signature and TTL)
        hasher.update(&[self.header.version]);
        hasher.update(&[self.header.message_type.as_u8()]);
        hasher.update(&self.header.timestamp.as_millis().to_be_bytes());
        
        // Include sender ID
        hasher.update(self.sender_id.as_bytes());
        
        // Include recipient ID if present
        if let Some(recipient_id) = &self.recipient_id {
            hasher.update(recipient_id.as_bytes());
        }
        
        // Include payload
        hasher.update(&self.payload);
        
        Ok(hasher.finalize().to_vec())
    }

    /// Get the message type
    pub fn message_type(&self) -> MessageType {
        self.header.message_type
    }

    /// Get the sender ID
    pub fn sender_id(&self) -> PeerId {
        self.sender_id
    }

    /// Get the recipient ID
    pub fn recipient_id(&self) -> Option<PeerId> {
        self.recipient_id
    }

    /// Get the payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
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
        assert_eq!(MessageType::from_u8(0x01).unwrap(), MessageType::Announce);
        assert_eq!(MessageType::Message.as_u8(), 0x02);
        assert!(MessageType::from_u8(0xFF).is_err());
    }

    #[test]
    fn test_packet_flags() {
        let flags = PacketFlags::NONE.with_recipient().with_signature();

        assert!(flags.has_recipient());
        assert!(flags.has_signature());
        assert!(!flags.is_compressed());
        assert_eq!(flags.as_u8(), 0x03);
    }

    #[test]
    fn test_header_serialization() {
        let header = PacketHeader::new(
            MessageType::Message,
            Ttl::new(5),
            Timestamp::new(1234567890000),
            PacketFlags::NONE.with_recipient(),
            100,
        );

        let bytes = header.to_bytes().unwrap();
        assert_eq!(bytes.len(), HEADER_SIZE_V1);

        let parsed = PacketHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_packet_creation() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let recipient = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);
        let payload = b"Hello, BitChat!".to_vec();

        let packet = BitchatPacket::new_simple(MessageType::Message, sender, payload.clone())
            .with_recipient(recipient);

        assert_eq!(packet.sender_id, sender);
        assert_eq!(packet.recipient_id, Some(recipient));
        assert_eq!(packet.payload, payload);
        assert!(packet.header.flags.has_recipient());
        assert!(packet.is_private());
        assert!(!packet.is_broadcast());

        packet.validate().unwrap();
    }

    #[test]
    fn test_broadcast_packet() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"Broadcast message".to_vec();

        let packet = BitchatPacket::new_simple(MessageType::Announce, sender, payload);

        assert!(packet.is_broadcast());
        assert!(!packet.is_private());
        assert!(!packet.header.flags.has_recipient());

        packet.validate().unwrap();
    }
}

// ----------------------------------------------------------------------------
// Custom Serde for large arrays
// ----------------------------------------------------------------------------

mod signature_serde {
    use alloc::vec::Vec;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<[u8; 64]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(array) => serializer.serialize_some(&array[..]),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 64]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::Deserialize;

        let opt_vec: Option<Vec<u8>> = Option::deserialize(deserializer)?;
        match opt_vec {
            Some(vec) => {
                if vec.len() == 64 {
                    let mut array = [0u8; 64];
                    array.copy_from_slice(&vec);
                    Ok(Some(array))
                } else {
                    Err(serde::de::Error::invalid_length(vec.len(), &"64 bytes"))
                }
            }
            None => Ok(None),
        }
    }
}
