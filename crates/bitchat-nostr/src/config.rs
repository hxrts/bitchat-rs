//! Configuration for Nostr transport

use nostr_sdk::Keys;
use std::time::Duration;

// ----------------------------------------------------------------------------
// Nostr Configuration
// ----------------------------------------------------------------------------

/// Configuration for Nostr transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NostrTransportConfig {
    /// List of Nostr relay URLs
    pub relay_urls: Vec<String>,
    /// Connection timeout for relays
    pub connection_timeout: Duration,
    /// Maximum time to wait for message delivery
    pub message_timeout: Duration,
    /// Maximum packet size
    pub max_packet_size: usize,
    /// Whether to automatically reconnect to relays
    pub auto_reconnect: bool,
    /// Private key for Nostr identity (None = generate random)
    #[serde(skip)]
    pub private_key: Option<Keys>,
}

impl Default for NostrTransportConfig {
    fn default() -> Self {
        Self {
            relay_urls: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            connection_timeout: Duration::from_secs(10),
            message_timeout: Duration::from_secs(30),
            max_packet_size: 64000, // Nostr event content limit
            auto_reconnect: true,
            private_key: None,
        }
    }
}

/// Create a configuration for local development with a local relay
pub fn create_local_relay_config() -> NostrTransportConfig {
    NostrTransportConfig {
        relay_urls: vec!["ws://localhost:7777".to_string()],
        connection_timeout: Duration::from_secs(5),
        message_timeout: Duration::from_secs(10),
        max_packet_size: 64000,
        auto_reconnect: true,
        private_key: None,
    }
}
