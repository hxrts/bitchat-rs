//! Cryptographic identity with public keys and handshake history

use serde::{Deserialize, Serialize};

use crate::types::{Fingerprint, Timestamp};

/// Cryptographic identity with public keys and handshake history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptographicIdentity {
    /// Fingerprint (SHA-256 of Noise public key)
    pub fingerprint: Fingerprint,
    /// Noise static public key
    pub public_key: [u8; 32],
    /// Optional Ed25519 signing public key
    pub signing_public_key: Option<[u8; 32]>,
    /// First seen timestamp
    pub first_seen: Timestamp,
    /// Last successful handshake timestamp
    pub last_handshake: Timestamp,
    /// Number of successful handshakes
    pub handshake_count: u32,
}

impl CryptographicIdentity {
    /// Create a new cryptographic identity
    pub fn new(public_key: [u8; 32], signing_public_key: Option<[u8; 32]>) -> Self {
        let fingerprint = crate::protocol::generate_fingerprint(public_key);
        let now = Timestamp::now();

        Self {
            fingerprint,
            public_key,
            signing_public_key,
            first_seen: now,
            last_handshake: now,
            handshake_count: 0,
        }
    }

    /// Update handshake time
    pub fn update_handshake_time(&mut self) {
        self.last_handshake = Timestamp::now();
        self.handshake_count = self.handshake_count.saturating_add(1);
    }
}
