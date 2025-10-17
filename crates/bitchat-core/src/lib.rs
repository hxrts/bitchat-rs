//! BitChat Core Protocol Implementation
//!
//! This crate provides the foundational types, cryptographic primitives, and serialization
//! for the BitChat peer-to-peer messaging protocol. It is designed to be `no_std` compatible
//! and work across both native and WebAssembly targets.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// ----------------------------------------------------------------------------
// Module Declarations
// ----------------------------------------------------------------------------

pub mod crypto;
pub mod packet;
pub mod types;

// ----------------------------------------------------------------------------
// Public API
// ----------------------------------------------------------------------------

pub use packet::{BitchatMessage, BitchatPacket, MessageType};
pub use types::{PeerId, Fingerprint, Timestamp};

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Core error types for the BitChat protocol
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

pub type Result<T> = core::result::Result<T, BitchatError>;