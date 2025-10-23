//! Canonical NostrEmbeddedBitChat implementation
//!
//! This module provides canonical-compatible embedding strategies for BitChat messages
//! within Nostr events, matching the Swift/iOS reference implementation.

use std::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use super::error::NostrTransportError;
use bitchat_core::protocol::{BitchatPacket, MessageType, NoisePayload, NoisePayloadType, PacketFlags, WireFormat};
use bitchat_core::types::{PeerId, Timestamp};
use bitchat_core::{BitchatError, Result as BitchatResult};

/// Canonical BitChat prefix for Nostr embedding
pub const BITCHAT_EMBEDDING_PREFIX: &str = "bitchat1:";

// ----------------------------------------------------------------------------
// Embedding Strategy
// ----------------------------------------------------------------------------

/// Canonical embedding strategy for BitChat messages in Nostr events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingStrategy {
    /// Embed in NIP-17 encrypted private messages (default)
    PrivateMessage,
    /// Embed in public geohash events for location-based channels
    PublicGeohash,
    /// Use custom Nostr event kind for specialized applications
    CustomKind(u16),
}

impl Default for EmbeddingStrategy {
    fn default() -> Self {
        Self::PrivateMessage
    }
}

// ----------------------------------------------------------------------------
// Canonical NostrEmbeddedBitChat
// ----------------------------------------------------------------------------

/// Canonical implementation of NostrEmbeddedBitChat for encoding BitChat messages
/// and acknowledgments into Nostr-compatible format
pub struct NostrEmbeddedBitChat;

impl NostrEmbeddedBitChat {
    /// Encode a private message for Nostr transmission
    /// Matches canonical `encodePMForNostr` function
    pub fn encode_pm_for_nostr(
        sender_id: PeerId,
        recipient_id: PeerId,
        payload: &NoisePayload,
    ) -> BitchatResult<String> {
        Self::encode_pm_for_nostr_with_config(sender_id, recipient_id, payload, &EmbeddingConfig::default())
    }

    /// Encode a private message for Nostr transmission with custom configuration
    pub fn encode_pm_for_nostr_with_config(
        sender_id: PeerId,
        recipient_id: PeerId,
        payload: &NoisePayload,
        config: &EmbeddingConfig,
    ) -> BitchatResult<String> {
        // Create TLV-encoded private message with NoisePayloadType prefix
        let message_data = payload.to_binary();

        // Apply padding if configured
        let padded_data = config.apply_padding(&message_data);

        // Create BitchatPacket with noise encrypted type
        let packet = BitchatPacket::new(
            MessageType::NoiseEncrypted,
            sender_id,
            Some(recipient_id),
            Timestamp::now(),
            padded_data,
            PacketFlags::NONE,
        )?;

        Self::encode_packet_to_nostr(&packet)
    }

    /// Encode a private message for Nostr geohash DMs (no recipient)
    /// Matches canonical `encodePMForNostrNoRecipient` function
    pub fn encode_pm_for_nostr_no_recipient(
        sender_id: PeerId,
        payload: &NoisePayload,
    ) -> BitchatResult<String> {
        Self::encode_pm_for_nostr_no_recipient_with_config(sender_id, payload, &EmbeddingConfig::default())
    }

    /// Encode a private message for Nostr geohash DMs with custom configuration
    pub fn encode_pm_for_nostr_no_recipient_with_config(
        sender_id: PeerId,
        payload: &NoisePayload,
        config: &EmbeddingConfig,
    ) -> BitchatResult<String> {
        // Create TLV-encoded private message with NoisePayloadType prefix
        let message_data = payload.to_binary();

        // Apply padding if configured
        let padded_data = config.apply_padding(&message_data);

        // Create BitchatPacket with no recipient for geohash DMs
        let packet = BitchatPacket::new(
            MessageType::NoiseEncrypted,
            sender_id,
            None, // No recipient for geohash DMs
            Timestamp::now(),
            padded_data,
            PacketFlags::NONE,
        )?;

        Self::encode_packet_to_nostr(&packet)
    }

