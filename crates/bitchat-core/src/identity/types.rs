//! Core identity types and enums

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::types::Fingerprint;

// ----------------------------------------------------------------------------
// Handshake State
// ----------------------------------------------------------------------------

/// Handshake state for a peer connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakeState {
    /// No handshake initiated
    None,
    /// Handshake in progress
    InProgress,
    /// Handshake completed successfully
    Completed { fingerprint: Fingerprint },
    /// Handshake failed
    Failed { reason: String },
}

impl HandshakeState {
    /// Check if handshake is complete
    pub fn is_complete(&self) -> bool {
        matches!(self, HandshakeState::Completed { .. })
    }

    /// Get the fingerprint if handshake is complete
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        match self {
            HandshakeState::Completed { fingerprint } => Some(fingerprint),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// Trust Level
// ----------------------------------------------------------------------------

/// Trust level for a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Unknown peer (default)
    Unknown = 0,
    /// Known but not trusted
    Known = 1,
    /// Trusted peer
    Trusted = 2,
    /// Verified peer (fingerprint confirmed out-of-band)
    Verified = 3,
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::Unknown
    }
}
