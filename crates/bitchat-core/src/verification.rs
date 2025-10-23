//! QR-based Peer Verification Implementation
//!
//! This module implements the QR-based Peer Verification feature from the roadmap,
//! providing cryptographic identity verification through QR code exchange.
//!
//! The implementation follows the canonical patterns with Ed25519 signature-based
//! proof of identity and challenge-response verification protocol.

use hashbrown::HashMap;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use rand_core::RngCore;

use crate::types::{PeerId, Timestamp};
use crate::protocol::crypto::{IdentityKeyPair, generate_fingerprint};
use crate::{BitchatError, Result as BitchatResult};

/// Configuration for QR-based peer verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Challenge expiration time
    pub challenge_timeout: Duration,
    /// Maximum number of pending challenges
    pub max_pending_challenges: usize,
    /// QR code dimensions for generation
    pub qr_dimensions: (u32, u32),
    /// Whether to include nickname in QR data
    pub include_nickname: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            challenge_timeout: Duration::from_secs(300), // 5 minutes
            max_pending_challenges: 10,
            qr_dimensions: (256, 256),
            include_nickname: true,
        }
    }
}

/// QR code data structure for peer verification
/// Contains public information only - never exposes private keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationQR {
    /// Protocol version for future compatibility
    pub version: u8,
    /// Noise protocol public key for the peer
    pub noise_public_key: [u8; 32],
    /// Ed25519 public key for signing verification (PUBLIC key only)
    pub signing_public_key: [u8; 32],
    /// Optional nickname for user-friendly display
    pub nickname: Option<String>,
    /// Timestamp when QR was generated
    pub timestamp: Timestamp,
    /// Random nonce for this verification session
    pub nonce: [u8; 32],
    /// Self-signature proving ownership of private key  
    pub signature: Vec<u8>,
}

impl VerificationQR {
    /// Create a new verification QR with self-signature
    pub fn new(
        noise_public_key: [u8; 32],
        signing_keypair: &IdentityKeyPair,
        nickname: Option<String>,
        nonce: [u8; 32],
    ) -> BitchatResult<Self> {
        let timestamp = Timestamp::now();
        
        // Create the QR data structure without signature
        let mut qr_data = Self {
            version: 1,
            noise_public_key,
            signing_public_key: signing_keypair.public_key_bytes(),
            nickname,
            timestamp,
            nonce,
            signature: Vec::new(), // Placeholder
        };
        
        // Sign the QR data to prove ownership of private key
        let data_to_sign = qr_data.signable_data()?;
        let signature = signing_keypair.sign(&data_to_sign);
        qr_data.signature = signature.to_vec();
        
        Ok(qr_data)
    }
    
    /// Get the data that should be signed for verification
    fn signable_data(&self) -> BitchatResult<Vec<u8>> {
        // Sign everything except the signature field itself
        let data_for_signing = (
            self.version,
            &self.noise_public_key,
            &self.signing_public_key,
            &self.nickname,
            self.timestamp,
            &self.nonce,
        );
        
        bincode::serialize(&data_for_signing)
            .map_err(|e| BitchatError::invalid_packet(format!("Failed to serialize QR data: {}", e)))
    }
    
    /// Verify the self-signature on this QR code
    pub fn verify_self_signature(&self) -> BitchatResult<bool> {
        if self.signature.len() != 64 {
            return Ok(false);
        }
        let data_to_verify = self.signable_data()?;
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&self.signature);
        Ok(IdentityKeyPair::verify(&self.signing_public_key, &data_to_verify, &sig_array).is_ok())
    }
    
    /// Convert to URI format for QR encoding
    pub fn to_uri(&self) -> BitchatResult<String> {
        let serialized = bincode::serialize(self)
            .map_err(|e| BitchatError::invalid_packet(format!("Failed to serialize QR: {}", e)))?;
        
        use base64::{engine::general_purpose, Engine as _};
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(&serialized);
        Ok(format!("bitchat://verify?data={}", encoded))
    }
    
    /// Parse from URI format
    pub fn from_uri(uri: &str) -> BitchatResult<Self> {
        if !uri.starts_with("bitchat://verify?data=") {
            return Err(BitchatError::invalid_packet("Invalid verification URI format".to_string()));
        }
        
        let encoded_data = &uri[22..]; // Skip "bitchat://verify?data="
        use base64::{engine::general_purpose, Engine as _};
        let serialized = general_purpose::URL_SAFE_NO_PAD.decode(encoded_data)
            .map_err(|e| BitchatError::invalid_packet(format!("Invalid base64: {}", e)))?;
        
        let qr_data: Self = bincode::deserialize(&serialized)
            .map_err(|e| BitchatError::invalid_packet(format!("Failed to deserialize QR: {}", e)))?;
        
        // Verify the self-signature
        if !qr_data.verify_self_signature()? {
            return Err(BitchatError::invalid_packet("Invalid QR self-signature".to_string()));
        }
        
        Ok(qr_data)
    }
    
    /// Check if this QR has expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        let now = Timestamp::now();
        let elapsed = if now.as_millis() >= self.timestamp.as_millis() {
            Duration::from_millis(now.as_millis() - self.timestamp.as_millis())
        } else {
            Duration::ZERO
        };
        elapsed > timeout
    }
    
    /// Get the peer ID for this verification QR
    pub fn peer_id(&self) -> PeerId {
        PeerId::from_noise_key(&self.noise_public_key)
    }
}

