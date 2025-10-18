//! Nostr transport implementation for BitChat
//!
//! This crate provides a Nostr transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Nostr relays.
//!
//! ## Architecture
//!
//! The Nostr transport is organized into several modules:
//!
//! - [`config`] - Transport configuration and settings
//! - [`error`] - Error types specific to Nostr transport
//! - [`message`] - BitChat message format for Nostr events
//! - [`transport`] - Main transport implementation
//!
//! ## Usage
//!
//! ```rust,no_run
//! use bitchat_nostr::{NostrTransport, NostrTransportConfig};
//! use bitchat_core::{PeerId, transport::Transport};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//! let mut transport = NostrTransport::new(peer_id)?;
//! transport.start().await?;
//!
//! // The transport will:
//! // - Connect to configured Nostr relays
//! // - Subscribe to BitChat events
//! // - Enable sending/receiving messages over Nostr
//! # Ok(())
//! # }
//! ```
//!
//! ## Nostr Integration
//!
//! BitChat messages are encoded as custom Nostr events (kind 30420) and transmitted
//! over standard Nostr relays. This provides:
//!
//! - Global connectivity when BLE is unavailable
//! - Fallback communication path for distant peers
//! - Integration with existing Nostr infrastructure

pub mod config;
pub mod error;
pub mod message;
pub mod transport;

// Re-export public API
pub use config::{create_local_relay_config, NostrTransportConfig};
pub use error::NostrTransportError;
pub use message::{BitchatNostrMessage, BITCHAT_KIND};
pub use transport::NostrTransport;

// Re-export Transport trait for convenience
pub use bitchat_core::transport::Transport;