    /// Encode a delivery acknowledgment for Nostr transmission
    /// Matches canonical `encodeAckForNostr` function
    pub fn encode_ack_for_nostr(
        sender_id: PeerId,
        recipient_id: PeerId,
        ack_payload: &NoisePayload,
    ) -> BitchatResult<String> {
        // Validate that this is an acknowledgment payload
        match ack_payload.payload_type {
            NoisePayloadType::Delivered | NoisePayloadType::ReadReceipt => {
                // Valid acknowledgment types
            }
            _ => {
                return Err(BitchatError::invalid_packet(
                    "Payload must be a delivery acknowledgment or read receipt".to_string(),
                ));
            }
        }

        // Create acknowledgment data with NoisePayloadType prefix
        let ack_data = ack_payload.to_binary();

        // Create BitchatPacket for acknowledgment
        let packet = BitchatPacket::new(
            MessageType::NoiseEncrypted,
            sender_id,
            Some(recipient_id),
            Timestamp::now(),
            ack_data,
            PacketFlags::NONE,
        )?;

        Self::encode_packet_to_nostr(&packet)
    }

    /// Encode an acknowledgment for Nostr geohash DMs (no recipient)
    /// Matches canonical `encodeAckForNostrNoRecipient` function
    pub fn encode_ack_for_nostr_no_recipient(
        sender_id: PeerId,
        ack_payload: &NoisePayload,
    ) -> BitchatResult<String> {
        // Validate that this is an acknowledgment payload
        match ack_payload.payload_type {
            NoisePayloadType::Delivered | NoisePayloadType::ReadReceipt => {
                // Valid acknowledgment types
            }
            _ => {
                return Err(BitchatError::invalid_packet(
                    "Payload must be a delivery acknowledgment or read receipt".to_string(),
                ));
            }
        }

        // Create acknowledgment data with NoisePayloadType prefix
        let ack_data = ack_payload.to_binary();

        // Create BitchatPacket for acknowledgment with no recipient
        let packet = BitchatPacket::new(
            MessageType::NoiseEncrypted,
            sender_id,
            None, // No recipient for geohash DMs
            Timestamp::now(),
            ack_data,
            PacketFlags::NONE,
        )?;

        Self::encode_packet_to_nostr(&packet)
    }

    /// Core function to encode a BitchatPacket to Nostr format
    /// Creates the canonical `bitchat1:` prefixed base64url string
    fn encode_packet_to_nostr(packet: &BitchatPacket) -> BitchatResult<String> {
        // Serialize packet to binary wire format
        let binary_data = WireFormat::encode(packet).map_err(|e| {
            BitchatError::invalid_packet(format!("Failed to encode packet: {}", e))
        })?;

        // Base64url encode (canonical uses base64url, not standard base64)
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                use base64::{engine::general_purpose, Engine as _};
                let base64_data = general_purpose::URL_SAFE_NO_PAD.encode(&binary_data);
            } else {
                // WASM stub - would use web crypto APIs
                let base64_data = "base64url_stub".to_string();
            }
        }

        // Add canonical prefix
        Ok(format!("{}{}", BITCHAT_EMBEDDING_PREFIX, base64_data))
    }

    /// Decode a Nostr-embedded BitChat message
    /// Extracts BitchatPacket from canonical `bitchat1:` format
    pub fn decode_from_nostr(embedded_content: &str) -> Result<Option<BitchatPacket>, NostrTransportError> {
        // Check for canonical prefix
        if !embedded_content.starts_with(BITCHAT_EMBEDDING_PREFIX) {
            return Ok(None); // Not a BitChat message
        }

        let base64_data = &embedded_content[BITCHAT_EMBEDDING_PREFIX.len()..];

        // Base64url decode
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                use base64::{engine::general_purpose, Engine as _};
                let binary_data = general_purpose::URL_SAFE_NO_PAD.decode(base64_data)
                    .map_err(|e| NostrTransportError::DeserializationFailed(format!("Invalid base64url: {}", e)))?;
            } else {
                // WASM stub
                return Err(NostrTransportError::DeserializationFailed("Base64url decode not implemented for WASM".to_string()));
            }
        }

        // Deserialize from wire format
        let packet = WireFormat::decode(&binary_data).map_err(|e| {
            NostrTransportError::DeserializationFailed(format!("Invalid wire format: {}", e))
        })?;

        Ok(Some(packet))
    }

    /// Extract NoisePayload from decoded BitchatPacket
    /// Handles the NoisePayloadType prefix added during encoding
    pub fn extract_noise_payload(packet: &BitchatPacket) -> Result<NoisePayload, NostrTransportError> {
        Self::extract_noise_payload_with_config(packet, &EmbeddingConfig::default())
    }

    /// Extract NoisePayload from decoded BitchatPacket with padding removal
    pub fn extract_noise_payload_with_config(
        packet: &BitchatPacket, 
        config: &EmbeddingConfig
    ) -> Result<NoisePayload, NostrTransportError> {
        if packet.payload.is_empty() {
            return Err(NostrTransportError::DeserializationFailed(
                "Empty packet payload".to_string(),
            ));
        }

        // Remove padding if configured
        let unpadded_data = config.remove_padding(&packet.payload)?;

        // Extract NoisePayload from packet data
        let noise_payload = NoisePayload::from_binary(&unpadded_data).map_err(|e| {
            NostrTransportError::DeserializationFailed(format!("Failed to deserialize NoisePayload: {}", e))
        })?;

        Ok(noise_payload)
    }

    /// Check if content contains an embedded BitChat message
    pub fn is_bitchat_content(content: &str) -> bool {
        content.starts_with(BITCHAT_EMBEDDING_PREFIX)
    }

    /// Get the embedding strategy best suited for a message type
    pub fn recommended_strategy(payload_type: NoisePayloadType, is_geohash: bool) -> EmbeddingStrategy {
        match (payload_type, is_geohash) {
            (NoisePayloadType::PrivateMessage, true) => EmbeddingStrategy::PublicGeohash,
            (NoisePayloadType::PrivateMessage, false) => EmbeddingStrategy::PrivateMessage,
            (NoisePayloadType::Delivered, _) => EmbeddingStrategy::PrivateMessage,
            (NoisePayloadType::ReadReceipt, _) => EmbeddingStrategy::PrivateMessage,
            _ => EmbeddingStrategy::PrivateMessage,
        }
    }
}

