//! BLE protocol constants and utilities for BitChat

use bitchat_core::{PeerId, BitchatError, Result as BitchatResult};
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
// Protocol Utilities
// ----------------------------------------------------------------------------

/// Generate a BLE-compatible device name for this peer
pub fn generate_device_name(peer_id: &PeerId, prefix: &str) -> String {
    format!("{}-{}", prefix, hex::encode(peer_id.as_bytes()))
}

/// SECURITY WARNING: Extract peer ID from device name (INSECURE)
/// 
/// This function extracts peer IDs from BLE device names, which is insecure
/// as device names can be easily spoofed by malicious actors. This could lead
/// to man-in-the-middle attacks if users don't verify fingerprints.
/// 
/// Format: "BitChat-<hex_peer_id>"
/// 
/// TODO: Replace with cryptographically secure peer discovery using signed
/// announcements in BLE advertising data.
pub fn extract_peer_id_from_name(name: &str, device_name_prefix: &str) -> Option<PeerId> {
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

/// Verify cryptographic proof of peer identity (FUTURE IMPLEMENTATION)
/// 
/// This function would verify that a peer actually controls the claimed peer ID
/// by validating a cryptographic signature in the BLE advertising data.
/// 
/// The signature should be created using the peer's Ed25519 signing key over
/// a challenge that includes:
/// - The claimed peer ID
/// - A timestamp to prevent replay attacks  
/// - The BLE device's MAC address or other unique identifier
/// 
/// This prevents peer ID spoofing and man-in-the-middle attacks.
pub fn verify_peer_identity_proof(
    _claimed_peer_id: &PeerId,
    _advertising_data: &[u8],
    _public_key: &[u8; 32],
) -> BitchatResult<bool> {
    // TODO: Implement cryptographic verification of peer identity
    // This should:
    // 1. Extract signature from advertising data
    // 2. Verify signature against claimed peer ID and timestamp
    // 3. Check timestamp is recent (within last 60 seconds)
    // 4. Verify the signing key matches the peer ID
    
    Err(BitchatError::InvalidPacket(
        "Cryptographic peer verification not yet implemented".into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_extraction() {
        // Valid peer ID
        let name = "BitChat-0102030405060708";
        let peer_id = extract_peer_id_from_name(name, "BitChat");
        assert!(peer_id.is_some());
        assert_eq!(peer_id.unwrap(), PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));

        // Invalid format
        let name = "SomeOtherDevice";
        let peer_id = extract_peer_id_from_name(name, "BitChat");
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