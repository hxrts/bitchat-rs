//! Centralized Configuration Management
//!
//! This module consolidates all configuration structures used throughout BitChat Core
//! to provide a unified, consistent configuration interface.

use crate::{ChannelTransportType, PeerId};

#[cfg(feature = "task-logging")]
use crate::internal::LogLevel;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use core::time::Duration;
use serde::{Deserialize, Serialize};

cfg_if::cfg_if! {
    if #[cfg(not(feature = "std"))] {
        use alloc::vec;
    }
}

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
// Canonical Transport Configuration
// ----------------------------------------------------------------------------

/// BLE Transport configuration with canonical parameter compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleTransportConfig {
    // Core BLE parameters
    pub max_packet_size: usize,
    pub connection_timeout: Duration,
    pub scan_timeout: Duration,
    pub device_name_prefix: String,
    
    // Canonical fragmentation parameters
    pub fragment_size: usize,                    // bleDefaultFragmentSize: 469
    pub max_in_flight_assemblies: usize,         // bleMaxInFlightAssemblies: 128
    pub fragment_lifetime_secs: f64,             // bleFragmentLifetimeSeconds: 30.0
    pub expected_write_per_fragment_ms: u64,     // bleExpectedWritePerFragmentMs: 8
    pub expected_write_max_ms: u64,              // bleExpectedWriteMaxMs: 2000
    pub fragment_spacing_ms: u64,                // bleFragmentSpacingMs: 5
    pub fragment_spacing_directed_ms: u64,       // bleFragmentSpacingDirectedMs: 4
    
    // Canonical duty cycle parameters
    pub duty_on_duration: Duration,              // bleDutyOnDuration: 5.0s
    pub duty_off_duration: Duration,             // bleDutyOffDuration: 10.0s
    pub duty_on_duration_dense: Duration,        // bleDutyOnDurationDense: 3.0s
    pub duty_off_duration_dense: Duration,       // bleDutyOffDurationDense: 15.0s
    pub recent_traffic_force_scan_secs: f64,     // bleRecentTrafficForceScanSeconds: 10.0
    
    // Canonical connection and maintenance parameters
    pub max_central_links: usize,                // bleMaxCentralLinks: 6
    pub connect_rate_limit_interval: Duration,   // bleConnectRateLimitInterval: 0.5s
    pub maintenance_interval: Duration,          // bleMaintenanceInterval: 5.0s
    pub maintenance_leeway_secs: u64,            // bleMaintenanceLeewaySeconds: 1
    pub isolation_relax_threshold_secs: u64,     // bleIsolationRelaxThresholdSeconds: 60
    pub recent_timeout_window_secs: u64,         // bleRecentTimeoutWindowSeconds: 60
    pub peer_inactivity_timeout_secs: f64,       // blePeerInactivityTimeoutSeconds: 8.0
    
    // Canonical timing parameters
    pub announce_min_interval: Duration,         // bleAnnounceMinInterval: 1.0s
    pub initial_announce_delay_secs: f64,        // bleInitialAnnounceDelaySeconds: 0.6
    pub connect_timeout_secs: f64,               // bleConnectTimeoutSeconds: 8.0
    pub restart_scan_delay_secs: f64,            // bleRestartScanDelaySeconds: 0.1
    pub post_subscribe_announce_delay_secs: f64, // blePostSubscribeAnnounceDelaySeconds: 0.05
    pub post_announce_delay_secs: f64,           // blePostAnnounceDelaySeconds: 0.4
    pub force_announce_min_interval_secs: f64,   // bleForceAnnounceMinIntervalSeconds: 0.15
    
    // Canonical RSSI and connection parameters
    pub dynamic_rssi_threshold: i32,             // bleDynamicRSSIThresholdDefault: -90
    pub connection_candidates_max: usize,        // bleConnectionCandidatesMax: 100
    pub pending_write_buffer_cap: usize,         // blePendingWriteBufferCapBytes: 1M
    pub pending_notifications_cap: usize,        // blePendingNotificationsCapCount: 20
    pub rssi_isolated_base: i32,                 // bleRSSIIsolatedBase: -90
    pub rssi_isolated_relaxed: i32,              // bleRSSIIsolatedRelaxed: -92
    pub rssi_connected_threshold: i32,           // bleRSSIConnectedThreshold: -85
    pub rssi_high_timeout_threshold: i32,        // bleRSSIHighTimeoutThreshold: -80
    
    // Canonical network parameters
    pub high_degree_threshold: usize,            // bleHighDegreeThreshold: 6
    pub reachability_retention_verified_secs: f64,   // bleReachabilityRetentionVerifiedSeconds: 21.0
    pub reachability_retention_unverified_secs: f64, // bleReachabilityRetentionUnverifiedSeconds: 21.0
    pub ingress_record_lifetime_secs: f64,       // bleIngressRecordLifetimeSeconds: 3.0
    pub connect_timeout_backoff_window_secs: f64, // bleConnectTimeoutBackoffWindowSeconds: 120.0
    pub directed_spool_window_secs: f64,         // bleDirectedSpoolWindowSeconds: 15.0
    pub disconnect_notify_debounce_secs: f64,    // bleDisconnectNotifyDebounceSeconds: 0.9
    pub reconnect_log_debounce_secs: f64,        // bleReconnectLogDebounceSeconds: 2.0
    pub weak_link_cooldown_secs: f64,            // bleWeakLinkCooldownSeconds: 30.0
    pub weak_link_rssi_cutoff: i32,              // bleWeakLinkRSSICutoff: -90
    
    // Canonical packet tracking parameters
    pub recent_packet_window_secs: f64,          // bleRecentPacketWindowSeconds: 30.0
    pub recent_packet_window_max_count: usize,   // bleRecentPacketWindowMaxCount: 100
    pub announce_interval_secs: f64,             // bleAnnounceIntervalSeconds: 4.0
    pub connected_announce_base_secs_dense: f64, // bleConnectedAnnounceBaseSecondsDense: 30.0
    pub connected_announce_base_secs_sparse: f64, // bleConnectedAnnounceBaseSecondsSparse: 15.0
    pub connected_announce_jitter_dense: f64,    // bleConnectedAnnounceJitterDense: 8.0
    pub connected_announce_jitter_sparse: f64,   // bleConnectedAnnounceJitterSparse: 4.0
}

