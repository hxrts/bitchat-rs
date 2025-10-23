use crate::{
    errors::BitchatError,
    protocol::{
        crypto::IdentityKeyPair,
        packet::{BitchatPacket, MessageType, PacketFlags},
        tlv::{TlvCodec, TlvEntry, TlvType},
    },
    types::{PeerId, Timestamp},
};
use alloc::string::ToString;
use alloc::{format, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// Announce packet payload containing peer information for discovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnouncePayload {
    /// The announcing peer's chosen nickname
    pub nickname: String,
    /// The peer's Noise static public key (Curve25519)
    pub noise_public_key: [u8; 32],
    /// The peer's Ed25519 signing public key
    pub signing_public_key: [u8; 32],
    /// Optional list of directly connected neighbors (full 32-byte keys for mesh routing)
    pub direct_neighbors: Option<Vec<[u8; 32]>>,
}

impl AnnouncePayload {
    /// Create a new announce payload
    pub fn new(
        nickname: String,
        noise_public_key: [u8; 32],
        signing_public_key: [u8; 32],
        direct_neighbors: Option<Vec<[u8; 32]>>,
    ) -> Result<Self, BitchatError> {
        // Validate nickname length (max 255 bytes for TLV encoding)
        if nickname.len() > 255 {
            return Err(BitchatError::TlvValueTooLarge(nickname.len()));
        }

        Ok(Self {
            nickname,
            noise_public_key,
            signing_public_key,
            direct_neighbors,
        })
    }

    /// Encode the announce payload using TLV format
    pub fn encode(&self) -> Result<Vec<u8>, BitchatError> {
        let mut codec = TlvCodec::new();

        // Add required TLV entries
        codec.add_entry(TlvEntry::nickname(&self.nickname)?);
        codec.add_entry(TlvEntry::noise_public_key(&self.noise_public_key));
        codec.add_entry(TlvEntry::signing_public_key(&self.signing_public_key));

        // Add optional direct neighbors
        if let Some(ref neighbors) = self.direct_neighbors {
            let neighbor_refs: Vec<&[u8; 32]> = neighbors.iter().collect();
            codec.add_entry(TlvEntry::direct_neighbors(&neighbor_refs));
        }

        Ok(codec.encode())
    }

    /// Decode an announce payload from TLV-encoded bytes
    pub fn decode(data: &[u8]) -> Result<Self, BitchatError> {
        let codec = TlvCodec::decode(data)?;

        // Validate required TLV types are present
        let required_types = [
            TlvType::Nickname,
            TlvType::NoisePublicKey,
            TlvType::SigningPublicKey,
        ];
        codec.validate_required(&required_types)?;

        // Extract required fields
        let nickname_entry = codec
            .find_entry(TlvType::Nickname)
            .ok_or(BitchatError::MissingRequiredTlv(TlvType::Nickname as u8))?;
        let nickname = nickname_entry.as_nickname()?;

        let noise_key_entry =
            codec
                .find_entry(TlvType::NoisePublicKey)
                .ok_or(BitchatError::MissingRequiredTlv(
                    TlvType::NoisePublicKey as u8,
                ))?;
        let noise_public_key = noise_key_entry.as_key()?;

        let signing_key_entry =
            codec
                .find_entry(TlvType::SigningPublicKey)
                .ok_or(BitchatError::MissingRequiredTlv(
                    TlvType::SigningPublicKey as u8,
                ))?;
        let signing_public_key = signing_key_entry.as_key()?;

        // Extract optional neighbors
        let direct_neighbors = codec
            .find_entry(TlvType::DirectNeighbors)
            .map(|entry| entry.as_neighbors())
            .transpose()?;

        Ok(AnnouncePayload {
            nickname,
            noise_public_key,
            signing_public_key,
            direct_neighbors,
        })
    }
}

