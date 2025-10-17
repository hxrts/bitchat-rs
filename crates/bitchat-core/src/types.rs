//! Core types for the BitChat protocol
//!
//! This module defines the fundamental types used throughout the protocol,
//! using newtype patterns for semantic validation and type safety.

use core::fmt;
use serde::{Deserialize, Serialize};

// ----------------------------------------------------------------------------
// Peer Identifier
// ----------------------------------------------------------------------------

/// Unique identifier for a peer (8-byte truncated from full public key)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PeerId([u8; 8]);

impl PeerId {
    /// Create a new PeerId from 8 bytes
    pub fn new(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Create PeerId from the first 8 bytes of a longer identifier
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut id = [0u8; 8];
        let len = core::cmp::min(bytes.len(), 8);
        id[..len].copy_from_slice(&bytes[..len]);
        Self(id)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    /// Special broadcast peer ID (all 0xFF)
    pub const BROADCAST: Self = Self([0xFF; 8]);
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

// ----------------------------------------------------------------------------
// Fingerprint
// ----------------------------------------------------------------------------

/// SHA-256 fingerprint of a peer's static public key
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Create a new fingerprint from 32 bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create a PeerId from this fingerprint (first 8 bytes)
    pub fn to_peer_id(&self) -> PeerId {
        PeerId::from_bytes(&self.0)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

// ----------------------------------------------------------------------------
// Timestamp
// ----------------------------------------------------------------------------

/// Millisecond timestamp since Unix epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Create a new timestamp
    pub fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// Get current timestamp (requires std feature or WASM target)
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(duration.as_millis() as u64)
    }

    /// Get current timestamp for WASM targets
    #[cfg(target_arch = "wasm32")]
    pub fn now() -> Self {
        use instant::Instant;
        
        // For WASM, we use instant::Instant which is epoch-based
        // The instant crate provides WASM-compatible timing
        let now = Instant::now();
        let millis = now.elapsed().as_millis() as u64;
        
        // Add a base timestamp to simulate Unix epoch (approximation for WASM)
        // This is a compromise since WASM doesn't have direct access to system time
        const WASM_BASE_TIMESTAMP: u64 = 1_640_995_200_000; // Jan 1, 2022 as base
        Self(WASM_BASE_TIMESTAMP + millis)
    }

    /// Get the raw milliseconds
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}

// ----------------------------------------------------------------------------
// Time-to-Live (TTL)
// ----------------------------------------------------------------------------

/// Time-to-live for packet routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ttl(u8);

impl Ttl {
    /// Create a new TTL
    pub fn new(value: u8) -> Self {
        Self(value)
    }

    /// Default TTL for new packets
    pub const DEFAULT: Self = Self(7);

    /// Maximum TTL value
    pub const MAX: Self = Self(7);

    /// Get the raw value
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Decrement TTL, returning None if it reaches 0
    pub fn decrement(self) -> Option<Self> {
        if self.0 > 0 {
            Some(Self(self.0 - 1))
        } else {
            None
        }
    }
}

impl Default for Ttl {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// ----------------------------------------------------------------------------
// Time Source Trait
// ----------------------------------------------------------------------------

/// Trait for providing timestamps in a no_std compatible way
///
/// This trait allows the library to obtain current timestamps without
/// depending on std. Implementations should provide monotonic timestamps
/// when possible.
pub trait TimeSource {
    /// Get the current timestamp
    fn now(&self) -> Timestamp;
}

/// Standard library implementation of TimeSource
#[cfg(any(feature = "std", target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct StdTimeSource;

#[cfg(any(feature = "std", target_arch = "wasm32"))]
impl TimeSource for StdTimeSource {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

// ----------------------------------------------------------------------------
// Hex Encoding Helper
// ----------------------------------------------------------------------------

// Use the optimized hex crate instead of custom implementation
use hex;

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        let peer_id = PeerId::new(bytes);
        assert_eq!(peer_id.as_bytes(), &bytes);

        let from_long = PeerId::from_bytes(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(from_long.as_bytes(), &bytes);
    }

    #[test]
    fn test_ttl() {
        let mut ttl = Ttl::new(3);
        assert_eq!(ttl.value(), 3);

        ttl = ttl.decrement().unwrap();
        assert_eq!(ttl.value(), 2);

        ttl = ttl.decrement().unwrap();
        assert_eq!(ttl.value(), 1);

        ttl = ttl.decrement().unwrap();
        assert_eq!(ttl.value(), 0);

        assert!(ttl.decrement().is_none());
    }

    #[test]
    fn test_fingerprint_to_peer_id() {
        let fingerprint_bytes = [1u8; 32];
        let fingerprint = Fingerprint::new(fingerprint_bytes);
        let peer_id = fingerprint.to_peer_id();
        assert_eq!(peer_id.as_bytes(), &[1u8; 8]);
    }
}
