//! Centralized Configuration Management
//!
//! This module consolidates all configuration structures used throughout BitChat Core
//! to provide a unified, consistent configuration interface.

use crate::{ChannelTransportType, PeerId};

#[cfg(feature = "task-logging")]
use crate::internal::LogLevel;
use alloc::sync::Arc;
use alloc::vec::Vec;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "std"))] {
        use alloc::vec;
        use alloc::boxed::Box;
    }
}
use core::time::Duration;

// ----------------------------------------------------------------------------
// Rate Limiting Configuration
// ----------------------------------------------------------------------------

/// Configuration for rate limiting
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RateLimitConfig {
    /// Maximum messages per peer per time window
    pub messages_per_peer_per_window: u32,
    /// Maximum connections per peer per time window
    pub connections_per_peer_per_window: u32,
    /// Maximum global messages per time window
    pub global_messages_per_window: u32,
    /// Maximum global connections per time window  
    pub global_connections_per_window: u32,
    /// Time window duration in seconds
    pub window_duration_secs: u64,
    /// Maximum number of peers to track (LRU eviction)
    pub max_tracked_peers: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            messages_per_peer_per_window: 50, // 50 messages per peer per window
            connections_per_peer_per_window: 5, // 5 connections per peer per window
            global_messages_per_window: 1000, // 1000 messages globally per window
            global_connections_per_window: 100, // 100 connections globally per window
            window_duration_secs: 60,         // 60 second windows
            max_tracked_peers: 1000,          // Track up to 1000 peers
        }
    }
}

impl RateLimitConfig {
    /// Create a permissive rate limit config for testing
    pub fn permissive() -> Self {
        Self {
            messages_per_peer_per_window: 1000,
            connections_per_peer_per_window: 100,
            global_messages_per_window: 10000,
            global_connections_per_window: 1000,
            window_duration_secs: 60,
            max_tracked_peers: 1000,
        }
    }

    /// Create a strict rate limit config for high-security environments
    pub fn strict() -> Self {
        Self {
            messages_per_peer_per_window: 10,
            connections_per_peer_per_window: 2,
            global_messages_per_window: 200,
            global_connections_per_window: 20,
            window_duration_secs: 60,
            max_tracked_peers: 500,
        }
    }
}

// ----------------------------------------------------------------------------
// Channel Configuration
// ----------------------------------------------------------------------------

/// Configuration for CSP channel buffer sizes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelConfig {
    /// Buffer size for Command channels (UI → Core Logic)
    pub command_buffer_size: usize,
    /// Buffer size for Event channels (Transport → Core Logic)
    pub event_buffer_size: usize,
    /// Buffer size for Effect channels (Core Logic → Transport)
    pub effect_buffer_size: usize,
    /// Buffer size for AppEvent channels (Core Logic → UI)
    pub app_event_buffer_size: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            command_buffer_size: 32,   // UI commands are infrequent
            event_buffer_size: 128,    // Network events can be bursty
            effect_buffer_size: 64,    // Effects are processed quickly
            app_event_buffer_size: 64, // UI updates need responsiveness
        }
    }
}

impl ChannelConfig {
    /// Create configuration optimized for browser memory constraints
    pub fn browser_optimized() -> Self {
        Self {
            command_buffer_size: 20,    // Smaller for memory efficiency
            event_buffer_size: 50,      // Moderate for responsiveness
            effect_buffer_size: 50,     // Moderate for transport coordination
            app_event_buffer_size: 100, // Larger for UI responsiveness
        }
    }

    /// Create configuration for high-memory environments
    pub fn high_memory() -> Self {
        Self {
            command_buffer_size: 100,
            event_buffer_size: 256,
            effect_buffer_size: 128,
            app_event_buffer_size: 200,
        }
    }

    /// Create configuration for low-memory environments
    pub fn low_memory() -> Self {
        Self {
            command_buffer_size: 10,
            event_buffer_size: 25,
            effect_buffer_size: 25,
            app_event_buffer_size: 50,
        }
    }

    /// Create configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            command_buffer_size: 100,
            event_buffer_size: 100,
            effect_buffer_size: 100,
            app_event_buffer_size: 100,
        }
    }
}