/// Pending challenge for peer verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingChallenge {
    /// Challenge nonce A (from our QR)
    pub nonce_a: [u8; 32],
    /// Challenge nonce B (from their QR)
    pub nonce_b: [u8; 32],
    /// When this challenge expires
    pub expires_at: Timestamp,
    /// Peer's verification QR data
    pub peer_qr: VerificationQR,
}

impl PendingChallenge {
    /// Create challenge data for response
    pub fn challenge_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.nonce_a);
        data.extend_from_slice(&self.nonce_b);
        data
    }
    
    /// Check if this challenge has expired
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }
}

/// Response to a verification challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResponse {
    /// The challenge being responded to
    pub challenge_nonce_a: [u8; 32],
    pub challenge_nonce_b: [u8; 32],
    /// Signature proving possession of private key
    pub signature: Vec<u8>,
    /// Responder's verification QR
    pub responder_qr: VerificationQR,
}

impl VerificationResponse {
    /// Create a new verification response
    pub fn new(
        challenge: &PendingChallenge,
        signing_keypair: &IdentityKeyPair,
        responder_qr: VerificationQR,
    ) -> BitchatResult<Self> {
        let challenge_data = challenge.challenge_data();
        let signature = signing_keypair.sign(&challenge_data);
        
        Ok(Self {
            challenge_nonce_a: challenge.nonce_a,
            challenge_nonce_b: challenge.nonce_b,
            signature: signature.to_vec(),
            responder_qr,
        })
    }
    
    /// Verify this response against a challenge
    pub fn verify(&self, challenge: &PendingChallenge) -> BitchatResult<bool> {
        // Check nonces match
        if self.challenge_nonce_a != challenge.nonce_a || self.challenge_nonce_b != challenge.nonce_b {
            return Ok(false);
        }
        
        // Verify the signature
        if self.signature.len() != 64 {
            return Ok(false);
        }
        let challenge_data = challenge.challenge_data();
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&self.signature);
        Ok(IdentityKeyPair::verify(&self.responder_qr.signing_public_key, &challenge_data, &sig_array).is_ok())
    }
}

/// Result of a verification attempt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationResult {
    /// Verification successful
    Success {
        peer_id: PeerId,
        nickname: Option<String>,
        verified_at: Timestamp,
    },
    /// Verification failed
    Failed {
        peer_id: PeerId,
        reason: String,
    },
    /// Challenge expired
    Expired {
        peer_id: PeerId,
    },
    /// Invalid signature
    InvalidSignature {
        peer_id: PeerId,
    },
}

/// Main verification service for managing QR-based peer verification
pub struct VerificationService {
    /// Configuration
    config: VerificationConfig,
    /// Pending verification challenges
    pending_challenges: HashMap<PeerId, PendingChallenge>,
    /// Our signing keypair for verification
    signing_keypair: IdentityKeyPair,
    /// Our noise public key
    noise_public_key: [u8; 32],
    /// Our nickname
    nickname: Option<String>,
}

impl VerificationService {
    /// Create a new verification service
    pub fn new(
        config: VerificationConfig,
        signing_keypair: IdentityKeyPair,
        noise_public_key: [u8; 32],
        nickname: Option<String>,
    ) -> Self {
        Self {
            config,
            pending_challenges: HashMap::new(),
            signing_keypair,
            noise_public_key,
            nickname,
        }
    }
    
