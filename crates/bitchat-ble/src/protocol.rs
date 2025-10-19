//! BLE protocol constants and utilities for BitChat

use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use bitchat_core::internal::{IdentityKeyPair, generate_fingerprint};
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
            generate_fingerprint(pub_key_array).to_peer_id();
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
/// This function uses cryptographic verification of peer announcements to prevent
/// man-in-the-middle attacks by verifying that the peer actually controls the claimed identity.
/// 
/// Returns Ok(Some(peer_id)) if verification succeeds, Ok(None) if verification fails,
/// or Err for parsing/validation errors.
#[allow(dead_code)]
pub fn extract_and_verify_peer_id(
    device_name: &str,
    advertising_data: &[u8],
    device_name_prefix: &str,
    max_age_seconds: u64,
) -> BitchatResult<Option<PeerId>> {
    // Parse the secure announcement from advertising data
    let announcement = PeerAnnouncement::from_bytes(advertising_data)
        .map_err(|_| BitchatError::InvalidPacket("Invalid or missing secure advertising data".into()))?;

    // Verify the announcement cryptographically
    if !announcement.verify(device_name, max_age_seconds)? {
        return Ok(None);
    }

    // Verify the device name matches the expected format
    let expected_name = generate_device_name(&announcement.peer_id, device_name_prefix);
    if device_name == expected_name {
        Ok(Some(announcement.peer_id))
    } else {
        // Device name doesn't match - reject for security
        Ok(None)
    }
}



/// Generate secure advertising data for this peer
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
            generate_fingerprint(&identity.public_key_bytes()).to_peer_id();
        let device_name = generate_device_name(&peer_id, "BitChat");

        // Generate secure advertising data
        let advertising_data = generate_advertising_data(peer_id, &identity, &device_name).unwrap();

        // Verify peer discovery through secure verification
        let discovered_peer = extract_and_verify_peer_id(&device_name, &advertising_data, "BitChat", 60).unwrap();
        assert_eq!(discovered_peer, Some(peer_id));

        // Test with wrong device name should fail
        let wrong_device_name = "BitChat-wrongpeerid";
        let discovered_peer = extract_and_verify_peer_id(wrong_device_name, &advertising_data, "BitChat", 60).unwrap();
        assert_eq!(discovered_peer, None);
    }


    #[test]
    fn test_device_name_generation() {
        let peer_id = PeerId::new([0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A]);
        let name = generate_device_name(&peer_id, "BitChat");
        assert_eq!(name, "BitChat-abcdef123456789a");
    }
}
