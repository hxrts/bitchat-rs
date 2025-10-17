//! Configuration management for the BitChat CLI

use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};

use bitchat_ble_transport::BleTransportConfig;
use bitchat_nostr_transport::NostrTransportConfig;
use crate::error::{CliError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// User display name
    pub display_name: String,
    /// BLE transport configuration
    pub ble: BleTransportConfig,
    /// Nostr transport configuration
    pub nostr: NostrTransportConfig,
    /// Transport selection preferences
    pub transport_preferences: TransportPreferences,
    /// UI configuration
    pub ui: UiConfig,
    /// State persistence configuration
    pub state: StateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportPreferences {
    /// Prefer BLE over Nostr when both are available
    pub prefer_ble: bool,
    /// Fallback to secondary transport on failure
    pub enable_fallback: bool,
    /// Maximum time to wait for preferred transport
    pub preferred_timeout: Duration,
    /// Enable automatic peer discovery
    pub auto_discovery: bool,
    /// Peer discovery interval
    pub discovery_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Maximum number of messages to display in chat
    pub max_chat_messages: usize,
    /// Refresh rate for the TUI (milliseconds)
    pub refresh_rate_ms: u64,
    /// Show timestamps in chat
    pub show_timestamps: bool,
    /// Show peer IDs in chat
    pub show_peer_ids: bool,
    /// Enable message notifications
    pub enable_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    /// Whether to persist application state
    pub enable_persistence: bool,
    /// Directory for state files
    pub state_dir: Option<PathBuf>,
    /// Auto-save interval (seconds)
    pub auto_save_interval: Duration,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            display_name: "BitChat User".to_string(),
            ble: BleTransportConfig::default(),
            nostr: NostrTransportConfig::default(),
            transport_preferences: TransportPreferences::default(),
            ui: UiConfig::default(),
            state: StateConfig::default(),
        }
    }
}

impl Default for TransportPreferences {
    fn default() -> Self {
        Self {
            prefer_ble: true,
            enable_fallback: true,
            preferred_timeout: Duration::from_secs(5),
            auto_discovery: true,
            discovery_interval: Duration::from_secs(10),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            max_chat_messages: 100,
            refresh_rate_ms: 50,
            show_timestamps: true,
            show_peer_ids: false,
            enable_notifications: true,
        }
    }
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            enable_persistence: true,
            state_dir: None, // Will be set to appropriate default based on OS
            auto_save_interval: Duration::from_secs(30),
        }
    }
}

impl AppConfig {
    /// Load configuration from file
    pub fn load_from_file(path: &str) -> Result<Self> {
        let config_str = std::fs::read_to_string(path)
            .map_err(|e| CliError::Config(format!("Failed to read config file '{}': {}", path, e)))?;
        
        toml::from_str(&config_str)
            .map_err(|e| CliError::Config(format!("Failed to parse config file '{}': {}", path, e)))
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(path, config_str)
            .map_err(|e| CliError::Config(format!("Failed to write config file '{}': {}", path, e)))
    }

    /// Get the state directory, creating it if necessary
    pub fn get_state_dir(&self) -> Result<PathBuf> {
        let state_dir = if let Some(dir) = &self.state.state_dir {
            dir.clone()
        } else {
            // Use platform-appropriate default
            let mut dir = dirs::config_dir()
                .ok_or_else(|| CliError::Config("Failed to get config directory".to_string()))?;
            dir.push("bitchat");
            dir
        };

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&state_dir)
            .map_err(|e| CliError::StatePersistence(format!("Failed to create state directory: {}", e)))?;

        Ok(state_dir)
    }
}