// ----------------------------------------------------------------------------
// Message Store Configuration
// ----------------------------------------------------------------------------

/// Configuration for message storage and validation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageStoreConfig {
    /// Maximum size for a single message content in bytes
    pub max_message_size: usize,
    /// Maximum number of messages to store per conversation
    pub max_messages_per_conversation: usize,
    /// Maximum total number of messages to store
    pub max_total_messages: usize,
    /// Maximum content length for messages (in characters, not bytes)
    pub max_content_length: usize,
    /// Maximum age for stored messages (in seconds)
    pub max_message_age_secs: u64,
    /// Whether to enforce strict content validation
    pub strict_content_validation: bool,
}

impl Default for MessageStoreConfig {
    fn default() -> Self {
        Self {
            max_message_size: 65536,              // 64KB per message
            max_messages_per_conversation: 10000, // 10K messages per conversation
            max_total_messages: 100000,           // 100K total messages
            max_content_length: 32768,            // 32K characters
            max_message_age_secs: 86400 * 30,     // 30 days
            strict_content_validation: true,
        }
    }
}

impl MessageStoreConfig {
    /// Create configuration optimized for low memory environments
    pub fn low_memory() -> Self {
        Self {
            max_message_size: 4096,              // 4KB per message
            max_messages_per_conversation: 1000, // 1K messages per conversation
            max_total_messages: 10000,           // 10K total messages
            max_content_length: 2048,            // 2K characters
            max_message_age_secs: 86400 * 7,     // 7 days
            strict_content_validation: true,
        }
    }

    /// Create configuration optimized for high capacity environments
    pub fn high_capacity() -> Self {
        Self {
            max_message_size: 1048576,             // 1MB per message
            max_messages_per_conversation: 100000, // 100K messages per conversation
            max_total_messages: 1000000,           // 1M total messages
            max_content_length: 524288,            // 512K characters
            max_message_age_secs: 86400 * 365,     // 1 year
            strict_content_validation: false,      // Less strict for high throughput
        }
    }

    /// Create configuration for testing with permissive limits
    pub fn testing() -> Self {
        Self {
            max_message_size: 1024,             // 1KB per message for testing
            max_messages_per_conversation: 100, // 100 messages per conversation
            max_total_messages: 1000,           // 1K total messages
            max_content_length: 512,            // 512 characters
            max_message_age_secs: 3600,         // 1 hour
            strict_content_validation: true,
        }
    }
}

// ----------------------------------------------------------------------------
// Delivery Configuration
// ----------------------------------------------------------------------------

/// Configuration for message delivery and retry behavior
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeliveryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay (for exponential backoff)
    pub max_retry_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f32,
    /// Timeout for delivery confirmation
    pub confirmation_timeout: Duration,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            confirmation_timeout: Duration::from_secs(60),
        }
    }
}

impl DeliveryConfig {
    /// Create configuration for aggressive retry behavior
    pub fn aggressive() -> Self {
        Self {
            max_retries: 10,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(10),
            backoff_multiplier: 1.5,
            confirmation_timeout: Duration::from_secs(30),
        }
    }

    /// Create configuration for conservative retry behavior
    pub fn conservative() -> Self {
        Self {
            max_retries: 3,
            initial_retry_delay: Duration::from_secs(2),
            max_retry_delay: Duration::from_secs(60),
            backoff_multiplier: 3.0,
            confirmation_timeout: Duration::from_secs(120),
        }
    }

    /// Create configuration optimized for testing (fast retries)
    pub fn testing() -> Self {
        Self {
            max_retries: 2,
            initial_retry_delay: Duration::from_millis(10),
            max_retry_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            confirmation_timeout: Duration::from_millis(500),
        }
    }
}

// ----------------------------------------------------------------------------
// Session Configuration
// ----------------------------------------------------------------------------

