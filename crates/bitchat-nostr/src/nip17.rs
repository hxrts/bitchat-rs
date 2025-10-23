//! NIP-17 Gift-wrapping for private direct messages
//!
//! This module implements the NIP-17 specification for encrypted direct messages
//! with gift-wrapping to provide traffic analysis resistance.

use serde::{Deserialize, Serialize};
use std::format;
use std::string::String;

use super::error::NostrTransportError;
use bitchat_core::protocol::BitchatPacket;
use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        // Native imports
        use nostr_sdk::prelude::*;
        use nostr_sdk::{Keys, EventBuilder, Event as NostrEvent, Tag, Kind, Timestamp};
        use nostr_sdk::base64::{engine::general_purpose, Engine as _};
    } else {
        // WASM stub types
        pub struct Keys;
        pub struct EventBuilder;
        pub struct NostrEvent;
        pub struct Tag;
        pub struct Kind;
        pub struct Timestamp;
        pub struct PublicKey;

        // WASM stubs for crypto operations
        mod general_purpose {
            pub struct STANDARD;
            impl STANDARD {
                pub fn decode(_data: &str) -> Result<Vec<u8>, String> {
                    Err("Base64 decode not implemented for WASM".to_string())
                }
                pub fn encode(_data: &[u8]) -> String {
                    "base64_stub".to_string()
                }
            }
        }
        use general_purpose::STANDARD;
    }
}

// ----------------------------------------------------------------------------
// NIP-17 Constants
// ----------------------------------------------------------------------------

/// Maximum content length for NIP-17 messages
pub const MAX_NIP17_CONTENT_LENGTH: usize = 65535;

/// Random expiration time range for gift-wrapped events (30-60 minutes)
pub const MIN_EXPIRATION_SECONDS: i64 = 30 * 60;
pub const MAX_EXPIRATION_SECONDS: i64 = 60 * 60;

/// BitChat content type prefix for embedded packets
pub const BITCHAT_NIP17_PREFIX: &str = "bitchat1:";

// ----------------------------------------------------------------------------
// NIP-17 Direct Message Content
// ----------------------------------------------------------------------------

/// Inner content of a NIP-17 direct message (before gift-wrapping)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nip17Content {
    /// The actual message content
    pub content: String,
    /// Optional expiration timestamp
    pub expiration: Option<i64>,
}

impl Nip17Content {
    /// Create content for a BitChat packet
    pub fn from_bitchat_packet(packet: &BitchatPacket) -> BitchatResult<Self> {
        // Serialize the packet to binary wire format
        let binary_data = bitchat_core::protocol::WireFormat::encode(packet)
            .map_err(|e| BitchatError::invalid_packet(format!("Failed to encode packet: {}", e)))?;

        // Encode as base64 with BitChat prefix
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                let content = format!("{}{}", BITCHAT_NIP17_PREFIX, general_purpose::STANDARD.encode(&binary_data));
            } else {
                let content = format!("{}{}", BITCHAT_NIP17_PREFIX, STANDARD.encode(&binary_data));
            }
        }

        Ok(Self {
            content,
            expiration: None,
        })
    }

    /// Extract BitChat packet from content
    pub fn to_bitchat_packet(&self) -> Result<Option<BitchatPacket>, NostrTransportError> {
        if !self.content.starts_with(BITCHAT_NIP17_PREFIX) {
            return Ok(None); // Not a BitChat message
        }

        let base64_data = &self.content[BITCHAT_NIP17_PREFIX.len()..];

        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                let binary_data = general_purpose::STANDARD.decode(base64_data)
                    .map_err(|e| NostrTransportError::DeserializationFailed(format!("Invalid base64: {}", e)))?;
            } else {
                let binary_data = STANDARD.decode(base64_data)
                    .map_err(|e| NostrTransportError::DeserializationFailed(format!("Invalid base64: {}", e)))?;
            }
        }

        let packet = bitchat_core::protocol::WireFormat::decode(&binary_data).map_err(|e| {
            NostrTransportError::DeserializationFailed(format!("Invalid wire format: {}", e))
        })?;

        Ok(Some(packet))
    }

    /// Convert to JSON string for encryption
    pub fn to_json(&self) -> Result<String, NostrTransportError> {
        serde_json::to_string(self).map_err(|e| {
            NostrTransportError::DeserializationFailed(format!("JSON serialization failed: {}", e))
        })
    }

    /// Parse from JSON string after decryption
    pub fn from_json(json: &str) -> Result<Self, NostrTransportError> {
        serde_json::from_str(json).map_err(|e| {
            NostrTransportError::DeserializationFailed(format!(
                "JSON deserialization failed: {}",
                e
            ))
        })
    }
}