// ----------------------------------------------------------------------------
// Embedding Configuration
// ----------------------------------------------------------------------------

/// Configuration for BitChat embedding in Nostr events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Default embedding strategy
    pub default_strategy: EmbeddingStrategy,
    /// Enable message padding for traffic analysis resistance
    pub enable_padding: bool,
    /// Maximum padding size in bytes
    pub max_padding_bytes: usize,
    /// Enable timing randomization
    pub enable_timing_jitter: bool,
    /// Maximum timing jitter in milliseconds
    pub max_timing_jitter_ms: u64,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 128,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 1000,
        }
    }
}

impl EmbeddingConfig {
    /// Create configuration optimized for privacy
    pub fn privacy_focused() -> Self {
        Self {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 256,
            enable_timing_jitter: true,
            max_timing_jitter_ms: 2000,
        }
    }

    /// Create configuration optimized for performance
    pub fn performance_focused() -> Self {
        Self {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        }
    }

    /// Apply padding to message data based on configuration
    pub fn apply_padding(&self, data: &[u8]) -> Vec<u8> {
        if !self.enable_padding || self.max_padding_bytes == 0 {
            return data.to_vec();
        }

        // Generate random padding length (0 to max_padding_bytes)
        use rand::Rng;
        let padding_length = rand::thread_rng().gen_range(0..=self.max_padding_bytes);
        
        if padding_length == 0 {
            return data.to_vec();
        }

        let mut padded_data = data.to_vec();
        
        // Add random padding bytes
        let padding_bytes: Vec<u8> = (0..padding_length)
            .map(|_| rand::thread_rng().gen::<u8>())
            .collect();
        
        // Use a simple padding scheme: data + length_byte + padding
        padded_data.push(padding_length as u8);
        padded_data.extend_from_slice(&padding_bytes);
        
        padded_data
    }

    /// Remove padding from message data
    pub fn remove_padding(&self, padded_data: &[u8]) -> Result<Vec<u8>, NostrTransportError> {
        if !self.enable_padding || padded_data.is_empty() {
            return Ok(padded_data.to_vec());
        }

        // Check if data has enough bytes for padding scheme: at least original_data + length_byte
        if padded_data.len() < 2 {
            return Ok(padded_data.to_vec()); // Not padded
        }

        // The padding scheme is: original_data + padding_length_byte + padding_bytes
        // So we need to find where the padding starts by reading backwards
        
        let data_len = padded_data.len();
        
        // Try different possible padding lengths from the end
        for possible_padding_len in 1..=self.max_padding_bytes.min(data_len - 1) {
            if data_len < possible_padding_len + 1 {
                continue;
            }
            
            // The padding length byte should be at position: data_len - possible_padding_len - 1
            let padding_len_pos = data_len - possible_padding_len - 1;
            let stored_padding_len = padded_data[padding_len_pos] as usize;
            
            // Check if the stored padding length matches what we expect
            if stored_padding_len == possible_padding_len {
                // Found valid padding, return the original data (everything before the padding length byte)
                return Ok(padded_data[0..padding_len_pos].to_vec());
            }
        }

        // No valid padding found, return original data
        Ok(padded_data.to_vec())
    }