/// Configuration for session timeouts and lifecycle management
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionConfig {
    /// Maximum time for handshake completion
    pub handshake_timeout: Duration,
    /// Maximum idle time before session cleanup
    pub idle_timeout: Duration,
    /// Key rotation interval
    pub key_rotation_interval: Duration,
    /// Maximum number of concurrent sessions
    pub max_concurrent_sessions: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300), // 5 minutes
            key_rotation_interval: Duration::from_secs(3600), // 1 hour
            max_concurrent_sessions: 100,
        }
    }
}

impl SessionConfig {
    /// Create configuration for low-latency environments
    pub fn low_latency() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(120), // 2 minutes
            key_rotation_interval: Duration::from_secs(1800), // 30 minutes
            max_concurrent_sessions: 50,
        }
    }

    /// Create configuration for high-security environments
    pub fn high_security() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(600), // 10 minutes
            key_rotation_interval: Duration::from_secs(300), // 5 minutes
            max_concurrent_sessions: 20,
        }
    }

    /// Create configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            handshake_timeout: Duration::from_millis(100),
            idle_timeout: Duration::from_secs(10),
            key_rotation_interval: Duration::from_secs(60),
            max_concurrent_sessions: 10,
        }
    }
}

// ----------------------------------------------------------------------------
// Monitoring Configuration
// ----------------------------------------------------------------------------

#[cfg(feature = "monitoring")]
/// Configuration for monitoring and metrics collection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitoringConfig {
    /// Maximum number of communication events to retain
    pub max_comm_events: usize,
    /// Maximum number of performance samples to retain
    pub max_performance_samples: usize,
    /// Interval for collecting performance metrics
    pub metrics_interval: Duration,
    /// Enable detailed channel utilization tracking
    pub track_channel_utilization: bool,
    /// Enable task health monitoring
    pub enable_health_monitoring: bool,
    /// Log level for monitoring output
    pub log_level: LogLevel,
}

#[cfg(not(feature = "monitoring"))]
/// Configuration for monitoring (no-op when monitoring disabled)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitoringConfig {
    // Minimal fields for compatibility
}

#[cfg(feature = "monitoring")]
impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            max_comm_events: 1000,
            max_performance_samples: 500,
            metrics_interval: Duration::from_secs(1),
            track_channel_utilization: true,
            enable_health_monitoring: true,
            log_level: LogLevel::Info,
        }
    }
}

#[cfg(not(feature = "monitoring"))]
impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {}
    }
}

#[cfg(feature = "monitoring")]
impl MonitoringConfig {
    /// Create configuration for minimal monitoring (low overhead)
    pub fn minimal() -> Self {
        Self {
            max_comm_events: 100,
            max_performance_samples: 50,
            metrics_interval: Duration::from_secs(10),
            track_channel_utilization: false,
            enable_health_monitoring: false,
            log_level: LogLevel::Warn,
        }
    }

    /// Create configuration for detailed monitoring (high overhead)
    pub fn detailed() -> Self {
        Self {
            max_comm_events: 5000,
            max_performance_samples: 2000,
            metrics_interval: Duration::from_millis(500),
            track_channel_utilization: true,
            enable_health_monitoring: true,
            log_level: LogLevel::Debug,
        }
    }

    /// Create configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            max_comm_events: 50,
            max_performance_samples: 25,
            metrics_interval: Duration::from_millis(100),
            track_channel_utilization: true,
            enable_health_monitoring: true,
            log_level: LogLevel::Debug,
        }
    }
}

#[cfg(not(feature = "monitoring"))]
impl MonitoringConfig {
    /// Create configuration for minimal monitoring (no-op)
    pub fn minimal() -> Self {
        Self {}
    }

    /// Create configuration for detailed monitoring (no-op)
    pub fn detailed() -> Self {
        Self {}
    }

    /// Create configuration optimized for testing (no-op)
    pub fn testing() -> Self {
        Self {}
    }
}

// ----------------------------------------------------------------------------
// Test Configuration
// ----------------------------------------------------------------------------