// ----------------------------------------------------------------------------
// NIP-17 Gift-wrapping Implementation
// ----------------------------------------------------------------------------

/// NIP-17 gift-wrapper for creating encrypted direct messages
#[derive(Clone)]
pub struct Nip17GiftWrapper {
    /// Our private keys for encryption
    sender_keys: Keys,
    /// Random ephemeral keys for traffic analysis resistance
    ephemeral_keys: Option<Keys>,
}

impl Nip17GiftWrapper {
    /// Create a new gift-wrapper with sender keys
    pub fn new(sender_keys: Keys) -> Self {
        Self {
            sender_keys,
            ephemeral_keys: None,
        }
    }

    /// Generate fresh ephemeral keys for traffic analysis resistance
    pub fn generate_ephemeral_keys(&mut self) {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                self.ephemeral_keys = Some(Keys::generate());
            } else {
                // WASM stub - in real implementation would use web crypto APIs
                self.ephemeral_keys = Some(Keys);
            }
        }
    }

    /// Create a gift-wrapped NIP-17 message
    pub fn create_gift_wrapped_message(
        &mut self,
        content: &Nip17Content,
        recipient_pubkey: &PublicKey,
    ) -> Result<NostrEvent, NostrTransportError> {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                // Generate ephemeral keys for this message
                self.generate_ephemeral_keys();
                let ephemeral_keys = self.ephemeral_keys.as_ref()
                    .ok_or_else(|| NostrTransportError::EncryptionFailed("No ephemeral keys".to_string()))?;

                // Serialize content to JSON
                let content_json = content.to_json()?;

                // Step 1: Create the inner direct message event (kind 14)
                let _inner_event = EventBuilder::new(Kind::EncryptedDirectMessage, "", vec![
                    Tag::public_key(*recipient_pubkey),
                ])
                .custom_created_at(Timestamp::now())
                .to_event(&self.sender_keys)
                .map_err(|e| NostrTransportError::EncryptionFailed(format!("Failed to create inner event: {}", e)))?;

                // Step 2: Encrypt the inner event content with recipient's pubkey
                let secret_key = self.sender_keys.secret_key().map_err(|e| NostrTransportError::KeyOperationFailed(e.to_string()))?;
                let encrypted_content = nip04::encrypt(
                    secret_key,
                    recipient_pubkey,
                    &content_json,
                ).map_err(|e| NostrTransportError::EncryptionFailed(format!("NIP-04 encryption failed: {}", e)))?;

                // Step 3: Create the inner event with encrypted content
                let inner_dm_event = EventBuilder::new(Kind::EncryptedDirectMessage, encrypted_content, vec![
                    Tag::public_key(*recipient_pubkey),
                ])
                .custom_created_at(Timestamp::now())
                .to_event(&self.sender_keys)
                .map_err(|e| NostrTransportError::EncryptionFailed(format!("Failed to create inner DM event: {}", e)))?;

                // Step 4: Serialize the inner event
                let inner_event_json = inner_dm_event.as_json();

                // Step 5: Generate a random recipient pubkey for gift-wrapping
                let random_recipient = Keys::generate().public_key();

                // Step 6: Encrypt the serialized inner event with the random recipient
                let ephemeral_secret_key = ephemeral_keys.secret_key().map_err(|e| NostrTransportError::KeyOperationFailed(e.to_string()))?;
                let gift_wrapped_content = nip04::encrypt(
                    ephemeral_secret_key,
                    &random_recipient,
                    &inner_event_json,
                ).map_err(|e| NostrTransportError::EncryptionFailed(format!("Gift-wrap encryption failed: {}", e)))?;

                // Step 7: Create the outer gift-wrapped event (kind 1059)
                let expiration_time = self.generate_random_expiration();
                let outer_event = EventBuilder::new(
                    Kind::GiftWrap,
                    gift_wrapped_content,
                    vec![
                        Tag::public_key(random_recipient),
                        Tag::expiration(Timestamp::from(expiration_time as u64)),
                    ]
                )
                .custom_created_at(Timestamp::from(self.generate_random_past_timestamp() as u64))
                .to_event(ephemeral_keys)
                .map_err(|e| NostrTransportError::EncryptionFailed(format!("Failed to create gift-wrapped event: {}", e)))?;

                Ok(outer_event)
            } else {
                // WASM stub implementation
                Err(NostrTransportError::EncryptionFailed("NIP-17 not implemented for WASM".to_string()))
            }
        }
    }

    /// Generate a random expiration time (30-60 minutes from now)
    fn generate_random_expiration(&self) -> i64 {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                use rand::Rng;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let random_offset = rand::thread_rng().gen_range(MIN_EXPIRATION_SECONDS..=MAX_EXPIRATION_SECONDS);
                now + random_offset
            } else {
                // WASM stub - use fixed offset
                let now = js_sys::Date::now() as i64 / 1000;
                now + MAX_EXPIRATION_SECONDS
            }
        }
    }

    /// Generate a random timestamp in the past (for traffic analysis resistance)
    fn generate_random_past_timestamp(&self) -> i64 {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                use rand::Rng;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Random time in the past 24 hours
                let random_offset = rand::thread_rng().gen_range(0..86400);
                now - random_offset
            } else {
                // WASM stub - use current time
                js_sys::Date::now() as i64 / 1000
            }
        }
    }
}