    /// Get timing jitter delay based on configuration
    pub fn get_timing_jitter(&self) -> std::time::Duration {
        if !self.enable_timing_jitter || self.max_timing_jitter_ms == 0 {
            return std::time::Duration::from_millis(0);
        }

        use rand::Rng;
        let jitter_ms = rand::thread_rng().gen_range(0..=self.max_timing_jitter_ms);
        std::time::Duration::from_millis(jitter_ms)
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.max_padding_bytes > 2048 {
            return Err("max_padding_bytes exceeds reasonable limit (2048)".to_string());
        }

        if self.max_timing_jitter_ms > 60_000 {
            return Err("max_timing_jitter_ms exceeds reasonable limit (60s)".to_string());
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::protocol::{BitchatMessage, MessageFlags};
    use bitchat_core::types::PeerId;

    #[test]
    fn test_canonical_embedding_roundtrip() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let recipient = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);

        // Create a test private message
        let message = BitchatMessage::new(
            "msg123".to_string(),
            "Alice".to_string(),
            "Hello, Nostr!".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        // Encode for Nostr
        let embedded = NostrEmbeddedBitChat::encode_pm_for_nostr(
            sender,
            recipient,
            &noise_payload,
        ).unwrap();

        // Should have canonical prefix
        assert!(embedded.starts_with(BITCHAT_EMBEDDING_PREFIX));
        assert!(NostrEmbeddedBitChat::is_bitchat_content(&embedded));

        // Decode from Nostr
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
            .unwrap()
            .unwrap();

        // Verify packet structure
        assert_eq!(decoded_packet.sender_id, sender);
        assert_eq!(decoded_packet.recipient_id, Some(recipient));
        assert_eq!(decoded_packet.message_type(), MessageType::NoiseEncrypted);

        // Extract and verify payload
        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload(&decoded_packet).unwrap();
        assert_eq!(extracted_payload.payload_type, NoisePayloadType::PrivateMessage);

        // For this test, we'd need to deserialize the payload data back to BitchatMessage
        // This is a simplified test - in practice you'd use the appropriate deserialization
    }

    #[test]
    fn test_geohash_encoding_no_recipient() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        let message = BitchatMessage::new(
            "geo_msg".to_string(),
            "Bob".to_string(),
            "Geohash message".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        // Encode for geohash (no recipient)
        let embedded = NostrEmbeddedBitChat::encode_pm_for_nostr_no_recipient(
            sender,
            &noise_payload,
        ).unwrap();

        // Decode and verify
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
            .unwrap()
            .unwrap();

        assert_eq!(decoded_packet.sender_id, sender);
        assert_eq!(decoded_packet.recipient_id, None); // No recipient for geohash
    }

    #[test]
    fn test_acknowledgment_encoding() {
        use bitchat_core::protocol::acknowledgments::DeliveryAck;
        use bitchat_core::protocol::message_store::MessageId;
        use sha2::{Digest, Sha256};

        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let recipient = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);

        // Create test delivery acknowledgment
        let hash = Sha256::digest(b"test message");
        let message_id = MessageId::from_bytes(hash.into());
        let delivery_ack = DeliveryAck::new(message_id, sender, Some("Alice".to_string()));
        let ack_payload = NoisePayload::new(
            NoisePayloadType::Delivered,
            delivery_ack.to_binary().unwrap(),
        );

        // Encode acknowledgment
        let embedded = NostrEmbeddedBitChat::encode_ack_for_nostr(
            sender,
            recipient,
            &ack_payload,
        ).unwrap();

        // Decode and verify
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
            .unwrap()
            .unwrap();

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload(&decoded_packet).unwrap();
        assert_eq!(extracted_payload.payload_type, NoisePayloadType::Delivered);
    }