/// Configuration for test orchestration and execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestConfig {
    /// Peer identity for this test instance
    pub peer_id: PeerId,
    /// Whether to enable verbose logging
    pub enable_logging: bool,
    /// Transports to activate
    pub active_transports: Vec<ChannelTransportType>,
    /// Test duration in seconds (None = run until stopped)
    pub test_duration: Option<u64>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            peer_id: PeerId::new([0; 8]), // Will be randomized in new()
            enable_logging: false,
            active_transports: vec![ChannelTransportType::Ble, ChannelTransportType::Nostr],
            test_duration: None,
        }
    }
}

impl TestConfig {
    /// Create new test config with random peer ID
    pub fn new() -> Self {
        cfg_if::cfg_if! {
            if #[cfg(feature = "std")] {
                use std::time::{SystemTime, UNIX_EPOCH};

                // Generate pseudo-random peer ID from current time
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

                Self {
                    peer_id: PeerId::new(peer_bytes),
                    ..Default::default()
                }
            } else if #[cfg(feature = "wasm")] {
                use crate::types::Timestamp;

                // Generate pseudo-random peer ID from WASM timestamp
                let timestamp = Timestamp::now().as_millis();

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

                Self {
                    peer_id: PeerId::new(peer_bytes),
                    ..Default::default()
                }
            } else {
                // For no_std environments without time access, use a fixed ID
                Self {
                    peer_id: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
                    ..Default::default()
                }
            }
        }
    }

    /// Enable verbose logging
    pub fn with_logging(mut self) -> Self {
        self.enable_logging = true;
        self
    }

    /// Set specific peer ID
    pub fn with_peer_id(mut self, peer_id: PeerId) -> Self {
        self.peer_id = peer_id;
        self
    }

    /// Set active transports
    pub fn with_transports(mut self, transports: Vec<ChannelTransportType>) -> Self {
        self.active_transports = transports;
        self
    }

    /// Set test duration
    pub fn with_duration(mut self, seconds: u64) -> Self {
        self.test_duration = Some(seconds);
        self
    }
}

// ----------------------------------------------------------------------------
// Master Configuration
// ----------------------------------------------------------------------------

/// Master configuration struct that consolidates all BitChat configurations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct BitchatConfig {
    /// Channel buffer configuration
    pub channels: ChannelConfig,
    /// Message delivery configuration
    pub delivery: DeliveryConfig,
    /// Message store configuration
    pub message_store: MessageStoreConfig,
    /// Session management configuration
    pub session: SessionConfig,
    /// Monitoring and metrics configuration
    pub monitoring: MonitoringConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
    /// Test configuration (optional, used in testing contexts)
    pub test: Option<TestConfig>,
}

impl BitchatConfig {
    /// Create new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new builder for BitchatConfig
    pub fn builder() -> BitchatConfigBuilder {
        BitchatConfigBuilder::new()
    }

    /// Create configuration optimized for browser/WASM environments
    pub fn browser_optimized() -> Self {
        Self {
            channels: ChannelConfig::browser_optimized(),
            delivery: DeliveryConfig::conservative(), // Be gentle on browser resources
            message_store: MessageStoreConfig::low_memory(),
            session: SessionConfig::low_latency(),
            monitoring: MonitoringConfig::minimal(),
            rate_limiting: RateLimitConfig::default(),
            test: None,
        }
    }

    /// Create configuration optimized for server/high-performance environments
    pub fn server_optimized() -> Self {
        Self {
            channels: ChannelConfig::high_memory(),
            delivery: DeliveryConfig::aggressive(),
            message_store: MessageStoreConfig::high_capacity(),
            session: SessionConfig::default(),
            monitoring: MonitoringConfig::detailed(),
            rate_limiting: RateLimitConfig::permissive(),
            test: None,
        }
    }

    /// Create configuration optimized for mobile/low-power environments
    pub fn mobile_optimized() -> Self {
        Self {
            channels: ChannelConfig::low_memory(),
            delivery: DeliveryConfig::conservative(),
            message_store: MessageStoreConfig::low_memory(),
            session: SessionConfig::low_latency(),
            monitoring: MonitoringConfig::minimal(),
            rate_limiting: RateLimitConfig::strict(),
            test: None,
        }
    }

