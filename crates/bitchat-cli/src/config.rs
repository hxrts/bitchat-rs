//! BitChat CLI Configuration Management
//!
//! This module provides robust configuration loading for the BitChat CLI application.
//! It supports loading configuration from:
//! - Configuration files (bitchat.toml)
//! - Environment variables (BITCHAT_*)
//! - Command line arguments
//!
//! The configuration system uses figment for flexible, layered configuration loading
//! with proper priority ordering: CLI args > env vars > config file > defaults.

use std::path::PathBuf;
use std::time::Duration;

use figment::{Figment, providers::{Format, Toml, Env, Serialized}};
use serde::{Deserialize, Serialize};

use bitchat_core::{
    PeerId, ChannelTransportType,
    internal::BitchatConfig,
};
use bitchat_ble::BleTransportConfig;
use bitchat_nostr::NostrConfig;

// ----------------------------------------------------------------------------
// CLI Application Configuration
// ----------------------------------------------------------------------------

/// Complete configuration for the BitChat CLI application
/// 
/// This struct consolidates all configuration needed by the CLI:
/// - Core BitChat configuration from bitchat-core
/// - Transport-specific configurations (BLE, Nostr)
/// - CLI-specific settings (logging, interface, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliAppConfig {
    /// Core BitChat protocol configuration
    pub core: BitchatConfig,
    
    /// BLE transport configuration
    pub ble: BleTransportConfig,
    
    /// Nostr transport configuration
    pub nostr: NostrConfig,
    
    /// CLI-specific configuration
    pub cli: CliConfig,
    
    /// Application identity configuration
    pub identity: IdentityConfig,
    
    /// Runtime behavior configuration
    pub runtime: RuntimeConfig,
}

/// CLI-specific configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Enable verbose logging output
    pub verbose: bool,
    
    /// Prompt style for the interactive interface
    pub prompt: String,
    
    /// Maximum number of recent messages to display
    pub max_recent_messages: usize,
    
    /// Whether to auto-start discovery on startup
    pub auto_start_discovery: bool,
    
    /// Whether to use colored output
    pub colored_output: bool,
    
    /// Update interval for status display (in milliseconds)
    pub status_update_interval_ms: u64,
    
    /// Command history size
    pub history_size: usize,
}

/// Identity and peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    /// Fixed peer ID (hex string, 16 characters)
    /// If None, will be generated from name or randomly
    pub peer_id: Option<String>,
    
    /// Human-readable name for this peer
    /// Used to generate consistent peer ID if peer_id is not set
    pub name: Option<String>,
    
    /// Whether to save generated identity to file for reuse
    pub persist_identity: bool,
    
    /// Path to identity file (defaults to ~/.bitchat/identity.toml)
    pub identity_file: Option<PathBuf>,
}

/// Runtime behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Which transports to enable by default
    pub enabled_transports: Vec<String>, // ["ble", "nostr"]
    
    /// Startup timeout in seconds
    pub startup_timeout_secs: u64,
    
    /// Shutdown timeout in seconds
    pub shutdown_timeout_secs: u64,
    
    /// Whether to exit on first error or retry
    pub exit_on_error: bool,
    
    /// Heartbeat interval for health checking (in seconds)
    pub heartbeat_interval_secs: u64,
}

// ----------------------------------------------------------------------------
// Default Implementations
// ----------------------------------------------------------------------------

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            prompt: "bitchat> ".to_string(),
            max_recent_messages: 5,
            auto_start_discovery: true,
            colored_output: true,
            status_update_interval_ms: 1000,
            history_size: 100,
        }
    }
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            peer_id: None,
            name: None,
            persist_identity: true,
            identity_file: None, // Will default to ~/.bitchat/identity.toml
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enabled_transports: vec!["ble".to_string(), "nostr".to_string()],
            startup_timeout_secs: 30,
            shutdown_timeout_secs: 10,
            exit_on_error: false,
            heartbeat_interval_secs: 30,
        }
    }
}

impl Default for CliAppConfig {
    fn default() -> Self {
        Self {
            core: BitchatConfig::default(),
            ble: BleTransportConfig::default(),
            nostr: NostrConfig::default(),
            cli: CliConfig::default(),
            identity: IdentityConfig::default(),
            runtime: RuntimeConfig::default(),
        }
    }
}

// ----------------------------------------------------------------------------
// Configuration Loading Logic
// ----------------------------------------------------------------------------

