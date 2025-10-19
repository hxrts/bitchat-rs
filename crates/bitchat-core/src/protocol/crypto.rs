//! Cryptographic primitives for BitChat
//!
//! This module provides clean, safe wrappers around the core cryptographic operations
//! required by the BitChat protocol, including Noise Protocol, Ed25519 signatures,
//! and fingerprint generation.

use alloc::{vec, vec::Vec};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use snow::{Builder, HandshakeState, TransportState};

use crate::types::Fingerprint;
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Noise Protocol configuration for BitChat
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

// ----------------------------------------------------------------------------
// Identity Key Pair (Ed25519)
// ----------------------------------------------------------------------------

/// Ed25519 signing key pair for identity
#[derive(Debug, Clone)]
pub struct IdentityKeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl IdentityKeyPair {
    /// Generate a new random identity key pair
    pub fn generate() -> Result<Self> {
        let mut rng = rand_core::OsRng;
        Self::generate_with_rng(&mut rng)
    }

    /// Generate a new identity key pair with custom RNG
    pub fn generate_with_rng<R: RngCore + CryptoRng>(rng: &mut R) -> Result<Self> {
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Create from raw private key bytes
    pub fn from_bytes(private_key: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(private_key);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Get the private key bytes
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Sign data with flexible input types
    pub fn sign<T: AsRef<[u8]>>(&self, data: T) -> [u8; 64] {
        self.signing_key.sign(data.as_ref()).to_bytes()
    }

    /// Verify a signature from another key with flexible input types
    pub fn verify<D: AsRef<[u8]>>(
        public_key: &[u8; 32],
        data: D,
        signature: &[u8; 64],
    ) -> Result<()> {
        let verifying_key =
            VerifyingKey::from_bytes(public_key).map_err(|_| BitchatError::signature_error())?;
        let signature = Signature::from_bytes(signature);

        verifying_key
            .verify(data.as_ref(), &signature)
            .map_err(|_| BitchatError::signature_error())
    }
}

// ----------------------------------------------------------------------------
// Noise Key Pair (X25519)
// ----------------------------------------------------------------------------

/// X25519 key pair for Noise Protocol
#[derive(Debug)]
pub struct NoiseKeyPair {
    private_key: [u8; 32],
    public_key: [u8; 32],
}

impl NoiseKeyPair {
    /// Generate a new random Noise key pair
    pub fn generate() -> Self {
        let mut rng = rand_core::OsRng;
        Self::generate_with_rng(&mut rng)
    }

    /// Generate a new Noise key pair with custom RNG
    pub fn generate_with_rng<R: RngCore + CryptoRng>(rng: &mut R) -> Self {
        let mut private_key = [0u8; 32];
        rng.fill_bytes(&mut private_key);

        // Use curve25519-dalek for key derivation
        use curve25519_dalek::constants::X25519_BASEPOINT;
        use curve25519_dalek::scalar::Scalar;

        let scalar = Scalar::from_bytes_mod_order(private_key);
        let point = scalar * X25519_BASEPOINT;
        let public_key = point.to_bytes();

        Self {
            private_key,
            public_key,
        }
    }

    /// Create from raw private key bytes
    pub fn from_bytes(private_key: &[u8; 32]) -> Self {
        use curve25519_dalek::constants::X25519_BASEPOINT;
        use curve25519_dalek::scalar::Scalar;

        let scalar = Scalar::from_bytes_mod_order(*private_key);
        let point = scalar * X25519_BASEPOINT;
        let public_key = point.to_bytes();

        Self {
            private_key: *private_key,
            public_key,
        }
    }

    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key
    }

    /// Get the private key bytes
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.private_key
    }

    /// Generate fingerprint from public key
    pub fn fingerprint(&self) -> Fingerprint {
        generate_fingerprint(self.public_key_bytes())
    }
}

// ----------------------------------------------------------------------------
// Fingerprint Generation
// ----------------------------------------------------------------------------

/// Generate SHA-256 fingerprint from a public key with flexible input types
pub fn generate_fingerprint<T: AsRef<[u8]>>(public_key: T) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(public_key.as_ref());
    let hash = hasher.finalize();

    let mut fingerprint = [0u8; 32];
    fingerprint.copy_from_slice(&hash);
    Fingerprint::new(fingerprint)
}

// ----------------------------------------------------------------------------
// Noise Protocol Handshake
// ----------------------------------------------------------------------------

/// Noise Protocol handshake state
pub struct NoiseHandshake {
    state: HandshakeState,
}

impl core::fmt::Debug for NoiseHandshake {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NoiseHandshake")
            .field("state", &"<HandshakeState>")
            .finish()
    }
}

