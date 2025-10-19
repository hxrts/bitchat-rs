//! Core types for the BitChat protocol
//!
//! This module defines the fundamental types used throughout the protocol,
//! using newtype patterns for semantic validation and type safety.

use core::fmt;
use core::ops::Deref;
use core::str::FromStr;
use serde::{Deserialize, Serialize};

cfg_if::cfg_if! {
    if #[cfg(not(feature = "std"))] {
        use alloc::string::{String, ToString};
    }
}

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
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for PeerId {
    type Err = crate::BitchatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Remove common prefixes that might be present
        let clean_str = s.strip_prefix("0x").unwrap_or(s);

        // Decode hex string
        let bytes = hex::decode(clean_str)
            .map_err(|_| crate::BitchatError::invalid_packet("Invalid hex in PeerId"))?;

        // Must be exactly 8 bytes or we truncate/pad
        if bytes.len() != 8 {
            if bytes.len() > 8 {
                // Truncate to first 8 bytes
                let mut id = [0u8; 8];
                id.copy_from_slice(&bytes[..8]);
                Ok(Self(id))
            } else {
                // Pad with zeros
                let mut id = [0u8; 8];
                id[..bytes.len()].copy_from_slice(&bytes);
                Ok(Self(id))
            }
        } else {
            let mut id = [0u8; 8];
            id.copy_from_slice(&bytes);
            Ok(Self(id))
        }
    }
}

impl Deref for PeerId {
    type Target = [u8; 8];

    fn deref(&self) -> &Self::Target {
        &self.0
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
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for Fingerprint {
    type Err = crate::BitchatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Remove common prefixes that might be present
        let clean_str = s.strip_prefix("0x").unwrap_or(s);

        // Decode hex string
        let bytes = hex::decode(clean_str)
            .map_err(|_| crate::BitchatError::invalid_packet("Invalid hex in Fingerprint"))?;

        // Must be exactly 32 bytes
        if bytes.len() != 32 {
            return Err(crate::BitchatError::invalid_packet(
                "Fingerprint must be exactly 32 bytes",
            ));
        }

        let mut fingerprint = [0u8; 32];
        fingerprint.copy_from_slice(&bytes);
        Ok(Self(fingerprint))
    }
}

impl Deref for Fingerprint {
    type Target = [u8; 32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ----------------------------------------------------------------------------
// Timestamp
// ----------------------------------------------------------------------------

/// Millisecond timestamp since Unix epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(u64);

use core::ops::{Add, Sub};

impl Add<u64> for Timestamp {
    type Output = Timestamp;

    fn add(self, other: u64) -> Timestamp {
        Timestamp(self.0 + other)
    }
}

impl Sub for Timestamp {
    type Output = u64;

    fn sub(self, other: Timestamp) -> u64 {
        self.0.saturating_sub(other.0)
    }
}

impl Timestamp {
    /// Create a new timestamp
    pub fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// Get current timestamp (context-aware based on available features)
    pub fn now() -> Self {
        cfg_if::cfg_if! {
            if #[cfg(feature = "std")] {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                Self(duration.as_millis() as u64)
            } else if #[cfg(all(feature = "wasm", target_arch = "wasm32"))] {
                // Use js-sys::Date::now() to get proper Unix timestamp in WASM
                use js_sys::Date;
                Self(Date::now() as u64)
            } else if #[cfg(feature = "wasm")] {
                // Fallback to instant crate if wasm feature but not WASM target
                use instant::Instant;

                // Get time since page load/module instantiation
                let millis = Instant::now().elapsed().as_millis() as u64;

                // This is a best-effort approximation - real applications should use std feature
                // which provides proper Date.now() access
                const WASM_FALLBACK_BASE: u64 = 1_700_000_000_000; // ~Nov 2023 as fallback base
                Self(WASM_FALLBACK_BASE + millis)
            } else {
                // Fallback for alloc-only builds
                Self(0)
            }
        }
    }

    /// Get the raw milliseconds
    pub fn as_millis(&self) -> u64 {
        self.0
    }

    /// Add seconds to this timestamp
    pub fn add_seconds(&self, seconds: u64) -> Self {
        Self(self.0 + (seconds * 1000))
    }

    /// Get duration since another timestamp
    pub fn duration_since(&self, other: Self) -> core::time::Duration {
        let millis_diff = self.0.saturating_sub(other.0);
        core::time::Duration::from_millis(millis_diff)
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

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Standard library implementation of TimeSource
        #[derive(Debug, Clone, Copy, Default)]
        pub struct SystemTimeSource;

        impl SystemTimeSource {
            pub fn new() -> Self {
                Self
            }
        }

        impl TimeSource for SystemTimeSource {
            fn now(&self) -> Timestamp {
                Timestamp::now()
            }
        }
    } else if #[cfg(feature = "wasm")] {
        /// WASM implementation of TimeSource
        #[derive(Debug, Clone, Copy, Default)]
        pub struct WasmTimeSource;

        impl TimeSource for WasmTimeSource {
            fn now(&self) -> Timestamp {
                Timestamp::now()
            }
        }
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
