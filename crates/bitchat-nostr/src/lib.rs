//! Nostr transport implementation for BitChat Hybrid Architecture
//!
//! This crate provides a Nostr transport task that integrates with the BitChat
//! hybrid CSP architecture through channel-based communication.
//!
//! ## Architecture
//!
//! The Nostr transport is organized into several modules:
//!
//! - [`config`] - Transport configuration and settings
//! - [`error`] - Error types specific to Nostr transport
//! - [`message`] - BitChat message format for Nostr events
//! - [`nip17`] - NIP-17 gift-wrapping for encrypted direct messages
//! - [`transport`] - Transport task implementation using CSP channels
//!
//! ## Usage
//!
//! ```rust,no_run
//! use bitchat_nostr::{NostrTransportTask, NostrConfig};
//! use bitchat_core::{
//!     TransportTask,
//!     internal::{
//!         create_event_channel, create_effect_channel,
//!         ChannelConfig,
//!     },
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ChannelConfig::default();
//! let (event_sender, _event_receiver) = create_event_channel(&config);
//! let (_effect_sender, effect_receiver) = create_effect_channel(&config);
//!
//! let mut nostr_task = NostrTransportTask::new(NostrConfig::default())?;
//! nostr_task.attach_channels(event_sender, effect_receiver)?;
//!
//! // The transport task is ready to run via the TransportTask trait
//! // In a real application, the BitchatRuntime would spawn:
//! // tokio::spawn(async move { nostr_task.run().await });
//!
//! // The transport task will:
//! // - Connect to configured Nostr relays
//! // - Process effects from Core Logic via channels
//! // - Send events to Core Logic via channels
//! // - Handle discovery, messaging, and relay management
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
//! - Channel-based coordination with Core Logic task

pub mod config;
pub mod embedding;
pub mod error;
pub mod message;
pub mod nip17;
pub mod relay_manager;
pub mod transport;

#[cfg(test)]
mod integration_tests;

// Re-export public API
pub use config::{NostrConfig, NostrRelayConfig};
pub use embedding::{EmbeddingConfig, EmbeddingStrategy, NostrEmbeddedBitChat, BITCHAT_EMBEDDING_PREFIX};
pub use error::NostrTransportError;
pub use message::{BitchatNostrMessage, BITCHAT_KIND};
pub use nip17::{Nip17Content, Nip17GiftUnwrapper, Nip17GiftWrapper, BITCHAT_NIP17_PREFIX};
pub use relay_manager::{GeoRelayDirectory, NostrRelayManager, RelayHealth, RelayInfo, RelaySelectionStrategy};
pub use transport::NostrTransportTask;

// Re-export TransportTask trait for convenience
pub use bitchat_core::transport_task::TransportTask;