impl NoiseHandshake {
    /// Create initiator handshake
    pub fn initiator(local_key: &NoiseKeyPair) -> Result<Self> {
        let builder = Builder::new(NOISE_PATTERN.parse()?);
        let state = builder
            .local_private_key(&local_key.private_key_bytes())
            .build_initiator()?;

        Ok(Self { state })
    }

    /// Create responder handshake
    pub fn responder(local_key: &NoiseKeyPair) -> Result<Self> {
        let builder = Builder::new(NOISE_PATTERN.parse()?);
        let state = builder
            .local_private_key(&local_key.private_key_bytes())
            .build_responder()?;

        Ok(Self { state })
    }

    /// Write handshake message
    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let mut output = vec![0u8; 65536]; // Larger buffer for handshake messages
        let len = self.state.write_message(payload, &mut output)?;
        output.truncate(len);
        Ok(output)
    }

    /// Read handshake message
    pub fn read_message(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        let mut output = vec![0u8; 65536]; // Larger buffer for handshake messages
        let len = self
            .state
            .read_message(input, &mut output)
            .map_err(BitchatError::Noise)?;
        output.truncate(len);
        Ok(output)
    }

    /// Check if handshake is complete
    pub fn is_handshake_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Convert to transport mode
    pub fn into_transport_mode(self) -> Result<NoiseTransport> {
        let transport = self
            .state
            .into_transport_mode()
            .map_err(BitchatError::Noise)?;

        Ok(NoiseTransport { state: transport })
    }

    /// Get remote static key (available after handshake)
    pub fn get_remote_static(&self) -> Option<[u8; 32]> {
        self.state.get_remote_static().map(|key| {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(key);
            bytes
        })
    }
}

// ----------------------------------------------------------------------------
// Noise Protocol Transport
// ----------------------------------------------------------------------------

/// Noise Protocol transport state for encrypted communication
pub struct NoiseTransport {
    state: TransportState,
}

impl core::fmt::Debug for NoiseTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NoiseTransport")
            .field("state", &"<TransportState>")
            .finish()
    }
}

impl NoiseTransport {
    /// Encrypt a message
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut ciphertext = vec![0u8; plaintext.len() + 16]; // +16 for tag
        let len = self
            .state
            .write_message(plaintext, &mut ciphertext)
            .map_err(BitchatError::Noise)?;
        ciphertext.truncate(len);
        Ok(ciphertext)
    }

    /// Decrypt a message
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut plaintext = vec![0u8; ciphertext.len()];
        let len = self
            .state
            .read_message(ciphertext, &mut plaintext)
            .map_err(BitchatError::Noise)?;
        plaintext.truncate(len);
        Ok(plaintext)
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_keypair() {
        let keypair = IdentityKeyPair::generate().unwrap();
        let public_key = keypair.public_key_bytes();
        let _private_key = keypair.private_key_bytes();

        // Test signing and verification
        let data = b"test message";
        let signature = keypair.sign(data);

        IdentityKeyPair::verify(&public_key, data, &signature).unwrap();

        // Test that wrong signature fails
        let wrong_signature = [0u8; 64];
        assert!(IdentityKeyPair::verify(&public_key, data, &wrong_signature).is_err());
    }

    #[test]
    fn test_noise_keypair() {
        let keypair = NoiseKeyPair::generate();
        let public_key = keypair.public_key_bytes();
        let fingerprint = keypair.fingerprint();

        // Test fingerprint generation
        let expected_fingerprint = generate_fingerprint(public_key);
        assert_eq!(fingerprint.as_bytes(), expected_fingerprint.as_bytes());
    }

    #[test]
    fn test_noise_handshake() {
        let alice_key = NoiseKeyPair::generate();
        let bob_key = NoiseKeyPair::generate();

        let mut alice = NoiseHandshake::initiator(&alice_key).unwrap();
        let mut bob = NoiseHandshake::responder(&bob_key).unwrap();

        // Step 1: Alice -> Bob
        let message1 = alice.write_message(b"").unwrap();
        let _response1 = bob.read_message(&message1).unwrap();

        // Step 2: Bob -> Alice
        let message2 = bob.write_message(b"").unwrap();
        let _response2 = alice.read_message(&message2).unwrap();

        // Step 3: Alice -> Bob
        let message3 = alice.write_message(b"").unwrap();
        let _response3 = bob.read_message(&message3).unwrap();

        assert!(alice.is_handshake_finished());
        assert!(bob.is_handshake_finished());

        // Test transport mode
        let mut alice_transport = alice.into_transport_mode().unwrap();
        let mut bob_transport = bob.into_transport_mode().unwrap();

        let plaintext = b"Hello, Bob!";
        let ciphertext = alice_transport.encrypt(plaintext).unwrap();
        let decrypted = bob_transport.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }
}
