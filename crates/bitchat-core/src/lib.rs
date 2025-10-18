//! BitChat Core Protocol Implementation
//!
//! This crate provides the foundational types, cryptographic primitives, and serialization
//! for the BitChat peer-to-peer messaging protocol. It is designed to be `no_std` compatible
//! and work across both native and WebAssembly targets.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;

// ----------------------------------------------------------------------------
// Module Declarations
// ----------------------------------------------------------------------------

pub mod crypto;
pub mod delivery;
pub mod fragmentation;
pub mod handlers;
pub mod packet;
pub mod rate_limiter;
pub mod session;
pub mod transport;
pub mod types;

// ----------------------------------------------------------------------------
// Public API
// ----------------------------------------------------------------------------

pub use delivery::{DeliveryConfig, DeliveryStatus, DeliveryTracker};
pub use fragmentation::{Fragment, MessageFragmenter, MessageReassembler};
pub use handlers::{BitchatEvent, EventHandler, MessageBuilder, MessageDispatcher, MessageHandler};
pub use packet::{BitchatMessage, BitchatPacket, MessageType};
pub use rate_limiter::{RateLimitConfig, RateLimitStats, RateLimiter};
pub use session::{NoiseSession, NoiseSessionManager, SessionState};
pub use transport::{Transport, TransportCapabilities, TransportManager, TransportType};
pub use types::{Fingerprint, PeerId, TimeSource, Timestamp};

// Error types - defined below in this module

// Convenience type aliases for std feature and WASM targets
#[cfg(any(feature = "std", target_arch = "wasm32"))]
pub use types::StdTimeSource;

#[cfg(any(feature = "std", target_arch = "wasm32"))]
pub type StdDeliveryTracker = DeliveryTracker<StdTimeSource>;

#[cfg(any(feature = "std", target_arch = "wasm32"))]
pub type StdNoiseSessionManager = NoiseSessionManager<StdTimeSource>;

#[cfg(any(feature = "std", target_arch = "wasm32"))]
pub type StdRateLimiter = RateLimiter<StdTimeSource>;

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Specific cryptographic error types
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum CryptographicError {
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Key derivation failed")]
    KeyDerivationFailed,
    #[error("Invalid key format")]
    InvalidKeyFormat,
    #[error("Random number generation failed")]
    RandomGenerationFailed,
}

/// Specific packet validation error types
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error("Packet payload too small (expected at least {expected}, got {actual})")]
    PayloadTooSmall { expected: usize, actual: usize },
    #[error("Packet payload too large (max {max}, got {actual})")]
    PayloadTooLarge { max: usize, actual: usize },
    #[error("Unknown message type: {message_type}")]
    UnknownMessageType { message_type: u8 },
    #[error("Invalid recipient ID")]
    InvalidRecipientId,
    #[error("Invalid sender ID")]
    InvalidSenderId,
    #[error("Malformed packet header")]
    MalformedHeader,
    #[error("Checksum verification failed")]
    ChecksumFailed,
    #[error("Fragment sequence error")]
    FragmentSequenceError,
    #[error("Duplicate fragment")]
    DuplicateFragment,
    #[error("{message}")]
    Generic { message: String },
}

#[cfg(feature = "std")]
impl From<String> for PacketError {
    fn from(message: String) -> Self {
        PacketError::Generic { message }
    }
}

#[cfg(feature = "std")]
impl From<&str> for PacketError {
    fn from(message: &str) -> Self {
        PacketError::Generic {
            message: message.to_string(),
        }
    }
}

/// Core error types for the BitChat protocol
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum BitchatError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Cryptographic error: {0}")]
    Crypto(#[from] CryptographicError),

    #[error("Invalid packet: {0}")]
    InvalidPacket(#[from] PacketError),

    #[error("Noise protocol error: {0}")]
    Noise(#[from] snow::Error),

    #[error("Transport error: {message}")]
    Transport { message: String },

    #[error("Session error: {message}")]
    Session { message: String },

    #[error("Fragmentation error: {message}")]
    Fragmentation { message: String },

    #[error("Signature verification failed")]
    Signature,
}

/// Specific cryptographic error types (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum CryptographicError {
    SignatureVerificationFailed,
    EncryptionFailed,
    DecryptionFailed,
    KeyDerivationFailed,
    InvalidKeyFormat,
    RandomGenerationFailed,
}

/// Specific packet validation error types (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum PacketError {
    PayloadTooSmall { expected: usize, actual: usize },
    PayloadTooLarge { max: usize, actual: usize },
    UnknownMessageType { message_type: u8 },
    InvalidRecipientId,
    InvalidSenderId,
    MalformedHeader,
    ChecksumFailed,
    FragmentSequenceError,
    DuplicateFragment,
    Generic { message: String },
}

#[cfg(not(feature = "std"))]
impl From<String> for PacketError {
    fn from(message: String) -> Self {
        PacketError::Generic { message }
    }
}

#[cfg(not(feature = "std"))]
impl From<&str> for PacketError {
    fn from(message: &str) -> Self {
        PacketError::Generic {
            message: message.to_string(),
        }
    }
}

/// Core error types for the BitChat protocol (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum BitchatError {
    Serialization(bincode::Error),
    Crypto(CryptographicError),
    InvalidPacket(PacketError),
    Noise(snow::Error),
    Transport { message: String },
    Session { message: String },
    Fragmentation { message: String },
    Signature,
}

#[cfg(not(feature = "std"))]
impl From<bincode::Error> for BitchatError {
    fn from(err: bincode::Error) -> Self {
        BitchatError::Serialization(err)
    }
}

#[cfg(not(feature = "std"))]
impl From<snow::Error> for BitchatError {
    fn from(err: snow::Error) -> Self {
        BitchatError::Noise(err)
    }
}

#[cfg(not(feature = "std"))]
impl From<CryptographicError> for BitchatError {
    fn from(err: CryptographicError) -> Self {
        BitchatError::Crypto(err)
    }
}

#[cfg(not(feature = "std"))]
impl From<PacketError> for BitchatError {
    fn from(err: PacketError) -> Self {
        BitchatError::InvalidPacket(err)
    }
}

pub type Result<T> = core::result::Result<T, BitchatError>;