/// Extensions for BitchatPacket to handle announce packets
impl BitchatPacket {
    /// Create a new announce packet
    pub fn create_announce(
        sender_id: PeerId,
        nickname: String,
        noise_public_key: [u8; 32],
        identity_keypair: &IdentityKeyPair,
        direct_neighbors: Option<Vec<[u8; 32]>>,
        timestamp: Timestamp,
    ) -> Result<Self, BitchatError> {
        // Create announce payload
        let payload = AnnouncePayload::new(
            nickname,
            noise_public_key,
            identity_keypair.public_key_bytes(),
            direct_neighbors,
        )?;

        // Encode the payload
        let encoded_payload = payload.encode()?;

        // Create the packet (signature will be added later during signing)
        let mut packet = BitchatPacket::new(
            MessageType::Announce,
            sender_id,
            None, // Broadcast message - no specific recipient
            timestamp,
            encoded_payload,
            PacketFlags::NONE,
        )?;

        // Sign the packet
        packet.sign(identity_keypair)?;

        Ok(packet)
    }

    /// Parse an announce packet and extract the payload
    pub fn parse_announce(&self) -> Result<AnnouncePayload, BitchatError> {
        // Verify this is an announce packet
        if self.message_type() != MessageType::Announce {
            return Err(BitchatError::invalid_packet(format!(
                "Expected announce packet, got {:?}",
                self.message_type()
            )));
        }

        // Decode the TLV payload
        AnnouncePayload::decode(self.payload())
    }

    /// Verify an announce packet's signature and extract payload
    pub fn verify_and_parse_announce(&self) -> Result<AnnouncePayload, BitchatError> {
        // First parse the payload to get the signing key
        let payload = self.parse_announce()?;

        // Verify the signature using the signing key from the payload
        self.verify_signature(&payload.signing_public_key)?;

        Ok(payload)
    }
}

/// Peer information extracted from an announce packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// The peer's ID (derived from their Noise public key)
    pub peer_id: PeerId,
    /// The peer's chosen nickname
    pub nickname: String,
    /// The peer's Noise static public key
    pub noise_public_key: [u8; 32],
    /// The peer's Ed25519 signing public key
    pub signing_public_key: [u8; 32],
    /// Directly connected neighbors (if provided)
    pub direct_neighbors: Option<Vec<[u8; 32]>>,
    /// When this peer was last seen
    pub last_seen: Timestamp,
}

impl DiscoveredPeer {
    /// Create a discovered peer from an announce packet
    pub fn from_announce_packet(
        packet: &BitchatPacket,
        timestamp: Timestamp,
    ) -> Result<Self, BitchatError> {
        let payload = packet.verify_and_parse_announce()?;

        // Derive peer ID from Noise public key
        let peer_id = PeerId::from_noise_key(&payload.noise_public_key);

        // Verify the sender ID matches the derived peer ID
        if packet.sender_id() != peer_id {
            return Err(BitchatError::invalid_packet(
                "Sender ID does not match Noise public key",
            ));
        }

        Ok(DiscoveredPeer {
            peer_id,
            nickname: payload.nickname,
            noise_public_key: payload.noise_public_key,
            signing_public_key: payload.signing_public_key,
            direct_neighbors: payload.direct_neighbors,
            last_seen: timestamp,
        })
    }

    /// Get the cryptographic fingerprint of this peer
    pub fn fingerprint(&self) -> String {
        crate::protocol::crypto::generate_fingerprint(self.noise_public_key).to_string()
    }

    /// Check if this peer was recently seen (within the given duration)
    pub fn is_recent(&self, current_time: Timestamp, max_age_ms: u64) -> bool {
        current_time
            .as_millis()
            .saturating_sub(self.last_seen.as_millis())
            <= max_age_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::crypto::NoiseKeyPair;

    fn create_test_keypairs() -> (NoiseKeyPair, IdentityKeyPair) {
        let noise_keypair = NoiseKeyPair::generate();
        let identity_keypair = IdentityKeyPair::generate().unwrap();
        (noise_keypair, identity_keypair)
    }

    #[test]
    fn test_announce_payload_creation() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();

        let payload = AnnouncePayload::new(
            "alice".to_string(),
            noise_keypair.public_key_bytes(),
            identity_keypair.public_key_bytes(),
            None,
        )
        .unwrap();

        assert_eq!(payload.nickname, "alice");
        assert_eq!(payload.noise_public_key, noise_keypair.public_key_bytes());
        assert_eq!(
            payload.signing_public_key,
            identity_keypair.public_key_bytes()
        );
        assert!(payload.direct_neighbors.is_none());
    }