    /// Generate a QR code for peer verification
    pub fn generate_verification_qr(&self) -> BitchatResult<VerificationQR> {
        // Generate random nonce for this verification session
        let mut nonce = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut nonce);
        
        VerificationQR::new(
            self.noise_public_key,
            &self.signing_keypair,
            self.nickname.clone(),
            nonce,
        )
    }
    
    /// Process a scanned QR code and initiate verification
    pub fn process_scanned_qr(&mut self, qr_uri: &str) -> BitchatResult<PendingChallenge> {
        // Parse the QR code
        let peer_qr = VerificationQR::from_uri(qr_uri)?;
        
        // Check if QR has expired
        if peer_qr.is_expired(self.config.challenge_timeout) {
            return Err(BitchatError::invalid_packet("QR code has expired".to_string()));
        }
        
        // Clean up expired challenges
        self.cleanup_expired_challenges();
        
        // Check challenge limit
        if self.pending_challenges.len() >= self.config.max_pending_challenges {
            return Err(BitchatError::invalid_packet("Too many pending challenges".to_string()));
        }
        
        // Generate our own nonce for the challenge
        let mut our_nonce = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut our_nonce);
        
        let expires_at = Timestamp::new(
            Timestamp::now().as_millis() + self.config.challenge_timeout.as_millis() as u64
        );
        let challenge = PendingChallenge {
            nonce_a: peer_qr.nonce,
            nonce_b: our_nonce,
            expires_at,
            peer_qr: peer_qr.clone(),
        };
        
        let peer_id = peer_qr.peer_id();
        self.pending_challenges.insert(peer_id, challenge.clone());
        
        Ok(challenge)
    }
    
    /// Create a response to a verification challenge
    pub fn create_challenge_response(&self, challenge: &PendingChallenge) -> BitchatResult<VerificationResponse> {
        // Generate our QR for the response
        let our_qr = self.generate_verification_qr()?;
        
        VerificationResponse::new(challenge, &self.signing_keypair, our_qr)
    }
    
    /// Process a verification response
    pub fn process_verification_response(&mut self, response: VerificationResponse) -> BitchatResult<VerificationResult> {
        let peer_id = response.responder_qr.peer_id();
        
        // Find the corresponding challenge
        let challenge = self.pending_challenges.remove(&peer_id)
            .ok_or_else(|| BitchatError::invalid_packet("No pending challenge for peer".to_string()))?;
        
        // Check if challenge expired
        if challenge.is_expired() {
            return Ok(VerificationResult::Expired { peer_id });
        }
        
        // Verify the response
        match response.verify(&challenge) {
            Ok(true) => {
                Ok(VerificationResult::Success {
                    peer_id,
                    nickname: response.responder_qr.nickname,
                    verified_at: Timestamp::now(),
                })
            }
            Ok(false) => {
                Ok(VerificationResult::InvalidSignature { peer_id })
            }
            Err(e) => {
                Ok(VerificationResult::Failed {
                    peer_id,
                    reason: format!("Verification error: {:?}", e),
                })
            }
        }
    }
    
    /// Clean up expired challenges
    pub fn cleanup_expired_challenges(&mut self) {
        self.pending_challenges.retain(|_peer_id, challenge| !challenge.is_expired());
    }
    
    /// Get pending challenge for a peer
    pub fn get_pending_challenge(&self, peer_id: &PeerId) -> Option<&PendingChallenge> {
        self.pending_challenges.get(peer_id)
    }
    
    /// Get all pending challenges
    pub fn get_pending_challenges(&self) -> &HashMap<PeerId, PendingChallenge> {
        &self.pending_challenges
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: VerificationConfig) {
        self.config = config;
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &VerificationConfig {
        &self.config
    }
}

// Feature-gated QR code generation functionality
#[cfg(feature = "qr-generation")]
pub mod qr_generation {
    use super::*;
    use qrcode::{render::svg, QrCode, Color};
    
    /// Generate SVG QR code for verification
    pub fn generate_qr_svg(qr_data: &VerificationQR, config: &VerificationConfig) -> BitchatResult<String> {
        let uri = qr_data.to_uri()?;
        
        let qr_code = QrCode::new(&uri)
            .map_err(|e| BitchatError::invalid_packet(format!("QR generation failed: {}", e)))?;
        
        let svg = qr_code
            .render::<svg::Color>()
            .min_dimensions(config.qr_dimensions.0, config.qr_dimensions.1)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#FFFFFF"))
            .build();
        
        Ok(svg)
    }
    
