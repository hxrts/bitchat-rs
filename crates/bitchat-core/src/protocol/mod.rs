//! BitChat Protocol Module
//!
//! This module contains the core BitChat messaging protocol implementation:
//! - `crypto`: Cryptographic primitives and key management
//! - `session`: Noise session management and handshakes
//! - `delivery`: Message delivery tracking and reliability
//! - `message_store`: Content-addressed message storage
//! - `connection_state`: Connection state machine management

pub mod crypto;
pub mod session;
pub mod delivery;
pub mod message_store;
pub mod connection_state;

// Re-export crypto types
pub use crypto::{NoiseKeyPair, IdentityKeyPair, NoiseHandshake, NoiseTransport, generate_fingerprint};

// Re-export session types
pub use session::{NoiseSession, SessionState};

// Re-export delivery types
pub use delivery::{DeliveryConfig, DeliveryStatus, TrackedMessage, DeliveryAttempt};

// Re-export message store types
pub use message_store::{MessageId, ContentAddressedMessage, ConversationId, MessageStore, MessageStoreStats};

// Re-export connection state types
pub use connection_state::{ConnectionState, ConnectionEvent, StateTransition, StateTransitionError, AuditEntry, SessionParams};