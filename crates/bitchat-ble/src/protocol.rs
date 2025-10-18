//! BLE protocol constants and utilities for BitChat

use bitchat_core::crypto::IdentityKeyPair;
use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ----------------------------------------------------------------------------
// BLE Service and Characteristic UUIDs
// ----------------------------------------------------------------------------

/// BitChat BLE service UUID
pub const BITCHAT_SERVICE_UUID: Uuid = Uuid::from_u128(0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E);

/// BitChat BLE characteristic for sending data
pub const BITCHAT_TX_CHARACTERISTIC_UUID: Uuid =
    Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);

/// BitChat BLE characteristic for receiving data
pub const BITCHAT_RX_CHARACTERISTIC_UUID: Uuid =
    Uuid::from_u128(0x6E400003_B5A3_F393_E0A9_E50E24DCCA9E);

// ----------------------------------------------------------------------------
// Secure Peer Discovery Protocol
// ----------------------------------------------------------------------------

/// Secure peer announcement included in BLE advertising data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAnnouncement {
    /// The peer's identity (8-byte truncated from public key)
    pub peer_id: PeerId,
    /// Timestamp when this announcement was created (Unix timestamp in seconds)
    pub timestamp: u64,
    /// Ed25519 signature over (peer_id || timestamp || device_name)
    pub signature: Vec<u8>,
    /// The peer's full Ed25519 public key for verification
    pub public_key: Vec<u8>,
}

impl PeerAnnouncement {
    /// Create a new signed peer announcement
    #[allow(dead_code)]
    pub fn new(peer_id: PeerId, identity: &IdentityKeyPair, device_name: &str) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create challenge data: peer_id || timestamp || device_name
        let mut challenge = Vec::with_capacity(8 + 8 + device_name.len());
        challenge.extend_from_slice(peer_id.as_bytes());
        challenge.extend_from_slice(&timestamp.to_be_bytes());
        challenge.extend_from_slice(device_name.as_bytes());

        let signature = identity.sign(&challenge).to_vec();
        let public_key = identity.public_key_bytes().to_vec();

        Self {
            peer_id,
            timestamp,
            signature,
            public_key,
        }
    }

    /// Verify this announcement against the claimed identity
    #[allow(dead_code)]
    pub fn verify(&self, device_name: &str, max_age_seconds: u64) -> BitchatResult<bool> {
        // Check timestamp freshness
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if current_time.saturating_sub(self.timestamp) > max_age_seconds {
            return Ok(false);
        }

        // Verify the public key matches the peer ID
        if self.public_key.len() != 32 {
            return Ok(false);
        }
        let pub_key_array: [u8; 32] = self
            .public_key
            .as_slice()
            .try_into()
            .map_err(|_| BitchatError::InvalidPacket("Invalid public key length".into()))?;
        let expected_peer_id =
            bitchat_core::crypto::generate_fingerprint(&pub_key_array).to_peer_id();
        if expected_peer_id != self.peer_id {
            return Ok(false);
        }

        // Reconstruct challenge data and verify signature
        let mut challenge = Vec::with_capacity(8 + 8 + device_name.len());
        challenge.extend_from_slice(self.peer_id.as_bytes());
        challenge.extend_from_slice(&self.timestamp.to_be_bytes());
        challenge.extend_from_slice(device_name.as_bytes());

        // Convert signature Vec to array
        if self.signature.len() != 64 {
            return Ok(false);
        }
        let sig_array: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| BitchatError::InvalidPacket("Invalid signature length".into()))?;

        match IdentityKeyPair::verify(&pub_key_array, &challenge, &sig_array) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Serialize announcement to bytes for BLE advertising
    #[allow(dead_code)]
    pub fn to_bytes(&self) -> BitchatResult<Vec<u8>> {
        bincode::serialize(self).map_err(BitchatError::Serialization)
    }

    /// Deserialize announcement from BLE advertising bytes
    #[allow(dead_code)]
    pub fn from_bytes(data: &[u8]) -> BitchatResult<Self> {
        bincode::deserialize(data).map_err(BitchatError::Serialization)
    }
}

// ----------------------------------------------------------------------------
// Protocol Utilities
// ----------------------------------------------------------------------------

/// Generate a BLE-compatible device name for this peer
pub fn generate_device_name(peer_id: &PeerId, prefix: &str) -> String {
    format!("{}-{}", prefix, hex::encode(peer_id.as_bytes()))
}