impl Default for BleTransportConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl BleTransportConfig {
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            // Core parameters (existing)
            max_packet_size: 512,
            connection_timeout: Duration::from_secs(8),
            scan_timeout: Duration::from_secs(10),
            device_name_prefix: "BitChat".to_string(),
            
            // Canonical fragmentation parameters
            fragment_size: 469,
            max_in_flight_assemblies: 128,
            fragment_lifetime_secs: 30.0,
            expected_write_per_fragment_ms: 8,
            expected_write_max_ms: 2000,
            fragment_spacing_ms: 5,
            fragment_spacing_directed_ms: 4,
            
            // Canonical duty cycle parameters
            duty_on_duration: Duration::from_secs(5),
            duty_off_duration: Duration::from_secs(10),
            duty_on_duration_dense: Duration::from_secs(3),
            duty_off_duration_dense: Duration::from_secs(15),
            recent_traffic_force_scan_secs: 10.0,
            
            // Canonical connection parameters
            max_central_links: 6,
            connect_rate_limit_interval: Duration::from_millis(500),
            maintenance_interval: Duration::from_secs(5),
            maintenance_leeway_secs: 1,
            isolation_relax_threshold_secs: 60,
            recent_timeout_window_secs: 60,
            peer_inactivity_timeout_secs: 8.0,
            
            // Canonical timing parameters
            announce_min_interval: Duration::from_secs(1),
            initial_announce_delay_secs: 0.6,
            connect_timeout_secs: 8.0,
            restart_scan_delay_secs: 0.1,
            post_subscribe_announce_delay_secs: 0.05,
            post_announce_delay_secs: 0.4,
            force_announce_min_interval_secs: 0.15,
            
            // Canonical RSSI parameters
            dynamic_rssi_threshold: -90,
            connection_candidates_max: 100,
            pending_write_buffer_cap: 1_000_000,
            pending_notifications_cap: 20,
            rssi_isolated_base: -90,
            rssi_isolated_relaxed: -92,
            rssi_connected_threshold: -85,
            rssi_high_timeout_threshold: -80,
            
            // Canonical network parameters
            high_degree_threshold: 6,
            reachability_retention_verified_secs: 21.0,
            reachability_retention_unverified_secs: 21.0,
            ingress_record_lifetime_secs: 3.0,
            connect_timeout_backoff_window_secs: 120.0,
            directed_spool_window_secs: 15.0,
            disconnect_notify_debounce_secs: 0.9,
            reconnect_log_debounce_secs: 2.0,
            weak_link_cooldown_secs: 30.0,
            weak_link_rssi_cutoff: -90,
            