    /// Generate PNG QR code bytes for verification
    #[cfg(feature = "qr-png")]
    pub fn generate_qr_png(qr_data: &VerificationQR, config: &VerificationConfig) -> BitchatResult<Vec<u8>> {
        use qrcode::render::png;
        
        let uri = qr_data.to_uri()?;
        
        let qr_code = QrCode::new(&uri)
            .map_err(|e| BitchatError::invalid_packet(format!("QR generation failed: {}", e)))?;
        
        let png_data = qr_code
            .render::<png::Color>()
            .min_dimensions(config.qr_dimensions.0, config.qr_dimensions.1)
            .dark_color(png::Color(0, 0, 0))
            .light_color(png::Color(255, 255, 255))
            .build();
        
        Ok(png_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create_test_keypair() -> IdentityKeyPair {
        IdentityKeyPair::generate().unwrap()
    }

    #[test]
    fn test_verification_qr_creation() {
        let keypair = create_test_keypair();
        let noise_pubkey = [1u8; 32]; // Mock noise public key
        let mut nonce = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut nonce);
        
        let qr = VerificationQR::new(
            noise_pubkey,
            &keypair,
            Some("Alice".to_string()),
            nonce,
        ).unwrap();
        
        assert_eq!(qr.version, 1);
        assert_eq!(qr.nickname, Some("Alice".to_string()));
        assert!(qr.verify_self_signature().unwrap());
    }
    
    #[test]
    fn test_qr_uri_roundtrip() {
        let keypair = create_test_keypair();
        let noise_pubkey = [2u8; 32]; // Mock noise public key
        let mut nonce = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut nonce);
        
        let original_qr = VerificationQR::new(
            noise_pubkey,
            &keypair,
            Some("Bob".to_string()),
            nonce,
        ).unwrap();
        
        let uri = original_qr.to_uri().unwrap();
        assert!(uri.starts_with("bitchat://verify?data="));
        
        let parsed_qr = VerificationQR::from_uri(&uri).unwrap();
        assert_eq!(parsed_qr.version, original_qr.version);
        assert_eq!(parsed_qr.nickname, original_qr.nickname);
        assert_eq!(parsed_qr.nonce, original_qr.nonce);
    }
    
    #[test]
    fn test_verification_service_flow() {
        let alice_keypair = create_test_keypair();
        let bob_keypair = create_test_keypair();
        
        let alice_noise_key = [3u8; 32]; // Mock Alice's noise key
        let bob_noise_key = [4u8; 32]; // Mock Bob's noise key
        
        let config = VerificationConfig::default();
        
        // Alice creates verification service
        let mut alice_service = VerificationService::new(
            config.clone(),
            alice_keypair,
            alice_noise_key,
            Some("Alice".to_string()),
        );
        
        // Bob creates verification service  
        let mut bob_service = VerificationService::new(
            config,
            bob_keypair,
            bob_noise_key,
            Some("Bob".to_string()),
        );
        
        // Alice generates QR
        let alice_qr = alice_service.generate_verification_qr().unwrap();
        let alice_uri = alice_qr.to_uri().unwrap();
        
        // Bob scans Alice's QR
        let challenge = bob_service.process_scanned_qr(&alice_uri).unwrap();
        
        // Alice creates response to challenge
        let response = alice_service.create_challenge_response(&challenge).unwrap();
        
        // Bob processes Alice's response
        let result = bob_service.process_verification_response(response).unwrap();
        
        match result {
            VerificationResult::Success { peer_id, nickname, .. } => {
                assert_eq!(peer_id, alice_qr.peer_id());
                assert_eq!(nickname, Some("Alice".to_string()));
            }
            _ => panic!("Expected successful verification"),
        }
    }
    
    #[test]
    fn test_expired_qr_rejection() {
        let keypair = create_test_keypair();
        let noise_pubkey = [5u8; 32]; // Mock noise public key
        let config = VerificationConfig {
            challenge_timeout: Duration::from_millis(1), // Very short timeout
            ..Default::default()
        };
        
        let mut service = VerificationService::new(
            config,
            keypair.clone(),
            noise_pubkey,
            None,
        );
        
        let qr = VerificationQR::new(
            noise_pubkey,
            &keypair,
            None,
            [0u8; 32],
        ).unwrap();
        
        let uri = qr.to_uri().unwrap();
        
        // Wait for expiration
        std::thread::sleep(Duration::from_millis(2));
        
        let result = service.process_scanned_qr(&uri);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }
}