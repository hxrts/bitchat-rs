//! Error types for the BitChat protocol
//!
//! This module contains all error types used throughout the BitChat core protocol,
//! including cryptographic errors, transport errors, session errors, and the main
//! BitchatError type that unifies them all.

use alloc::string::String;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "std"))] {
        use alloc::string::ToString;
    }
}

// ----------------------------------------------------------------------------
// Specific Error Types
// ----------------------------------------------------------------------------

cfg_if::cfg_if! {
    // Specific cryptographic error types
    if #[cfg(feature = "std")] {
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
    } else {
        #[derive(Debug)]
        pub enum CryptographicError {
            SignatureVerificationFailed,
            EncryptionFailed,
            DecryptionFailed,
            KeyDerivationFailed,
            InvalidKeyFormat,
            RandomGenerationFailed,
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Specific transport error types
        #[derive(Debug, thiserror::Error)]
        pub enum TransportError {
            #[error("Connection failed to peer {peer_id}: {reason}")]
            ConnectionFailed { peer_id: String, reason: String },
            #[error("Network I/O error: {0}")]
            NetworkIo(#[from] std::io::Error),
            #[error("Transport is not available: {transport_type}")]
            TransportUnavailable { transport_type: String },
            #[error("Send failed: buffer full (capacity: {capacity})")]
            SendBufferFull { capacity: usize },
            #[error("Receive failed: {reason}")]
            ReceiveFailed { reason: String },
            #[error("Protocol mismatch: expected {expected}, got {actual}")]
            ProtocolMismatch { expected: String, actual: String },
            #[error("Transport timeout after {duration_ms}ms")]
            Timeout { duration_ms: u64 },
            #[error("Invalid transport configuration: {reason}")]
            InvalidConfiguration { reason: String },
            #[error("Transport shutdown: {reason}")]
            Shutdown { reason: String },
            #[error("Peer not found: {peer_id}")]
            PeerNotFound { peer_id: String },
            #[error("Authentication failed with peer {peer_id}")]
            AuthenticationFailed { peer_id: String },
        }
    } else {
        /// Specific transport error types (no_std version)
        #[derive(Debug)]
        pub enum TransportError {
            ConnectionFailed { peer_id: String, reason: String },
            TransportUnavailable { transport_type: String },
            SendBufferFull { capacity: usize },
            ReceiveFailed { reason: String },
            ProtocolMismatch { expected: String, actual: String },
            Timeout { duration_ms: u64 },
            InvalidConfiguration { reason: String },
            Shutdown { reason: String },
            PeerNotFound { peer_id: String },
            AuthenticationFailed { peer_id: String },
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Specific session error types
        #[derive(Debug, thiserror::Error)]
        pub enum SessionError {
            #[error("Session not found for peer {peer_id}")]
            SessionNotFound { peer_id: String },
            #[error("Session handshake failed with peer {peer_id}: {reason}")]
            HandshakeFailed { peer_id: String, reason: String },
            #[error("Session timeout for peer {peer_id} after {duration_ms}ms")]
            SessionTimeout { peer_id: String, duration_ms: u64 },
            #[error("Session state invalid for peer {peer_id}: expected {expected}, got {actual}")]
            InvalidState { peer_id: String, expected: String, actual: String },
            #[error("Session encryption failed for peer {peer_id}")]
            EncryptionFailed { peer_id: String },
            #[error("Session decryption failed for peer {peer_id}")]
            DecryptionFailed { peer_id: String },
            #[error("Session key rotation failed for peer {peer_id}: {reason}")]
            KeyRotationFailed { peer_id: String, reason: String },
            #[error("Session already exists for peer {peer_id}")]
            SessionAlreadyExists { peer_id: String },
            #[error("Maximum sessions reached: {current}/{max}")]
            MaxSessionsReached { current: usize, max: usize },
        }
    } else {
        /// Specific session error types (no_std version)
        #[derive(Debug)]
        pub enum SessionError {
            SessionNotFound { peer_id: String },
            HandshakeFailed { peer_id: String, reason: String },
            SessionTimeout { peer_id: String, duration_ms: u64 },
            InvalidState { peer_id: String, expected: String, actual: String },
            EncryptionFailed { peer_id: String },
            DecryptionFailed { peer_id: String },
            KeyRotationFailed { peer_id: String, reason: String },
            SessionAlreadyExists { peer_id: String },
            MaxSessionsReached { current: usize, max: usize },
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Specific fragmentation error types
        #[derive(Debug, thiserror::Error)]
        pub enum FragmentationError {
            #[error("Fragment too large: {size} bytes (max: {max_size})")]
            FragmentTooLarge { size: usize, max_size: usize },
            #[error("Fragment sequence error: expected {expected}, got {actual}")]
            SequenceError { expected: u16, actual: u16 },
            #[error("Duplicate fragment: sequence {sequence} already received")]
            DuplicateFragment { sequence: u16 },
            #[error("Missing fragments: {missing_count} fragments not received")]
            MissingFragments { missing_count: usize },
            #[error("Fragment timeout: incomplete message after {duration_ms}ms")]
            FragmentTimeout { duration_ms: u64 },
            #[error("Fragment buffer overflow: too many incomplete messages ({count})")]
            BufferOverflow { count: usize },
            #[error("Invalid fragment header: {reason}")]
            InvalidHeader { reason: String },
            #[error("Fragmentation not supported for message type {message_type}")]
            NotSupported { message_type: u8 },
        }
    } else {
        /// Specific fragmentation error types (no_std version)
        #[derive(Debug)]
        pub enum FragmentationError {
            FragmentTooLarge { size: usize, max_size: usize },
            SequenceError { expected: u16, actual: u16 },
            DuplicateFragment { sequence: u16 },
            MissingFragments { missing_count: usize },
            FragmentTimeout { duration_ms: u64 },
            BufferOverflow { count: usize },
            InvalidHeader { reason: String },
            NotSupported { message_type: u8 },
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Specific packet validation error types
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

        impl From<String> for PacketError {
            fn from(message: String) -> Self {
                PacketError::Generic { message }
            }
        }

        impl From<&str> for PacketError {
            fn from(message: &str) -> Self {
                PacketError::Generic {
                    message: message.to_string(),
                }
            }
        }
    } else {
        /// Specific packet validation error types (no_std version)
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

        impl From<String> for PacketError {
            fn from(message: String) -> Self {
                PacketError::Generic { message }
            }
        }

        impl From<&str> for PacketError {
            fn from(message: &str) -> Self {
                PacketError::Generic {
                    message: message.to_string(),
                }
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Core error types for the BitChat protocol
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

            #[error("Transport error: {0}")]
            Transport(#[from] TransportError),

            #[error("Session error: {0}")]
            Session(#[from] SessionError),

            #[error("Fragmentation error: {0}")]
            Fragmentation(#[from] FragmentationError),

            #[error("State transition error: {0}")]
            StateTransition(#[from] crate::protocol::StateTransitionError),

            #[error("Signature verification failed")]
            Signature,

            /// Channel communication error (internal to CSP architecture)
            #[error("Channel error: {message}")]
            Channel { message: String },

            /// Configuration error
            #[error("Configuration error: {reason}")]
            Configuration { reason: String },

            /// Rate limiting error
            #[error("Rate limited: {reason}")]
            RateLimited { reason: String },
        }
    } else {
        /// Core error types for the BitChat protocol (no_std version)
        #[derive(Debug)]
        pub enum BitchatError {
            Serialization(bincode::Error),
            Crypto(CryptographicError),
            InvalidPacket(PacketError),
            Noise(snow::Error),
            Transport(TransportError),
            Session(SessionError),
            Fragmentation(FragmentationError),
            StateTransition(crate::protocol::StateTransitionError),
            Signature,
            Channel { message: String },
            Configuration { reason: String },
            RateLimited { reason: String },
        }

        impl From<bincode::Error> for BitchatError {
            fn from(err: bincode::Error) -> Self {
                BitchatError::Serialization(err)
            }
        }

        impl From<snow::Error> for BitchatError {
            fn from(err: snow::Error) -> Self {
                BitchatError::Noise(err)
            }
        }

        impl From<CryptographicError> for BitchatError {
            fn from(err: CryptographicError) -> Self {
                BitchatError::Crypto(err)
            }
        }

        impl From<PacketError> for BitchatError {
            fn from(err: PacketError) -> Self {
                BitchatError::InvalidPacket(err)
            }
        }

        impl From<TransportError> for BitchatError {
            fn from(err: TransportError) -> Self {
                BitchatError::Transport(err)
            }
        }

        impl From<SessionError> for BitchatError {
            fn from(err: SessionError) -> Self {
                BitchatError::Session(err)
            }
        }

        impl From<FragmentationError> for BitchatError {
            fn from(err: FragmentationError) -> Self {
                BitchatError::Fragmentation(err)
            }
        }

        impl From<crate::protocol::StateTransitionError> for BitchatError {
            fn from(err: crate::protocol::StateTransitionError) -> Self {
                BitchatError::StateTransition(err)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Convenience Error Constructors
// ----------------------------------------------------------------------------

impl BitchatError {
    /// Create an invalid packet error with a message
    pub fn invalid_packet<T: Into<String>>(message: T) -> Self {
        BitchatError::InvalidPacket(PacketError::Generic {
            message: message.into(),
        })
    }

    /// Create a channel error with a message
    pub fn channel_error<T: Into<String>>(message: T) -> Self {
        BitchatError::Channel {
            message: message.into(),
        }
    }

    /// Create a configuration error with a reason
    pub fn config_error<T: Into<String>>(reason: T) -> Self {
        BitchatError::Configuration {
            reason: reason.into(),
        }
    }

    /// Create a rate limiting error with a reason
    pub fn rate_limited<T: Into<String>>(reason: T) -> Self {
        BitchatError::RateLimited {
            reason: reason.into(),
        }
    }

    /// Create a signature verification error
    pub fn signature_error() -> Self {
        BitchatError::Signature
    }

    /// Create a transport connection failed error
    pub fn connection_failed<P: Into<String>, R: Into<String>>(peer_id: P, reason: R) -> Self {
        BitchatError::Transport(TransportError::ConnectionFailed {
            peer_id: peer_id.into(),
            reason: reason.into(),
        })
    }

    /// Create a session not found error
    pub fn session_not_found<P: Into<String>>(peer_id: P) -> Self {
        BitchatError::Session(SessionError::SessionNotFound {
            peer_id: peer_id.into(),
        })
    }

    /// Create a handshake failed error
    pub fn handshake_failed<P: Into<String>, R: Into<String>>(peer_id: P, reason: R) -> Self {
        BitchatError::Session(SessionError::HandshakeFailed {
            peer_id: peer_id.into(),
            reason: reason.into(),
        })
    }

    /// Create a serialization error from any serialization failure
    pub fn serialization_error() -> Self {
        BitchatError::InvalidPacket(PacketError::Generic {
            message: "Serialization failed".into(),
        })
    }

    /// Create a deserialization error from any deserialization failure
    pub fn deserialization_error() -> Self {
        BitchatError::InvalidPacket(PacketError::Generic {
            message: "Deserialization failed".into(),
        })
    }
}

// ----------------------------------------------------------------------------
// Type Aliases
// ----------------------------------------------------------------------------

pub type Result<T> = core::result::Result<T, BitchatError>;
pub type BitchatResult<T> = Result<T>;