/// Extract and verify peer ID from secure BLE advertising data
///
/// This function replaces the insecure device name extraction with cryptographic
/// verification of peer announcements. It prevents man-in-the-middle attacks by
/// verifying that the peer actually controls the claimed identity.
#[allow(dead_code)]
pub fn extract_and_verify_peer_id(
    device_name: &str,
    advertising_data: &[u8],
    device_name_prefix: &str,
    max_age_seconds: u64,
) -> BitchatResult<Option<PeerId>> {
    // Try to parse the secure announcement from advertising data
    match PeerAnnouncement::from_bytes(advertising_data) {
        Ok(announcement) => {
            // Verify the announcement cryptographically
            match announcement.verify(device_name, max_age_seconds) {
                Ok(true) => {
                    // Verify the device name matches the expected format
                    let expected_name =
                        generate_device_name(&announcement.peer_id, device_name_prefix);
                    if device_name == expected_name {
                        Ok(Some(announcement.peer_id))
                    } else {
                        // Device name doesn't match - possible attack
                        Ok(None)
                    }
                }
                Ok(false) => {
                    // Signature verification failed
                    Ok(None)
                }
                Err(e) => Err(e),
            }
        }
        Err(_) => {
            // Fall back to insecure name-based extraction with warning
            // This maintains backward compatibility but logs security warnings
            tracing::warn!(
                "Falling back to insecure peer ID extraction for device: {}. \
                 This device may not support secure peer discovery.",
                device_name
            );
            Ok(extract_peer_id_from_name_legacy(
                device_name,
                device_name_prefix,
            ))
        }
    }
}

/// Legacy insecure peer ID extraction (deprecated)
///
/// This function is kept for backward compatibility but should not be used
/// for new implementations. Use extract_and_verify_peer_id instead.
#[deprecated(note = "Use extract_and_verify_peer_id for secure peer discovery")]
pub fn extract_peer_id_from_name(name: &str, device_name_prefix: &str) -> Option<PeerId> {
    extract_peer_id_from_name_legacy(name, device_name_prefix)
}

fn extract_peer_id_from_name_legacy(name: &str, device_name_prefix: &str) -> Option<PeerId> {
    if let Some(hex_part) = name.strip_prefix(&format!("{}-", device_name_prefix)) {
        if hex_part.len() == 16 {
            // 8 bytes = 16 hex chars
            if let Ok(bytes) = hex::decode(hex_part) {
                if bytes.len() == 8 {
                    let mut peer_id_bytes = [0u8; 8];
                    peer_id_bytes.copy_from_slice(&bytes);
                    return Some(PeerId::new(peer_id_bytes));
                }
            }
        }
    }
    None
}

/// Verify cryptographic proof of peer identity
///
/// This function verifies that a peer actually controls the claimed peer ID
/// by validating a cryptographic signature in the BLE advertising data.
///
/// Returns Ok(true) if the proof is valid, Ok(false) if invalid, or Err for parsing errors.
#[allow(dead_code)]
pub fn verify_peer_identity_proof(
    claimed_peer_id: &PeerId,
    advertising_data: &[u8],
    device_name: &str,
) -> BitchatResult<bool> {
    match PeerAnnouncement::from_bytes(advertising_data) {
        Ok(announcement) => {
            // Check that the announcement claims the expected peer ID
            if announcement.peer_id != *claimed_peer_id {
                return Ok(false);
            }

            // Verify the cryptographic proof with 60-second max age
            announcement.verify(device_name, 60)
        }
        Err(_) => {
            // Could not parse announcement data - invalid format
            Ok(false)
        }
    }
}

/// Generate secure advertising data for this peer
#[allow(dead_code)]
pub fn generate_advertising_data(
    peer_id: PeerId,
    identity: &IdentityKeyPair,
    device_name: &str,
) -> BitchatResult<Vec<u8>> {
    let announcement = PeerAnnouncement::new(peer_id, identity, device_name);
    announcement.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_peer_discovery() {
        // Generate a test identity
        let identity = IdentityKeyPair::generate().unwrap();
        let peer_id =
            bitchat_core::crypto::generate_fingerprint(&identity.public_key_bytes()).to_peer_id();
        let device_name = generate_device_name(&peer_id, "BitChat");

        // Generate secure advertising data
        let advertising_data = generate_advertising_data(peer_id, &identity, &device_name).unwrap();

        // Verify the peer identity
        let is_valid =
            verify_peer_identity_proof(&peer_id, &advertising_data, &device_name).unwrap();
        assert!(is_valid);

        // Test with wrong peer ID should fail
        let wrong_peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let is_valid =
            verify_peer_identity_proof(&wrong_peer_id, &advertising_data, &device_name).unwrap();
        assert!(!is_valid);
    }

    #[test]
    fn test_legacy_peer_id_extraction() {
        // Valid peer ID
        let name = "BitChat-0102030405060708";
        let peer_id = extract_peer_id_from_name_legacy(name, "BitChat");
        assert!(peer_id.is_some());
        assert_eq!(peer_id.unwrap(), PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));

        // Invalid format
        let name = "SomeOtherDevice";
        let peer_id = extract_peer_id_from_name_legacy(name, "BitChat");
        assert!(peer_id.is_none());

        // Invalid hex
        let name = "BitChat-invalid_hex";
        let peer_id = extract_peer_id_from_name(name, "BitChat");
        assert!(peer_id.is_none());
    }

    #[test]
    fn test_device_name_generation() {
        let peer_id = PeerId::new([0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A]);
        let name = generate_device_name(&peer_id, "BitChat");
        assert_eq!(name, "BitChat-abcdef123456789a");
    }
}