impl CliAppConfig {
    /// Load configuration with the standard priority order:
    /// 1. Command line arguments (highest priority)
    /// 2. Environment variables
    /// 3. Configuration file (bitchat.toml)
    /// 4. Default values (lowest priority)
    pub fn load() -> Result<Self, ConfigError> {
        let figment = Figment::new()
            // Start with defaults
            .merge(Serialized::defaults(Self::default()))
            // Load from configuration file if it exists
            .merge(Toml::file("bitchat.toml"))
            .merge(Toml::file(Self::default_config_path()?))
            // Load from environment variables with BITCHAT_ prefix
            .merge(Env::prefixed("BITCHAT_").split("_"));

        let config: CliAppConfig = figment.extract()
            .map_err(|e| ConfigError::Loading(format!("Failed to load configuration: {}", e)))?;

        // Validate the loaded configuration
        config.validate()?;

        Ok(config)
    }

    /// Load configuration from a specific file path
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ConfigError> {
        let figment = Figment::new()
            .merge(Serialized::defaults(Self::default()))
            .merge(Toml::file(path.as_ref()));

        let config: CliAppConfig = figment.extract()
            .map_err(|e| ConfigError::Loading(format!("Failed to load from {}: {}", path.as_ref().display(), e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Create a configuration with command line overrides
    pub fn load_with_overrides(
        peer_id: Option<String>,
        name: Option<String>,
        verbose: Option<bool>,
        transports: Option<Vec<String>>,
    ) -> Result<Self, ConfigError> {
        let mut figment = Figment::new()
            .merge(Serialized::defaults(Self::default()))
            .merge(Toml::file("bitchat.toml"))
            .merge(Toml::file(Self::default_config_path()?))
            .merge(Env::prefixed("BITCHAT_").split("_"));

        // Apply command line overrides
        if let Some(pid) = peer_id {
            figment = figment.merge(("identity.peer_id", pid));
        }
        if let Some(n) = name {
            figment = figment.merge(("identity.name", n));
        }
        if let Some(v) = verbose {
            figment = figment.merge(("cli.verbose", v));
        }
        if let Some(t) = transports {
            figment = figment.merge(("runtime.enabled_transports", t));
        }

        let config: CliAppConfig = figment.extract()
            .map_err(|e| ConfigError::Loading(format!("Failed to load with overrides: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Get the default configuration file path
    fn default_config_path() -> Result<PathBuf, ConfigError> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| ConfigError::Environment("No HOME or USERPROFILE environment variable".to_string()))?;
        
        Ok(PathBuf::from(home).join(".bitchat").join("config.toml"))
    }

    /// Save configuration to the default config file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::default_config_path()?;
        self.save_to_file(config_path)
    }

    /// Save configuration to a specific file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::FileSystem(format!("Failed to create config directory: {}", e)))?;
        }

        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Serialization(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path.as_ref(), toml_string)
            .map_err(|e| ConfigError::FileSystem(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Validate the configuration for consistency and correctness
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate peer ID format if provided
        if let Some(ref peer_id_str) = self.identity.peer_id {
            let _peer_id = self.parse_peer_id(peer_id_str)?;
        }

        // Validate enabled transports
        for transport in &self.runtime.enabled_transports {
            match transport.as_str() {
                "ble" | "nostr" => {}
                _ => return Err(ConfigError::Validation(format!("Unknown transport: {}", transport))),
            }
        }

        // Validate timeouts are reasonable
        if self.runtime.startup_timeout_secs == 0 {
            return Err(ConfigError::Validation("Startup timeout must be greater than 0".to_string()));
        }

        // Validate BLE configuration
        if self.ble.max_packet_size == 0 {
            return Err(ConfigError::Validation("BLE max packet size must be greater than 0".to_string()));
        }

        // Validate Nostr configuration
        if self.nostr.relays.is_empty() {
            return Err(ConfigError::Validation("At least one Nostr relay must be configured".to_string()));
        }

        Ok(())
    }

    /// Get the effective peer ID, generating one if necessary
    pub fn get_peer_id(&self) -> Result<PeerId, ConfigError> {
        if let Some(ref peer_id_str) = self.identity.peer_id {
            // Use explicit peer ID
            return self.parse_peer_id(peer_id_str);
        }

        if let Some(ref name) = self.identity.name {
            // Generate peer ID from name for consistency
            return Ok(self.generate_peer_id_from_name(name));
        }

        // Generate random peer ID
        Ok(self.generate_random_peer_id())
    }

    /// Parse a peer ID from hex string
    fn parse_peer_id(&self, peer_id_str: &str) -> Result<PeerId, ConfigError> {
        let peer_bytes = hex::decode(peer_id_str)
            .map_err(|_| ConfigError::Validation(format!("Invalid peer ID hex format: {}", peer_id_str)))?;

        if peer_bytes.len() != 8 {
            return Err(ConfigError::Validation(format!(
                "Peer ID must be exactly 8 bytes (16 hex chars), got {} bytes", 
                peer_bytes.len()
            )));
        }

        let mut peer_id_bytes = [0u8; 8];
        peer_id_bytes.copy_from_slice(&peer_bytes);
        Ok(PeerId::new(peer_id_bytes))
    }

    /// Generate a consistent peer ID from a name
    fn generate_peer_id_from_name(&self, name: &str) -> PeerId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let hash = hasher.finish();
        
        let peer_bytes = [
            (hash >> 56) as u8,
            (hash >> 48) as u8,
            (hash >> 40) as u8,
            (hash >> 32) as u8,
            (hash >> 24) as u8,
            (hash >> 16) as u8,
            (hash >> 8) as u8,
            hash as u8,
        ];
        
        PeerId::new(peer_bytes)
    }

    /// Generate a random peer ID
    fn generate_random_peer_id(&self) -> PeerId {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Generate pseudo-random peer ID from current time + some entropy
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        
        let peer_bytes = [
            (timestamp >> 56) as u8,
            (timestamp >> 48) as u8,
            (timestamp >> 40) as u8,
            (timestamp >> 32) as u8,
            (timestamp >> 24) as u8,
            (timestamp >> 16) as u8,
            (timestamp >> 8) as u8,
            timestamp as u8,
        ];
        
        PeerId::new(peer_bytes)
    }

    /// Get enabled transports as enum values
    pub fn get_enabled_transports(&self) -> Vec<ChannelTransportType> {
        self.runtime.enabled_transports
            .iter()
            .filter_map(|t| match t.as_str() {
                "ble" => Some(ChannelTransportType::Ble),
                "nostr" => Some(ChannelTransportType::Nostr),
                _ => None,
            })
            .collect()
    }

    /// Create example configuration file content
    pub fn example_config() -> String {
        let example_config = CliAppConfig {
            cli: CliConfig {
                verbose: false,
                prompt: "bitchat> ".to_string(),
                max_recent_messages: 5,
                auto_start_discovery: true,
                colored_output: true,
                status_update_interval_ms: 1000,
                history_size: 100,
            },
            identity: IdentityConfig {
                peer_id: Some("0102030405060708".to_string()),
                name: Some("my-node".to_string()),
                persist_identity: true,
                identity_file: None,
            },
            runtime: RuntimeConfig {
                enabled_transports: vec!["ble".to_string(), "nostr".to_string()],
                startup_timeout_secs: 30,
                shutdown_timeout_secs: 10,
                exit_on_error: false,
                heartbeat_interval_secs: 30,
            },
            ..Default::default()
        };

        toml::to_string_pretty(&example_config).unwrap_or_else(|_| {
            "# Failed to generate example config".to_string()
        })
    }
}

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Configuration-related errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration loading error: {0}")]
    Loading(String),

    #[error("Configuration validation error: {0}")]
    Validation(String),

    #[error("Environment error: {0}")]
    Environment(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_creation() {
        let config = CliAppConfig::default();
        assert!(!config.cli.verbose);
        assert_eq!(config.cli.prompt, "bitchat> ");
        assert!(config.runtime.enabled_transports.contains(&"ble".to_string()));
        assert!(config.runtime.enabled_transports.contains(&"nostr".to_string()));
    }

    #[test]
    fn test_config_validation() {
        let config = CliAppConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid peer ID
        let mut invalid_config = config.clone();
        invalid_config.identity.peer_id = Some("invalid".to_string());
        assert!(invalid_config.validate().is_err());

        // Test invalid transport
        let mut invalid_config = config.clone();
        invalid_config.runtime.enabled_transports = vec!["invalid".to_string()];
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_peer_id_generation() {
        let config = CliAppConfig::default();

        // Test explicit peer ID
        let mut config_with_id = config.clone();
        config_with_id.identity.peer_id = Some("0102030405060708".to_string());
        let peer_id = config_with_id.get_peer_id().unwrap();
        assert_eq!(peer_id.to_string(), "0102030405060708");

        // Test name-based generation
        let mut config_with_name = config.clone();
        config_with_name.identity.name = Some("test".to_string());
        let peer_id1 = config_with_name.get_peer_id().unwrap();
        let peer_id2 = config_with_name.get_peer_id().unwrap();
        assert_eq!(peer_id1, peer_id2); // Should be consistent

        // Test random generation
        let peer_id_random = config.get_peer_id().unwrap();
        assert_ne!(peer_id_random.to_string(), "0000000000000000");
    }

    #[test]
    fn test_transport_parsing() {
        let mut config = CliAppConfig::default();
        config.runtime.enabled_transports = vec!["ble".to_string(), "nostr".to_string()];
        
        let transports = config.get_enabled_transports();
        assert_eq!(transports.len(), 2);
        assert!(transports.contains(&ChannelTransportType::Ble));
        assert!(transports.contains(&ChannelTransportType::Nostr));
    }

    #[test]
    fn test_example_config_generation() {
        let example = CliAppConfig::example_config();
        assert!(!example.is_empty());
        assert!(example.contains("[cli]"));
        assert!(example.contains("[identity]"));
        assert!(example.contains("[runtime]"));
    }
}