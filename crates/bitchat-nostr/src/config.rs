//! Configuration for Nostr transport task

use nostr_sdk::{Keys, ToBech32, FromBech32};
use serde::Deserialize;
use std::time::Duration;

// ----------------------------------------------------------------------------
// Keys Serialization Helper Functions
// ----------------------------------------------------------------------------

fn serialize_keys<S>(keys: &Option<Keys>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match keys {
        Some(k) => serializer.serialize_some(&k.secret_key().unwrap().to_bech32().unwrap()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_keys<'de, D>(deserializer: D) -> Result<Option<Keys>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt_string: Option<String> = Deserialize::deserialize(deserializer)?;
    match opt_string {
        Some(s) => {
            let secret_key = nostr_sdk::SecretKey::from_bech32(&s)
                .map_err(|e| serde::de::Error::custom(format!("Invalid secret key: {}", e)))?;
            Ok(Some(Keys::new(secret_key)))
        }
        None => Ok(None),
    }
}

// ----------------------------------------------------------------------------
// Nostr Configuration
// ----------------------------------------------------------------------------

/// Configuration for individual Nostr relays
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NostrRelayConfig {
    /// Relay URL
    pub url: String,
    /// Relay-specific connection timeout
    pub connection_timeout: Duration,
    /// Whether this relay is read-only
    pub read_only: bool,
}

impl NostrRelayConfig {
    pub fn new(url: String) -> Self {
        Self {
            url,
            connection_timeout: Duration::from_secs(10),
            read_only: false,
        }
    }
}

/// Configuration for Nostr transport task
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NostrConfig {
    /// List of Nostr relay configurations
    pub relays: Vec<NostrRelayConfig>,
    /// Global connection timeout for relays
    pub connection_timeout: Duration,
    /// Maximum time to wait for message delivery
    pub message_timeout: Duration,
    /// Maximum data size for transport layer
    pub max_data_size: usize,
    /// Whether to automatically reconnect to relays
    pub auto_reconnect: bool,
    /// Reconnection retry interval
    pub reconnect_interval: Duration,
    /// Private key for Nostr identity (required)
    #[serde(
        serialize_with = "serialize_keys",
        deserialize_with = "deserialize_keys",
        skip_serializing_if = "Option::is_none"
    )]
    pub private_key: Option<Keys>,
}

impl Default for NostrConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                NostrRelayConfig::new("wss://relay.damus.io".to_string()),
                NostrRelayConfig::new("wss://nos.lol".to_string()),
                NostrRelayConfig::new("wss://relay.nostr.band".to_string()),
            ],
            connection_timeout: Duration::from_secs(10),
            message_timeout: Duration::from_secs(30),
            max_data_size: 64000, // Nostr event content limit
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(5),
            private_key: Some(Keys::generate()), // Generate new identity by default
        }
    }
}

impl NostrConfig {
    /// Create a configuration for local development with a local relay
    pub fn local_development() -> Self {
        Self {
            relays: vec![NostrRelayConfig::new("ws://localhost:7777".to_string())],
            connection_timeout: Duration::from_secs(5),
            message_timeout: Duration::from_secs(10),
            max_data_size: 64000,
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(2),
            private_key: Some(Keys::generate()), // Generate new identity for local dev
        }
    }

    /// Create a configuration with a specific private key
    pub fn with_private_key(keys: Keys) -> Self {
        Self {
            private_key: Some(keys),
            ..Self::default()
        }
    }

    /// Get the private key, generating one if none exists
    pub fn get_or_generate_keys(&mut self) -> &Keys {
        if self.private_key.is_none() {
            self.private_key = Some(Keys::generate());
        }
        self.private_key.as_ref().unwrap()
    }

    /// Add a relay to the configuration
    pub fn add_relay(&mut self, url: String) {
        self.relays.push(NostrRelayConfig::new(url));
    }

    /// Create a configuration with a single relay (for testing)
    pub fn default_with_relay(relay_url: &str) -> Self {
        Self {
            relays: vec![NostrRelayConfig::new(relay_url.to_string())],
            connection_timeout: Duration::from_secs(5),
            message_timeout: Duration::from_secs(10),
            max_data_size: 64000,
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(2),
            private_key: Some(Keys::generate()),
        }
    }
}