            // Canonical packet tracking parameters
            recent_packet_window_secs: 30.0,
            recent_packet_window_max_count: 100,
            announce_interval_secs: 4.0,
            connected_announce_base_secs_dense: 30.0,
            connected_announce_base_secs_sparse: 15.0,
            connected_announce_jitter_dense: 8.0,
            connected_announce_jitter_sparse: 4.0,
        }
    }
    
    /// Create configuration optimized for battery life
    pub fn battery_optimized() -> Self {
        let mut config = Self::canonical();
        config.duty_on_duration = Duration::from_secs(3);
        config.duty_off_duration = Duration::from_secs(15);
        config.maintenance_interval = Duration::from_secs(10);
        config.announce_interval_secs = 8.0;
        config
    }
    
    /// Create configuration optimized for development (faster intervals)
    pub fn development() -> Self {
        let mut config = Self::canonical();
        config.duty_on_duration = Duration::from_secs(2);
        config.duty_off_duration = Duration::from_secs(5);
        config.maintenance_interval = Duration::from_secs(2);
        config.announce_interval_secs = 2.0;
        config
    }
    
    /// Create configuration optimized for testing (very fast intervals)
    pub fn testing() -> Self {
        let mut config = Self::canonical();
        config.duty_on_duration = Duration::from_millis(500);
        config.duty_off_duration = Duration::from_millis(1000);
        config.maintenance_interval = Duration::from_millis(500);
        config.connection_timeout = Duration::from_millis(1000);
        config.scan_timeout = Duration::from_millis(2000);
        config.announce_interval_secs = 1.0;
        config
    }
}

/// Nostr Transport configuration with canonical parameter compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrTransportConfig {
    // Core Nostr parameters
    pub relays: Vec<String>,
    pub connection_timeout_secs: u64,
    pub max_message_size: usize,
    pub verify_tls: bool,
    
    // Canonical timing parameters
    pub read_ack_interval: Duration,              // nostrReadAckInterval: 0.35s
    pub geohash_initial_lookback_secs: u64,       // nostrGeohashInitialLookbackSeconds: 3600s
    pub geohash_initial_limit: usize,             // nostrGeohashInitialLimit: 200
    pub geo_relay_count: usize,                   // nostrGeoRelayCount: 5
    pub geohash_sample_lookback_secs: u64,        // nostrGeohashSampleLookbackSeconds: 300s
    pub geohash_sample_limit: usize,              // nostrGeohashSampleLimit: 100
    
    // Canonical backoff and reconnection parameters
    pub relay_initial_backoff_secs: f64,          // nostrRelayInitialBackoffSeconds: 1.0s
    pub relay_max_backoff_secs: f64,              // nostrRelayMaxBackoffSeconds: 300.0s
    pub relay_backoff_multiplier: f64,            // nostrRelayBackoffMultiplier: 2.0
    pub relay_max_reconnect_attempts: usize,      // nostrRelayMaxReconnectAttempts: 10
}

impl Default for NostrTransportConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl NostrTransportConfig {
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            // Core parameters
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            connection_timeout_secs: 10,
            max_message_size: 65536,
            verify_tls: true,
            
            // Canonical timing parameters
            read_ack_interval: Duration::from_millis(350),
            geohash_initial_lookback_secs: 3600,
            geohash_initial_limit: 200,
            geo_relay_count: 5,
            geohash_sample_lookback_secs: 300,
            geohash_sample_limit: 100,
            
            // Canonical backoff parameters
            relay_initial_backoff_secs: 1.0,
            relay_max_backoff_secs: 300.0,
            relay_backoff_multiplier: 2.0,
            relay_max_reconnect_attempts: 10,
        }
    }
    
    /// Create configuration optimized for testing (faster timeouts)
    pub fn testing() -> Self {
        let mut config = Self::canonical();
        config.connection_timeout_secs = 1;
        config.read_ack_interval = Duration::from_millis(50);
        config.relay_initial_backoff_secs = 0.1;
        config.relay_max_backoff_secs = 5.0;
        config.relay_max_reconnect_attempts = 3;
        config
    }
    
    /// Create configuration optimized for development
    pub fn development() -> Self {
        let mut config = Self::canonical();
        config.connection_timeout_secs = 5;
        config.relay_initial_backoff_secs = 0.5;
        config.relay_max_backoff_secs = 30.0;
        config
    }
}