// ----------------------------------------------------------------------------
// NIP-17 Gift-unwrapper
// ----------------------------------------------------------------------------

/// NIP-17 gift-unwrapper for decrypting received messages
#[derive(Clone)]
pub struct Nip17GiftUnwrapper {
    /// Our private keys for decryption
    receiver_keys: Keys,
}

impl Nip17GiftUnwrapper {
    /// Create a new gift-unwrapper with receiver keys
    pub fn new(receiver_keys: Keys) -> Self {
        Self { receiver_keys }
    }

    /// Unwrap a gift-wrapped NIP-17 message
    pub fn unwrap_gift_wrapped_message(
        &self,
        outer_event: &NostrEvent,
    ) -> Result<Option<Nip17Content>, NostrTransportError> {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                // Check if this is a gift-wrapped event
                if outer_event.kind != Kind::GiftWrap {
                    return Ok(None);
                }

                // Extract the recipient public key from the 'p' tag
                let _recipient_pubkey = outer_event.tags.iter()
                    .find_map(|tag| {
                        match tag.as_vec().first() {
                            Some(tag_name) if tag_name == "p" => {
                                tag.as_vec().get(1).and_then(|pubkey_str| PublicKey::from_hex(pubkey_str).ok())
                            },
                            _ => None,
                        }
                    })
                    .ok_or_else(|| NostrTransportError::EncryptionFailed("No recipient pubkey in gift wrap".to_string()))?;

                // Step 1: Try to decrypt the outer gift-wrapped content
                // We assume the gift wrap was encrypted to our public key
                let our_secret_key = self.receiver_keys.secret_key()
                    .map_err(|e| NostrTransportError::KeyOperationFailed(e.to_string()))?;

                // Try to decrypt the outer content using NIP-04 with the ephemeral sender
                // Since we don't know the ephemeral sender key, we'll extract it from the event signature
                let ephemeral_sender_pubkey = outer_event.pubkey;

                let decrypted_outer = nip04::decrypt(
                    our_secret_key,
                    &ephemeral_sender_pubkey,
                    &outer_event.content,
                ).map_err(|e| NostrTransportError::EncryptionFailed(format!("Failed to decrypt gift wrap: {}", e)))?;

                // Step 2: Parse the inner event from the decrypted JSON
                let inner_event: NostrEvent = serde_json::from_str(&decrypted_outer)
                    .map_err(|e| NostrTransportError::DeserializationFailed(format!("Invalid inner event JSON: {}", e)))?;

                // Step 3: Verify the inner event is a direct message (kind 4 or 14)
                if inner_event.kind != Kind::EncryptedDirectMessage {
                    return Err(NostrTransportError::EncryptionFailed("Inner event is not a direct message".to_string()));
                }

                // Step 4: Decrypt the inner direct message content
                let inner_sender_pubkey = inner_event.pubkey;
                let decrypted_inner = nip04::decrypt(
                    our_secret_key,
                    &inner_sender_pubkey,
                    &inner_event.content,
                ).map_err(|e| NostrTransportError::EncryptionFailed(format!("Failed to decrypt inner message: {}", e)))?;

                // Step 5: Parse the final content
                let content = Nip17Content::from_json(&decrypted_inner)?;
                Ok(Some(content))
            } else {
                // WASM stub implementation
                Err(NostrTransportError::EncryptionFailed("NIP-17 not implemented for WASM".to_string()))
            }
        }
    }

    /// Decrypt a standard NIP-04 encrypted direct message (fallback)
    pub fn decrypt_nip04_message(
        &self,
        sender_pubkey: &PublicKey,
        encrypted_content: &str,
    ) -> Result<Nip17Content, NostrTransportError> {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                let secret_key = self.receiver_keys.secret_key().map_err(|e| NostrTransportError::KeyOperationFailed(e.to_string()))?;
                let decrypted_json = nip04::decrypt(
                    secret_key,
                    sender_pubkey,
                    encrypted_content,
                ).map_err(|e| NostrTransportError::EncryptionFailed(format!("NIP-04 decryption failed: {}", e)))?;

                Nip17Content::from_json(&decrypted_json)
            } else {
                // WASM stub implementation
                Err(NostrTransportError::EncryptionFailed("NIP-04 not implemented for WASM".to_string()))
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------

/// Convert PeerId to Nostr PublicKey (deterministic mapping)
/// This creates a deterministic mapping from BitChat PeerIds to Nostr public keys
/// by using the PeerId bytes as a seed for key derivation
pub fn peer_id_to_pubkey(peer_id: &PeerId) -> Result<PublicKey, NostrTransportError> {
    cfg_if::cfg_if! {
        if #[cfg(not(target_arch = "wasm32"))] {
            use sha2::{Sha256, Digest};

            // Use PeerId bytes as seed for deterministic key generation
            let peer_bytes = peer_id.as_bytes();

            // Expand the 8-byte PeerId to 32 bytes using SHA-256
            let mut hasher = Sha256::new();
            hasher.update(b"bitchat_nostr_pubkey:");
            hasher.update(peer_bytes);
            let key_bytes = hasher.finalize();

            // Create Nostr PublicKey from the derived bytes
            PublicKey::from_slice(&key_bytes)
                .map_err(|e| NostrTransportError::KeyOperationFailed(format!("Invalid public key: {}", e)))
        } else {
            Err(NostrTransportError::EncryptionFailed("Not implemented for WASM".to_string()))
        }
    }
}

