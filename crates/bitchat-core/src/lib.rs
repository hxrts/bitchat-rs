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
pub mod session;
pub mod transport;
pub mod types;

// ----------------------------------------------------------------------------
// Public API
// ----------------------------------------------------------------------------

pub use delivery::{DeliveryTracker, DeliveryStatus, DeliveryConfig};
pub use fragmentation::{MessageFragmenter, MessageReassembler, Fragment};
pub use handlers::{MessageHandler, MessageDispatcher, MessageBuilder, BitchatEvent, EventHandler};
pub use packet::{BitchatMessage, BitchatPacket, MessageType};
pub use session::{NoiseSession, NoiseSessionManager, SessionState};
pub use transport::{Transport, TransportManager, TransportCapabilities, TransportType};
pub use types::{PeerId, Fingerprint, Timestamp, TimeSource};

// Convenience type aliases for std feature
#[cfg(feature = "std")]
pub use types::StdTimeSource;

#[cfg(feature = "std")]
pub type StdDeliveryTracker = DeliveryTracker<StdTimeSource>;

#[cfg(feature = "std")]
pub type StdNoiseSessionManager = NoiseSessionManager<StdTimeSource>;

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Core error types for the BitChat protocol
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum BitchatError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    #[error("Invalid packet format: {0}")]
    InvalidPacket(String),
    
    #[error("Noise protocol error: {0}")]
    Noise(#[from] snow::Error),
    
    #[error("Ed25519 signature error")]
    Signature,
}

/// Core error types for the BitChat protocol (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum BitchatError {
    Serialization(bincode::Error),
    Crypto(String),
    InvalidPacket(String),
    Noise(snow::Error),
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

pub type Result<T> = core::result::Result<T, BitchatError>;