/// System limits configuration with canonical parameter compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub private_chat_cap: usize,                  // privateChatCap: 1337
    pub mesh_timeline_cap: usize,                 // meshTimelineCap: 1337
    pub geo_timeline_cap: usize,                  // geoTimelineCap: 1337
    pub content_lru_cap: usize,                   // contentLRUCap: 2000
    pub max_nickname_length: usize,               // maxNicknameLength: 50
    pub max_message_length: usize,                // maxMessageLength: 60000
    pub message_ttl_default: u8,                  // messageTTLDefault: 7
    pub processed_nostr_events_cap: usize,        // uiProcessedNostrEventsCap: 2000
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl LimitsConfig {
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            private_chat_cap: 1337,
            mesh_timeline_cap: 1337,
            geo_timeline_cap: 1337,
            content_lru_cap: 2000,
            max_nickname_length: 50,
            max_message_length: 60_000,
            message_ttl_default: 7,
            processed_nostr_events_cap: 2000,
        }
    }
    
    /// Create configuration for low-memory environments
    pub fn low_memory() -> Self {
        Self {
            private_chat_cap: 500,
            mesh_timeline_cap: 500,
            geo_timeline_cap: 500,
            content_lru_cap: 1000,
            max_nickname_length: 30,
            max_message_length: 10_000,
            message_ttl_default: 5,
            processed_nostr_events_cap: 500,
        }
    }
    
    /// Create configuration for testing (smaller limits)
    pub fn testing() -> Self {
        Self {
            private_chat_cap: 100,
            mesh_timeline_cap: 100,
            geo_timeline_cap: 100,
            content_lru_cap: 200,
            max_nickname_length: 20,
            max_message_length: 1000,
            message_ttl_default: 3,
            processed_nostr_events_cap: 100,
        }
    }
}

/// Timing configuration with canonical parameter compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    pub network_reset_grace_secs: u64,            // networkResetGraceSeconds: 600s
    pub base_public_flush_interval: Duration,     // basePublicFlushInterval: 0.08s
    pub channel_inactivity_threshold_secs: u64,   // uiChannelInactivityThresholdSeconds: 540s
    pub late_insert_threshold_secs: f64,          // uiLateInsertThreshold: 15.0s
    pub late_insert_threshold_geo_secs: f64,      // uiLateInsertThresholdGeo: 0.0s
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl TimingConfig {
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            network_reset_grace_secs: 600,
            base_public_flush_interval: Duration::from_millis(80),
            channel_inactivity_threshold_secs: 540, // 9 minutes
            late_insert_threshold_secs: 15.0,
            late_insert_threshold_geo_secs: 0.0,
        }
    }
    
    /// Create configuration optimized for testing (faster timeouts)
    pub fn testing() -> Self {
        Self {
            network_reset_grace_secs: 10,
            base_public_flush_interval: Duration::from_millis(10),
            channel_inactivity_threshold_secs: 30,
            late_insert_threshold_secs: 1.0,
            late_insert_threshold_geo_secs: 0.0,
        }
    }
}

/// UI-related configuration with canonical parameter compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    // Rate limiting parameters
    pub sender_rate_bucket_capacity: f64,         // uiSenderRateBucketCapacity: 5.0
    pub sender_rate_bucket_refill_per_sec: f64,   // uiSenderRateBucketRefillPerSec: 1.0
    pub content_rate_bucket_capacity: f64,        // uiContentRateBucketCapacity: 3.0
    pub content_rate_bucket_refill_per_sec: f64,  // uiContentRateBucketRefillPerSec: 0.5
    
    // Timing parameters
    pub scroll_throttle_secs: f64,                // uiScrollThrottleSeconds: 0.5s
    pub geo_notify_cooldown_secs: f64,            // uiGeoNotifyCooldownSeconds: 60.0s
    pub geo_notify_snippet_max_len: usize,        // uiGeoNotifySnippetMaxLen: 80
    
    // Message display parameters
    pub long_message_length_threshold: usize,     // uiLongMessageLengthThreshold: 2000
    pub very_long_token_threshold: usize,         // uiVeryLongTokenThreshold: 512
    pub long_message_line_limit: usize,           // uiLongMessageLineLimit: 30
    pub content_key_prefix_length: usize,         // contentKeyPrefixLength: 256
    pub fingerprint_sample_count: usize,          // uiFingerprintSampleCount: 3
    
    // Animation timing
    pub animation_short_secs: f64,                // uiAnimationShortSeconds: 0.15
    pub animation_medium_secs: f64,               // uiAnimationMediumSeconds: 0.2
    pub animation_sidebar_secs: f64,              // uiAnimationSidebarSeconds: 0.25
    
    // Gesture thresholds
    pub back_swipe_translation_large: f64,        // uiBackSwipeTranslationLarge: 50
    pub back_swipe_translation_small: f64,        // uiBackSwipeTranslationSmall: 30
    pub back_swipe_velocity_threshold: f64,       // uiBackSwipeVelocityThreshold: 300
}

impl Default for UiConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl UiConfig {
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            // Rate limiting
            sender_rate_bucket_capacity: 5.0,
            sender_rate_bucket_refill_per_sec: 1.0,
            content_rate_bucket_capacity: 3.0,
            content_rate_bucket_refill_per_sec: 0.5,
            