    /// Create configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            channels: ChannelConfig::testing(),
            delivery: DeliveryConfig::testing(),
            message_store: MessageStoreConfig::testing(),
            session: SessionConfig::testing(),
            monitoring: MonitoringConfig::testing(),
            rate_limiting: RateLimitConfig::permissive(),
            test: Some(TestConfig::new()),
        }
    }

    /// Create configuration optimized for high-security environments
    pub fn high_security() -> Self {
        Self {
            channels: ChannelConfig::default(),
            delivery: DeliveryConfig::conservative(),
            message_store: MessageStoreConfig::default(),
            session: SessionConfig::high_security(),
            monitoring: MonitoringConfig::detailed(),
            rate_limiting: RateLimitConfig::strict(),
            test: None,
        }
    }

    /// Builder method for customizing channel configuration
    pub fn with_channels(mut self, channels: ChannelConfig) -> Self {
        self.channels = channels;
        self
    }

    /// Builder method for customizing delivery configuration
    pub fn with_delivery(mut self, delivery: DeliveryConfig) -> Self {
        self.delivery = delivery;
        self
    }

    /// Builder method for customizing session configuration
    pub fn with_session(mut self, session: SessionConfig) -> Self {
        self.session = session;
        self
    }

    /// Builder method for customizing monitoring configuration
    pub fn with_monitoring(mut self, monitoring: MonitoringConfig) -> Self {
        self.monitoring = monitoring;
        self
    }

    /// Builder method for customizing rate limiting configuration
    pub fn with_rate_limiting(mut self, rate_limiting: RateLimitConfig) -> Self {
        self.rate_limiting = rate_limiting;
        self
    }

    /// Builder method for adding test configuration
    pub fn with_test(mut self, test: TestConfig) -> Self {
        self.test = Some(test);
        self
    }

    /// Validate the configuration for consistency and feasibility
    pub fn validate(&self) -> Result<(), alloc::string::String> {
        // Validate channel buffer sizes
        if self.channels.command_buffer_size == 0 {
            return Err("Command buffer size cannot be zero".into());
        }
        if self.channels.event_buffer_size == 0 {
            return Err("Event buffer size cannot be zero".into());
        }
        if self.channels.effect_buffer_size == 0 {
            return Err("Effect buffer size cannot be zero".into());
        }
        if self.channels.app_event_buffer_size == 0 {
            return Err("App event buffer size cannot be zero".into());
        }

        // Validate delivery configuration
        if self.delivery.max_retries == 0 {
            return Err("Max retries cannot be zero".into());
        }
        if self.delivery.backoff_multiplier <= 1.0 {
            return Err("Backoff multiplier must be greater than 1.0".into());
        }
        if self.delivery.initial_retry_delay > self.delivery.max_retry_delay {
            return Err("Initial retry delay cannot be greater than max retry delay".into());
        }

        // Validate session configuration
        if self.session.max_concurrent_sessions == 0 {
            return Err("Max concurrent sessions cannot be zero".into());
        }
        if self.session.handshake_timeout > self.session.idle_timeout {
            return Err("Handshake timeout should not exceed idle timeout".into());
        }

        // Validate monitoring configuration (only when monitoring is enabled)
        #[cfg(feature = "monitoring")]
        {
            if self.monitoring.max_comm_events == 0 {
                return Err("Max communication events cannot be zero".into());
            }
            if self.monitoring.max_performance_samples == 0 {
                return Err("Max performance samples cannot be zero".into());
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Configuration Presets
// ----------------------------------------------------------------------------

/// Predefined configuration presets for common use cases
pub struct ConfigPresets;

impl ConfigPresets {
    /// Development configuration (balanced for development work)
    pub fn development() -> BitchatConfig {
        BitchatConfig {
            channels: ChannelConfig::default(),
            delivery: DeliveryConfig::default(),
            message_store: MessageStoreConfig::default(),
            session: SessionConfig::default(),
            monitoring: MonitoringConfig::detailed(),
            rate_limiting: RateLimitConfig::permissive(),
            test: Some(TestConfig::new().with_logging()),
        }
    }

    /// Production configuration (optimized for production use)
    pub fn production() -> BitchatConfig {
        BitchatConfig {
            channels: ChannelConfig::default(),
            delivery: DeliveryConfig::default(),
            message_store: MessageStoreConfig::default(),
            session: SessionConfig::default(),
            monitoring: MonitoringConfig::default(),
            rate_limiting: RateLimitConfig::default(),
            test: None,
        }
    }

    /// Embedded configuration (minimal resource usage)
    pub fn embedded() -> BitchatConfig {
        BitchatConfig {
            channels: ChannelConfig::low_memory(),
            delivery: DeliveryConfig::conservative(),
            message_store: MessageStoreConfig::low_memory(),
            session: SessionConfig::low_latency(),
            monitoring: MonitoringConfig::minimal(),
            rate_limiting: RateLimitConfig::strict(),
            test: None,
        }
    }
}

// ----------------------------------------------------------------------------
// Arc-wrapped Configuration for Efficient Sharing
// ----------------------------------------------------------------------------

/// Arc-wrapped BitchatConfig for efficient sharing across tasks and components
pub type SharedBitchatConfig = Arc<BitchatConfig>;

/// Arc-wrapped ChannelConfig for efficient sharing
pub type SharedChannelConfig = Arc<ChannelConfig>;

/// Arc-wrapped DeliveryConfig for efficient sharing
pub type SharedDeliveryConfig = Arc<DeliveryConfig>;

/// Arc-wrapped MessageStoreConfig for efficient sharing
pub type SharedMessageStoreConfig = Arc<MessageStoreConfig>;

/// Arc-wrapped SessionConfig for efficient sharing  
pub type SharedSessionConfig = Arc<SessionConfig>;

/// Arc-wrapped MonitoringConfig for efficient sharing
pub type SharedMonitoringConfig = Arc<MonitoringConfig>;

/// Arc-wrapped RateLimitConfig for efficient sharing
pub type SharedRateLimitConfig = Arc<RateLimitConfig>;

impl BitchatConfig {
    /// Convert to Arc-wrapped config for efficient sharing
    pub fn into_shared(self) -> SharedBitchatConfig {
        Arc::new(self)
    }

    /// Create a shared config with default values
    pub fn shared() -> SharedBitchatConfig {
        Arc::new(Self::default())
    }

    /// Create a shared browser-optimized config
    pub fn shared_browser_optimized() -> SharedBitchatConfig {
        Arc::new(Self::browser_optimized())
    }

    /// Create a shared server-optimized config
    pub fn shared_server_optimized() -> SharedBitchatConfig {
        Arc::new(Self::server_optimized())
    }

    /// Create a shared mobile-optimized config
    pub fn shared_mobile_optimized() -> SharedBitchatConfig {
        Arc::new(Self::mobile_optimized())
    }

    /// Create a shared testing config
    pub fn shared_testing() -> SharedBitchatConfig {
        Arc::new(Self::testing())
    }

    /// Create a shared high-security config
    pub fn shared_high_security() -> SharedBitchatConfig {
        Arc::new(Self::high_security())
    }
}

// ----------------------------------------------------------------------------
// Configuration Builder Pattern
// ----------------------------------------------------------------------------

/// Builder for BitchatConfig that provides compile-time safety and validation
#[derive(Debug, Clone)]
pub struct BitchatConfigBuilder {
    channels: Option<ChannelConfig>,
    delivery: Option<DeliveryConfig>,
    message_store: Option<MessageStoreConfig>,
    session: Option<SessionConfig>,
    monitoring: Option<MonitoringConfig>,
    rate_limiting: Option<RateLimitConfig>,
    test: Option<TestConfig>,
}

/// Error type for configuration building
#[derive(Debug, Clone)]
pub struct ConfigBuilderError {
    pub message: alloc::string::String,
}

impl core::fmt::Display for ConfigBuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Configuration builder error: {}", self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigBuilderError {}

impl BitchatConfigBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            channels: None,
            delivery: None,
            message_store: None,
            session: None,
            monitoring: None,
            rate_limiting: None,
            test: None,
        }
    }

    /// Set channel configuration
    pub fn channels(mut self, config: ChannelConfig) -> Self {
        self.channels = Some(config);
        self
    }

    /// Set delivery configuration
    pub fn delivery(mut self, config: DeliveryConfig) -> Self {
        self.delivery = Some(config);
        self
    }

    /// Set message store configuration
    pub fn message_store(mut self, config: MessageStoreConfig) -> Self {
        self.message_store = Some(config);
        self
    }

    /// Set session configuration
    pub fn session(mut self, config: SessionConfig) -> Self {
        self.session = Some(config);
        self
    }

    /// Set monitoring configuration
    pub fn monitoring(mut self, config: MonitoringConfig) -> Self {
        self.monitoring = Some(config);
        self
    }

    /// Set rate limiting configuration
    pub fn rate_limiting(mut self, config: RateLimitConfig) -> Self {
        self.rate_limiting = Some(config);
        self
    }

    /// Set test configuration
    pub fn test(mut self, config: TestConfig) -> Self {
        self.test = Some(config);
        self
    }

    /// Use browser-optimized preset as base (can be further customized)
    pub fn browser_optimized(self) -> Self {
        Self {
            channels: Some(ChannelConfig::browser_optimized()),
            delivery: Some(DeliveryConfig::conservative()),
            message_store: Some(MessageStoreConfig::low_memory()),
            session: Some(SessionConfig::low_latency()),
            monitoring: Some(MonitoringConfig::minimal()),
            rate_limiting: Some(RateLimitConfig::default()),
            test: None,
        }
    }

    /// Use server-optimized preset as base (can be further customized)
    pub fn server_optimized(self) -> Self {
        Self {
            channels: Some(ChannelConfig::high_memory()),
            delivery: Some(DeliveryConfig::aggressive()),
            message_store: Some(MessageStoreConfig::high_capacity()),
            session: Some(SessionConfig::default()),
            monitoring: Some(MonitoringConfig::detailed()),
            rate_limiting: Some(RateLimitConfig::permissive()),
            test: None,
        }
    }

    /// Use mobile-optimized preset as base (can be further customized)
    pub fn mobile_optimized(self) -> Self {
        Self {
            channels: Some(ChannelConfig::low_memory()),
            delivery: Some(DeliveryConfig::conservative()),
            message_store: Some(MessageStoreConfig::low_memory()),
            session: Some(SessionConfig::low_latency()),
            monitoring: Some(MonitoringConfig::minimal()),
            rate_limiting: Some(RateLimitConfig::strict()),
            test: None,
        }
    }

    /// Use high-security preset as base (can be further customized)
    pub fn high_security(self) -> Self {
        Self {
            channels: Some(ChannelConfig::default()),
            delivery: Some(DeliveryConfig::conservative()),
            message_store: Some(MessageStoreConfig::default()),
            session: Some(SessionConfig::high_security()),
            monitoring: Some(MonitoringConfig::detailed()),
            rate_limiting: Some(RateLimitConfig::strict()),
            test: None,
        }
    }

    /// Use testing preset as base (can be further customized)
    pub fn testing(self) -> Self {
        Self {
            channels: Some(ChannelConfig::testing()),
            delivery: Some(DeliveryConfig::testing()),
            message_store: Some(MessageStoreConfig::testing()),
            session: Some(SessionConfig::testing()),
            monitoring: Some(MonitoringConfig::testing()),
            rate_limiting: Some(RateLimitConfig::permissive()),
            test: Some(TestConfig::new()),
        }
    }

    /// Build the configuration with validation
    pub fn build(self) -> Result<BitchatConfig, ConfigBuilderError> {
        let config = BitchatConfig {
            channels: self.channels.unwrap_or_default(),
            delivery: self.delivery.unwrap_or_default(),
            message_store: self.message_store.unwrap_or_default(),
            session: self.session.unwrap_or_default(),
            monitoring: self.monitoring.unwrap_or_default(),
            rate_limiting: self.rate_limiting.unwrap_or_default(),
            test: self.test,
        };

        // Validate the built configuration
        config
            .validate()
            .map_err(|msg| ConfigBuilderError { message: msg })?;

        Ok(config)
    }

    /// Build the configuration without validation (use with caution)
    pub fn build_unchecked(self) -> BitchatConfig {
        BitchatConfig {
            channels: self.channels.unwrap_or_default(),
            delivery: self.delivery.unwrap_or_default(),
            message_store: self.message_store.unwrap_or_default(),
            session: self.session.unwrap_or_default(),
            monitoring: self.monitoring.unwrap_or_default(),
            rate_limiting: self.rate_limiting.unwrap_or_default(),
            test: self.test,
        }
    }
}