    #[test]
    fn test_non_bitchat_content() {
        let regular_content = "This is just a regular Nostr message";
        assert!(!NostrEmbeddedBitChat::is_bitchat_content(regular_content));

        let result = NostrEmbeddedBitChat::decode_from_nostr(regular_content).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_strategy_recommendations() {
        // Private messages in geohash should use public geohash strategy
        assert_eq!(
            NostrEmbeddedBitChat::recommended_strategy(NoisePayloadType::PrivateMessage, true),
            EmbeddingStrategy::PublicGeohash
        );

        // Private messages not in geohash should use private message strategy
        assert_eq!(
            NostrEmbeddedBitChat::recommended_strategy(NoisePayloadType::PrivateMessage, false),
            EmbeddingStrategy::PrivateMessage
        );

        // Acknowledgments should always use private message strategy
        assert_eq!(
            NostrEmbeddedBitChat::recommended_strategy(NoisePayloadType::Delivered, true),
            EmbeddingStrategy::PrivateMessage
        );
        assert_eq!(
            NostrEmbeddedBitChat::recommended_strategy(NoisePayloadType::ReadReceipt, false),
            EmbeddingStrategy::PrivateMessage
        );
    }

    #[test]
    fn test_embedding_config_padding() {
        let config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 8, // Small padding for reliable test
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };

        let test_data = b"Hello, BitChat!";
        let padded = config.apply_padding(test_data);
        
        // Should be longer than original due to padding
        assert!(padded.len() > test_data.len());
        
        // Should be able to remove padding and get original data back
        let unpadded = config.remove_padding(&padded).unwrap();
        assert_eq!(unpadded, test_data);
    }

    #[test]
    fn test_embedding_config_no_padding() {
        let config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };

        let test_data = b"Hello, BitChat!";
        let padded = config.apply_padding(test_data);
        
        // Should be same length when padding disabled
        assert_eq!(padded.len(), test_data.len());
        assert_eq!(padded, test_data);
        
        let unpadded = config.remove_padding(&padded).unwrap();
        assert_eq!(unpadded, test_data);
    }

    #[test]
    fn test_embedding_config_timing_jitter() {
        let config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: true,
            max_timing_jitter_ms: 1000,
        };

        let jitter = config.get_timing_jitter();
        assert!(jitter.as_millis() <= 1000);
        
        let config_no_jitter = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };
        
        let no_jitter = config_no_jitter.get_timing_jitter();
        assert_eq!(no_jitter.as_millis(), 0);
    }

    #[test]
    fn test_embedding_config_validation() {
        let valid_config = EmbeddingConfig::default();
        assert!(valid_config.validate().is_ok());

        let invalid_padding_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 3000, // Too large
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };
        assert!(invalid_padding_config.validate().is_err());

        let invalid_jitter_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: true,
            max_timing_jitter_ms: 70_000, // Too large
        };
        assert!(invalid_jitter_config.validate().is_err());
    }

    #[test]
    fn test_privacy_focused_config() {
        let config = EmbeddingConfig::privacy_focused();
        assert!(config.enable_padding);
        assert!(config.enable_timing_jitter);
        assert!(config.max_padding_bytes > 0);
        assert!(config.max_timing_jitter_ms > 0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_performance_focused_config() {
        let config = EmbeddingConfig::performance_focused();
        assert!(!config.enable_padding);
        assert!(!config.enable_timing_jitter);
        assert_eq!(config.max_padding_bytes, 0);
        assert_eq!(config.max_timing_jitter_ms, 0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_padding_edge_cases() {
        let config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 10,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };

        // Empty data
        let empty_data = b"";
        let padded_empty = config.apply_padding(empty_data);
        let unpadded_empty = config.remove_padding(&padded_empty).unwrap();
        assert_eq!(unpadded_empty, empty_data);

        // Single byte
        let single_byte = b"a";
        let padded_single = config.apply_padding(single_byte);
        let unpadded_single = config.remove_padding(&padded_single).unwrap();
        assert_eq!(unpadded_single, single_byte);

        // Data that's not padded (should pass through unchanged)
        let unpadded_data = b"not padded data";
        let result = config.remove_padding(unpadded_data).unwrap();
        assert_eq!(result, unpadded_data);
    }
}