            // Timing
            scroll_throttle_secs: 0.5,
            geo_notify_cooldown_secs: 60.0,
            geo_notify_snippet_max_len: 80,
            
            // Message display
            long_message_length_threshold: 2000,
            very_long_token_threshold: 512,
            long_message_line_limit: 30,
            content_key_prefix_length: 256,
            fingerprint_sample_count: 3,
            
            // Animation timing
            animation_short_secs: 0.15,
            animation_medium_secs: 0.2,
            animation_sidebar_secs: 0.25,
            
            // Gesture thresholds
            back_swipe_translation_large: 50.0,
            back_swipe_translation_small: 30.0,
            back_swipe_velocity_threshold: 300.0,
        }
    }
    
    /// Create configuration optimized for testing (faster animations)
    pub fn testing() -> Self {
        let mut config = Self::canonical();
        config.scroll_throttle_secs = 0.1;
        config.geo_notify_cooldown_secs = 1.0;
        config.animation_short_secs = 0.01;
        config.animation_medium_secs = 0.02;
        config.animation_sidebar_secs = 0.03;
        config
    }
}

/// Configuration presets for different deployment scenarios
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigPreset {
    /// Canonical values matching Swift implementation exactly
    Canonical,
    /// Optimized for development with faster intervals
    Development,
    /// Optimized for production use
    Production,
    /// Optimized for battery life on mobile devices
    BatteryOptimized,
    /// Optimized for testing with short timeouts
    Testing,
}

impl ConfigPreset {
    /// Apply preset to BLE transport configuration
    pub fn apply_to_ble_config(&self, config: &mut BleTransportConfig) {
        match self {
            ConfigPreset::Canonical => {
                *config = BleTransportConfig::canonical();
            }
            ConfigPreset::Development => {
                *config = BleTransportConfig::development();
            }
            ConfigPreset::Production => {
                *config = BleTransportConfig::canonical(); // Production uses canonical values
            }
            ConfigPreset::BatteryOptimized => {
                *config = BleTransportConfig::battery_optimized();
            }
            ConfigPreset::Testing => {
                *config = BleTransportConfig::testing();
            }
        }
    }
    
    /// Apply preset to Nostr transport configuration
    pub fn apply_to_nostr_config(&self, config: &mut NostrTransportConfig) {
        match self {
            ConfigPreset::Canonical | ConfigPreset::Production => {
                *config = NostrTransportConfig::canonical();
            }
            ConfigPreset::Development => {
                *config = NostrTransportConfig::development();
            }
            ConfigPreset::BatteryOptimized => {
                *config = NostrTransportConfig::canonical(); // No specific battery optimization for Nostr
            }
            ConfigPreset::Testing => {
                *config = NostrTransportConfig::testing();
            }
        }
    }
    
    /// Apply preset to limits configuration
    pub fn apply_to_limits_config(&self, config: &mut LimitsConfig) {
        match self {
            ConfigPreset::Canonical | ConfigPreset::Development | ConfigPreset::Production => {
                *config = LimitsConfig::canonical();
            }
            ConfigPreset::BatteryOptimized => {
                *config = LimitsConfig::low_memory(); // Reduce memory usage for battery optimization
            }
            ConfigPreset::Testing => {
                *config = LimitsConfig::testing();
            }
        }
    }
    
    /// Apply preset to timing configuration
    pub fn apply_to_timing_config(&self, config: &mut TimingConfig) {
        match self {
            ConfigPreset::Canonical | ConfigPreset::Development | ConfigPreset::Production | ConfigPreset::BatteryOptimized => {
                *config = TimingConfig::canonical();
            }
            ConfigPreset::Testing => {
                *config = TimingConfig::testing();
            }
        }
    }
    
    /// Apply preset to UI configuration
    pub fn apply_to_ui_config(&self, config: &mut UiConfig) {
        match self {
            ConfigPreset::Canonical | ConfigPreset::Development | ConfigPreset::Production | ConfigPreset::BatteryOptimized => {
                *config = UiConfig::canonical();
            }
            ConfigPreset::Testing => {
                *config = UiConfig::testing();
            }
        }
    }
}

