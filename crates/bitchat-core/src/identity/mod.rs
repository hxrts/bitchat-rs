//! Identity management system for BitChat
//!
//! Implements a three-layer identity model:
//! 1. Ephemeral Identity - Temporary per-session state
//! 2. Cryptographic Identity - Long-term public key and handshake state
//! 3. Social Identity - User-assigned names, trust levels, and social metadata

// Module declarations
pub mod cache;
pub mod crypto;
pub mod ephemeral;
pub mod manager;
pub mod social;
pub mod storage;
pub mod types;

// Re-export commonly used types
pub use cache::{IdentityCache, IdentityCacheStats};
pub use crypto::CryptographicIdentity;
pub use ephemeral::EphemeralIdentity;
pub use manager::SecureIdentityStateManager;
pub use social::SocialIdentity;
pub use storage::{create_default_storage, create_test_storage, SecureStorage, StorageConfig};
pub use types::{HandshakeState, TrustLevel};