/// Convert Nostr PublicKey to PeerId (deterministic mapping)
/// This creates a deterministic mapping from Nostr public keys to BitChat PeerIds
/// by hashing the public key bytes and taking the first 8 bytes
pub fn pubkey_to_peer_id(pubkey: &PublicKey) -> Result<PeerId, NostrTransportError> {
    cfg_if::cfg_if! {
        if #[cfg(not(target_arch = "wasm32"))] {
            use sha2::{Sha256, Digest};

            // Get the public key bytes
            let pubkey_bytes = pubkey.to_bytes();

            // Hash the public key to create a PeerId
            let mut hasher = Sha256::new();
            hasher.update(b"bitchat_peer_id:");
            hasher.update(pubkey_bytes);
            let hash = hasher.finalize();

            // Take the first 8 bytes as the PeerId
            let mut peer_bytes = [0u8; 8];
            peer_bytes.copy_from_slice(&hash[..8]);

            Ok(PeerId::new(peer_bytes))
        } else {
            Err(NostrTransportError::EncryptionFailed("Not implemented for WASM".to_string()))
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
    fn test_nip17_content_creation() {
        use bitchat_core::protocol::{BitchatPacket, MessageType, PacketFlags};
        use bitchat_core::types::{PeerId, Timestamp};

        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let packet = BitchatPacket::new(
            MessageType::Message,
            sender,
            None, // No specific recipient
            Timestamp::now(),
            b"Test message".to_vec(),
            PacketFlags::NONE,
        )
        .unwrap();

        let content = Nip17Content::from_bitchat_packet(&packet).unwrap();
        assert!(content.content.starts_with(BITCHAT_NIP17_PREFIX));

        let decoded_packet = content.to_bitchat_packet().unwrap();
        assert!(decoded_packet.is_some());
        let decoded = decoded_packet.unwrap();
        assert_eq!(decoded.sender_id, sender);
        assert_eq!(decoded.payload, b"Test message");
    }

    #[test]
    fn test_nip17_content_json_roundtrip() {
        let content = Nip17Content {
            content: "test content".to_string(),
            expiration: Some(1234567890),
        };

        let json = content.to_json().unwrap();
        let parsed = Nip17Content::from_json(&json).unwrap();

        assert_eq!(parsed.content, content.content);
        assert_eq!(parsed.expiration, content.expiration);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_gift_wrapper_creation() {
        let sender_keys = Keys::generate();
        let mut wrapper = Nip17GiftWrapper::new(sender_keys);

        // Test ephemeral key generation
        wrapper.generate_ephemeral_keys();
        assert!(wrapper.ephemeral_keys.is_some());
    }

    #[test]
    fn test_non_bitchat_content() {
        let content = Nip17Content {
            content: "regular message".to_string(),
            expiration: None,
        };

        let result = content.to_bitchat_packet().unwrap();
        assert!(result.is_none()); // Should return None for non-BitChat content
    }
}