/// Configuration validator for canonical parameter ranges
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate canonical parameter ranges for BLE transport
    pub fn validate_ble_config(config: &BleTransportConfig) -> Result<(), alloc::string::String> {
        if config.fragment_size < 100 || config.fragment_size > 1024 {
            return Err("BLE fragment size must be between 100-1024 bytes".into());
        }
        
        if config.max_central_links > 10 {
            return Err("BLE max central links should not exceed 10".into());
        }
        
        if config.high_degree_threshold > 20 {
            return Err("BLE high degree threshold should not exceed 20".into());
        }
        
        if config.dynamic_rssi_threshold > -50 || config.dynamic_rssi_threshold < -120 {
            return Err("BLE RSSI threshold should be between -120 and -50 dBm".into());
        }
        
        Ok(())
    }
    
    /// Validate canonical parameter ranges for Nostr transport
    pub fn validate_nostr_config(config: &NostrTransportConfig) -> Result<(), alloc::string::String> {
        if config.geo_relay_count > 20 {
            return Err("Nostr geo relay count should not exceed 20".into());
        }
        
        if config.relay_max_reconnect_attempts > 50 {
            return Err("Nostr max reconnect attempts should not exceed 50".into());
        }
        
        if config.relay_backoff_multiplier < 1.1 || config.relay_backoff_multiplier > 5.0 {
            return Err("Nostr backoff multiplier should be between 1.1 and 5.0".into());
        }
        
        Ok(())
    }
    
    /// Validate canonical parameter ranges for limits
    pub fn validate_limits_config(config: &LimitsConfig) -> Result<(), alloc::string::String> {
        if config.message_ttl_default > 15 {
            return Err("Message TTL should not exceed 15 hops".into());
        }
        
        if config.max_message_length > 100_000 {
            return Err("Max message length should not exceed 100KB".into());
        }
        
        if config.max_nickname_length > 200 {
            return Err("Max nickname length should not exceed 200 characters".into());
        }
        
        Ok(())
    }
    
    /// Validate all canonical configurations
    pub fn validate_canonical_ranges(
        ble: &BleTransportConfig,
        nostr: &NostrTransportConfig,
        limits: &LimitsConfig,
    ) -> Result<(), alloc::string::String> {
        Self::validate_ble_config(ble)?;
        Self::validate_nostr_config(nostr)?;
        Self::validate_limits_config(limits)?;
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Master Configuration
// ----------------------------------------------------------------------------

/// Master configuration struct that consolidates all BitChat configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    
    // Canonical transport configurations
    /// BLE transport configuration with canonical compatibility
    pub ble: BleTransportConfig,
    /// Nostr transport configuration with canonical compatibility
    pub nostr: NostrTransportConfig,
    /// System limits configuration with canonical compatibility
    pub limits: LimitsConfig,
    /// Timing configuration with canonical compatibility
    pub timing: TimingConfig,
    /// UI configuration with canonical compatibility
    pub ui: UiConfig,
}

impl Default for BitchatConfig {
    fn default() -> Self {
        Self::canonical()
    }
}

impl BitchatConfig {
    /// Create new configuration with canonical default values
    pub fn new() -> Self {
        Self::canonical()
    }
    