    #[test]
    fn test_announce_payload_encode_decode() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();

        let original_payload = AnnouncePayload::new(
            "test_peer".to_string(),
            noise_keypair.public_key_bytes(),
            identity_keypair.public_key_bytes(),
            Some(vec![[0x42; 32], [0x43; 32]]),
        )
        .unwrap();

        let encoded = original_payload.encode().unwrap();
        let decoded_payload = AnnouncePayload::decode(&encoded).unwrap();

        assert_eq!(original_payload, decoded_payload);
    }

    #[test]
    fn test_announce_packet_creation() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();
        let peer_id = PeerId::from_noise_key(&noise_keypair.public_key_bytes());
        let timestamp = Timestamp::now();

        let packet = BitchatPacket::create_announce(
            peer_id,
            "test_peer".to_string(),
            noise_keypair.public_key_bytes(),
            &identity_keypair,
            None,
            timestamp,
        )
        .unwrap();

        assert_eq!(packet.message_type(), MessageType::Announce);
        assert_eq!(packet.sender_id(), peer_id);
        assert!(packet.recipient_id().is_none());

        // Verify we can parse it back
        let payload = packet.parse_announce().unwrap();
        assert_eq!(payload.nickname, "test_peer");
        assert_eq!(payload.noise_public_key, noise_keypair.public_key_bytes());
    }

    #[test]
    fn test_discovered_peer_from_packet() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();
        let peer_id = PeerId::from_noise_key(&noise_keypair.public_key_bytes());
        let timestamp = Timestamp::now();

        let packet = BitchatPacket::create_announce(
            peer_id,
            "discovered_peer".to_string(),
            noise_keypair.public_key_bytes(),
            &identity_keypair,
            Some(vec![[0x11; 32]]),
            timestamp,
        )
        .unwrap();

        let discovered = DiscoveredPeer::from_announce_packet(&packet, timestamp).unwrap();

        assert_eq!(discovered.peer_id, peer_id);
        assert_eq!(discovered.nickname, "discovered_peer");
        assert_eq!(
            discovered.noise_public_key,
            noise_keypair.public_key_bytes()
        );
        assert_eq!(
            discovered.signing_public_key,
            identity_keypair.public_key_bytes()
        );
        assert_eq!(discovered.direct_neighbors, Some(vec![[0x11; 32]]));
        assert_eq!(discovered.last_seen, timestamp);
    }

    #[test]
    fn test_nickname_too_long() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();
        let long_nickname = "a".repeat(256); // Too long for TLV encoding

        let result = AnnouncePayload::new(
            long_nickname,
            noise_keypair.public_key_bytes(),
            identity_keypair.public_key_bytes(),
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_peer_fingerprint() {
        let (noise_keypair, identity_keypair) = create_test_keypairs();
        let peer_id = PeerId::from_noise_key(&noise_keypair.public_key_bytes());
        let timestamp = Timestamp::now();

        let packet = BitchatPacket::create_announce(
            peer_id,
            "fingerprint_test".to_string(),
            noise_keypair.public_key_bytes(),
            &identity_keypair,
            None,
            timestamp,
        )
        .unwrap();

        let discovered = DiscoveredPeer::from_announce_packet(&packet, timestamp).unwrap();
        let fingerprint = discovered.fingerprint();

        // Fingerprint should be a hex string representing the SHA-256 hash
        assert_eq!(fingerprint.len(), 64); // 32 bytes * 2 hex chars
        assert!(fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