impl Default for BitchatConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = BitchatConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_browser_optimized_config() {
        let config = BitchatConfig::browser_optimized();
        assert!(config.validate().is_ok());
        assert!(config.channels.command_buffer_size < ChannelConfig::default().command_buffer_size);
    }

    #[test]
    fn test_server_optimized_config() {
        let config = BitchatConfig::server_optimized();
        assert!(config.validate().is_ok());
        assert!(config.channels.command_buffer_size > ChannelConfig::default().command_buffer_size);
    }

    #[test]
    fn test_testing_config() {
        let config = BitchatConfig::testing();
        assert!(config.validate().is_ok());
        assert!(config.test.is_some());
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = BitchatConfig::new()
            .with_channels(ChannelConfig::high_memory())
            .with_delivery(DeliveryConfig::aggressive())
            .with_monitoring(MonitoringConfig::detailed());

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config_validation() {
        let mut config = BitchatConfig::default();
        config.channels.command_buffer_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_preset_configurations() {
        assert!(ConfigPresets::development().validate().is_ok());
        assert!(ConfigPresets::production().validate().is_ok());
        assert!(ConfigPresets::embedded().validate().is_ok());
    }

    #[test]
    fn test_builder_pattern() {
        let config = BitchatConfig::builder()
            .channels(ChannelConfig::high_memory())
            .delivery(DeliveryConfig::aggressive())
            .monitoring(MonitoringConfig::detailed())
            .build()
            .expect("Builder should create valid config");

        assert!(config.validate().is_ok());
        assert_eq!(config.channels.command_buffer_size, 100);
    }

    #[test]
    fn test_builder_presets() {
        let browser_config = BitchatConfig::builder()
            .browser_optimized()
            .build()
            .expect("Browser config should be valid");

        let server_config = BitchatConfig::builder()
            .server_optimized()
            .build()
            .expect("Server config should be valid");

        let mobile_config = BitchatConfig::builder()
            .mobile_optimized()
            .build()
            .expect("Mobile config should be valid");

        let security_config = BitchatConfig::builder()
            .high_security()
            .build()
            .expect("Security config should be valid");

        let test_config = BitchatConfig::builder()
            .testing()
            .build()
            .expect("Test config should be valid");

        assert!(browser_config.validate().is_ok());
        assert!(server_config.validate().is_ok());
        assert!(mobile_config.validate().is_ok());
        assert!(security_config.validate().is_ok());
        assert!(test_config.validate().is_ok());
    }

    #[test]
    fn test_builder_customization() {
        let config = BitchatConfig::builder()
            .browser_optimized()
            .delivery(DeliveryConfig::aggressive()) // Override the preset's conservative delivery
            .session(SessionConfig::high_security()) // Add high security session
            .build()
            .expect("Customized config should be valid");

        assert!(config.validate().is_ok());
        // Should retain browser-optimized channels but use aggressive delivery
        assert_eq!(config.channels.command_buffer_size, 20); // browser-optimized
    }

    #[test]
    fn test_builder_validation_error() {
        let result = BitchatConfigBuilder::new()
            .channels(ChannelConfig {
                command_buffer_size: 0, // Invalid
                ..ChannelConfig::default()
            })
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Command buffer size cannot be zero"));
    }

    #[test]
    fn test_builder_unchecked() {
        let config = BitchatConfigBuilder::new()
            .channels(ChannelConfig {
                command_buffer_size: 0, // Invalid but unchecked build will allow it
                ..ChannelConfig::default()
            })
            .build_unchecked();

        // Config was created but validation would fail
        assert!(config.validate().is_err());
    }
}