    /// Create configuration with canonical default values from Swift implementation
    pub fn canonical() -> Self {
        Self {
            channels: ChannelConfig::default(),
            delivery: DeliveryConfig::default(),
            message_store: MessageStoreConfig::default(),
            session: SessionConfig::default(),
            monitoring: MonitoringConfig::default(),
            rate_limiting: RateLimitConfig::default(),
            test: None,
            ble: BleTransportConfig::canonical(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::canonical(),
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
        }
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
            ble: BleTransportConfig::canonical(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::low_memory(), // Reduce memory usage for browser
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
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
            ble: BleTransportConfig::canonical(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::canonical(),
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
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
            ble: BleTransportConfig::battery_optimized(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::low_memory(),
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
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
            ble: BleTransportConfig::testing(),
            nostr: NostrTransportConfig::testing(),
            limits: LimitsConfig::testing(),
            timing: TimingConfig::testing(),
            ui: UiConfig::testing(),
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
            ble: BleTransportConfig::canonical(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::canonical(),
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
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
    
    /// Builder method for customizing BLE transport configuration
    pub fn with_ble(mut self, ble: BleTransportConfig) -> Self {
        self.ble = ble;
        self
    }
    
    /// Builder method for customizing Nostr transport configuration
    pub fn with_nostr(mut self, nostr: NostrTransportConfig) -> Self {
        self.nostr = nostr;
        self
    }
    
    /// Builder method for customizing limits configuration
    pub fn with_limits(mut self, limits: LimitsConfig) -> Self {
        self.limits = limits;
        self
    }
    
    /// Builder method for customizing timing configuration
    pub fn with_timing(mut self, timing: TimingConfig) -> Self {
        self.timing = timing;
        self
    }
    
    /// Builder method for customizing UI configuration
    pub fn with_ui(mut self, ui: UiConfig) -> Self {
        self.ui = ui;
        self
    }
    
    /// Apply a configuration preset
    pub fn with_preset(mut self, preset: ConfigPreset) -> Self {
        preset.apply_to_ble_config(&mut self.ble);
        preset.apply_to_nostr_config(&mut self.nostr);
        preset.apply_to_limits_config(&mut self.limits);
        preset.apply_to_timing_config(&mut self.timing);
        preset.apply_to_ui_config(&mut self.ui);
        self
    }
    
    /// Create configuration with a specific preset applied
    pub fn from_preset(preset: ConfigPreset) -> Self {
        Self::canonical().with_preset(preset)
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
        
        // Validate canonical transport configurations
        ConfigValidator::validate_canonical_ranges(&self.ble, &self.nostr, &self.limits)?;

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
        BitchatConfig::from_preset(ConfigPreset::Development)
            .with_monitoring(MonitoringConfig::detailed())
            .with_rate_limiting(RateLimitConfig::permissive())
            .with_test(TestConfig::new().with_logging())
    }

    /// Production configuration (optimized for production use)
    pub fn production() -> BitchatConfig {
        BitchatConfig::from_preset(ConfigPreset::Production)
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
            ble: BleTransportConfig::battery_optimized(),
            nostr: NostrTransportConfig::canonical(),
            limits: LimitsConfig::low_memory(),
            timing: TimingConfig::canonical(),
            ui: UiConfig::canonical(),
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
    ble_config: Option<BleTransportConfig>,
    nostr_config: Option<NostrTransportConfig>,
    limits_config: Option<LimitsConfig>,
    timing_config: Option<TimingConfig>,
    ui_config: Option<UiConfig>,
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
            ble_config: None,
            nostr_config: None,
            limits_config: None,
            timing_config: None,
            ui_config: None,
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
    
    /// Set BLE transport configuration
    pub fn ble(mut self, config: BleTransportConfig) -> Self {
        self.ble_config = Some(config);
        self
    }
    
    /// Set Nostr transport configuration
    pub fn nostr(mut self, config: NostrTransportConfig) -> Self {
        self.nostr_config = Some(config);
        self
    }
    
    /// Set limits configuration
    pub fn limits(mut self, config: LimitsConfig) -> Self {
        self.limits_config = Some(config);
        self
    }
    
    /// Set timing configuration
    pub fn timing(mut self, config: TimingConfig) -> Self {
        self.timing_config = Some(config);
        self
    }
    
    /// Set UI configuration
    pub fn ui(mut self, config: UiConfig) -> Self {
        self.ui_config = Some(config);
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
            ble_config: Some(BleTransportConfig::canonical()),
            nostr_config: Some(NostrTransportConfig::canonical()),
            limits_config: Some(LimitsConfig::low_memory()),
            timing_config: Some(TimingConfig::canonical()),
            ui_config: Some(UiConfig::canonical()),
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
            ble_config: Some(BleTransportConfig::canonical()),
            nostr_config: Some(NostrTransportConfig::canonical()),
            limits_config: Some(LimitsConfig::canonical()),
            timing_config: Some(TimingConfig::canonical()),
            ui_config: Some(UiConfig::canonical()),
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
            ble_config: Some(BleTransportConfig::battery_optimized()),
            nostr_config: Some(NostrTransportConfig::canonical()),
            limits_config: Some(LimitsConfig::low_memory()),
            timing_config: Some(TimingConfig::canonical()),
            ui_config: Some(UiConfig::canonical()),
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
            ble_config: Some(BleTransportConfig::canonical()),
            nostr_config: Some(NostrTransportConfig::canonical()),
            limits_config: Some(LimitsConfig::canonical()),
            timing_config: Some(TimingConfig::canonical()),
            ui_config: Some(UiConfig::canonical()),
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
            ble_config: Some(BleTransportConfig::testing()),
            nostr_config: Some(NostrTransportConfig::testing()),
            limits_config: Some(LimitsConfig::testing()),
            timing_config: Some(TimingConfig::testing()),
            ui_config: Some(UiConfig::testing()),
        }
    }
    
    /// Use canonical preset as base (exact Swift compatibility)
    pub fn canonical(self) -> Self {
        Self {
            channels: Some(ChannelConfig::default()),
            delivery: Some(DeliveryConfig::default()),
            message_store: Some(MessageStoreConfig::default()),
            session: Some(SessionConfig::default()),
            monitoring: Some(MonitoringConfig::default()),
            rate_limiting: Some(RateLimitConfig::default()),
            test: None,
            ble_config: Some(BleTransportConfig::canonical()),
            nostr_config: Some(NostrTransportConfig::canonical()),
            limits_config: Some(LimitsConfig::canonical()),
            timing_config: Some(TimingConfig::canonical()),
            ui_config: Some(UiConfig::canonical()),
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
            ble: self.ble_config.unwrap_or_default(),
            nostr: self.nostr_config.unwrap_or_default(),
            limits: self.limits_config.unwrap_or_default(),
            timing: self.timing_config.unwrap_or_default(),
            ui: self.ui_config.unwrap_or_default(),
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
            ble: self.ble_config.unwrap_or_default(),
            nostr: self.nostr_config.unwrap_or_default(),
            limits: self.limits_config.unwrap_or_default(),
            timing: self.timing_config.unwrap_or_default(),
            ui: self.ui_config.unwrap_or_default(),
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
    
    #[test]
    fn test_canonical_configuration() {
        let config = BitchatConfig::canonical();
        assert!(config.validate().is_ok());
        
        // Verify canonical BLE parameters
        assert_eq!(config.ble.fragment_size, 469);
        assert_eq!(config.ble.max_central_links, 6);
        assert_eq!(config.ble.duty_on_duration, Duration::from_secs(5));
        assert_eq!(config.ble.duty_off_duration, Duration::from_secs(10));
        
        // Verify canonical Nostr parameters
        assert_eq!(config.nostr.read_ack_interval, Duration::from_millis(350));
        assert_eq!(config.nostr.geohash_initial_lookback_secs, 3600);
        assert_eq!(config.nostr.geo_relay_count, 5);
        
        // Verify canonical limits
        assert_eq!(config.limits.private_chat_cap, 1337);
        assert_eq!(config.limits.message_ttl_default, 7);
        assert_eq!(config.limits.max_message_length, 60_000);
    }
    
    #[test]
    fn test_config_presets() {
        // Test development preset
        let dev_config = BitchatConfig::from_preset(ConfigPreset::Development);
        assert!(dev_config.validate().is_ok());
        assert_eq!(dev_config.ble.duty_on_duration, Duration::from_secs(2));
        
        // Test battery optimized preset
        let battery_config = BitchatConfig::from_preset(ConfigPreset::BatteryOptimized);
        assert!(battery_config.validate().is_ok());
        assert_eq!(battery_config.ble.duty_on_duration, Duration::from_secs(3));
        assert_eq!(battery_config.ble.duty_off_duration, Duration::from_secs(15));
        
        // Test testing preset
        let test_config = BitchatConfig::from_preset(ConfigPreset::Testing);
        assert!(test_config.validate().is_ok());
        assert_eq!(test_config.ble.duty_on_duration, Duration::from_millis(500));
    }
    
    #[test]
    fn test_transport_config_validation() {
        // Test valid BLE config
        let ble_config = BleTransportConfig::canonical();
        assert!(ConfigValidator::validate_ble_config(&ble_config).is_ok());
        
        // Test invalid BLE config
        let mut invalid_ble = BleTransportConfig::canonical();
        invalid_ble.fragment_size = 50; // Too small
        assert!(ConfigValidator::validate_ble_config(&invalid_ble).is_err());
        
        // Test valid Nostr config
        let nostr_config = NostrTransportConfig::canonical();
        assert!(ConfigValidator::validate_nostr_config(&nostr_config).is_ok());
        
        // Test invalid Nostr config
        let mut invalid_nostr = NostrTransportConfig::canonical();
        invalid_nostr.geo_relay_count = 25; // Too many
        assert!(ConfigValidator::validate_nostr_config(&invalid_nostr).is_err());
    }
    
    #[test]
    fn test_builder_with_canonical_params() {
        let config = BitchatConfig::builder()
            .canonical()
            .ble(BleTransportConfig::development())
            .nostr(NostrTransportConfig::testing())
            .build()
            .expect("Canonical config with custom transport should be valid");
        
        assert!(config.validate().is_ok());
        assert_eq!(config.ble.duty_on_duration, Duration::from_secs(2)); // Development preset
        assert_eq!(config.nostr.connection_timeout_secs, 1); // Testing preset
    }
    
    #[test]
    fn test_memory_optimized_configs() {
        let low_memory_config = BitchatConfig::mobile_optimized();
        assert!(low_memory_config.validate().is_ok());
        assert_eq!(low_memory_config.limits.private_chat_cap, 500); // Reduced for low memory
        
        let browser_config = BitchatConfig::browser_optimized();
        assert!(browser_config.validate().is_ok());
        assert_eq!(browser_config.limits.private_chat_cap, 500); // Reduced for browser memory constraints
    }
}
