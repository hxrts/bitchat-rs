//! Data-Driven Scenario Configuration System
//!
//! This module provides a YAML/TOML-based configuration system for defining
//! test scenarios, network conditions, and peer behaviors in a declarative way.

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use bitchat_core::PeerId;
use crate::network_router::{NetworkProfile, NetworkTopology};

// ----------------------------------------------------------------------------
// Core Scenario Configuration
// ----------------------------------------------------------------------------

/// Complete scenario configuration loaded from YAML/TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    /// Scenario metadata
    pub metadata: ScenarioMetadata,
    /// Network configuration
    pub network: NetworkConfig,
    /// Peer definitions
    pub peers: Vec<PeerConfig>,
    /// Test sequence definition
    pub sequence: Vec<TestStep>,
    /// Validation rules
    pub validation: ValidationConfig,
    /// Performance expectations
    #[serde(default)]
    pub performance: PerformanceConfig,
}

/// Scenario metadata and description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    /// Scenario name
    pub name: String,
    /// Brief description
    pub description: String,
    /// Version for scenario evolution
    #[serde(default = "default_version")]
    pub version: String,
    /// Test categories/tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Expected duration
    #[serde(default)]
    pub duration_seconds: Option<u32>,
    /// Author information
    #[serde(default)]
    pub author: Option<String>,
}

fn default_version() -> String {
    "1.0".to_string()
}

// ----------------------------------------------------------------------------
// Network Configuration
// ----------------------------------------------------------------------------

/// Network-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Base network profile
    pub profile: NetworkProfileConfig,
    /// Network topology
    #[serde(default)]
    pub topology: NetworkTopologyConfig,
    /// Dynamic network changes during test
    #[serde(default)]
    pub changes: Vec<NetworkChange>,
    /// Logging configuration
    #[serde(default)]
    pub logging: NetworkLoggingConfig,
}

/// Serializable network profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkProfileConfig {
    Perfect,
    SlowWifi {
        latency_ms: u32,
        jitter_ms: u32,
    },
    Unreliable3G {
        packet_loss: f32,
        reordering_chance: f32,
        latency_ms: u32,
        jitter_ms: u32,
    },
    Satellite {
        latency_ms: u32,
        jitter_ms: u32,
        occasional_outages: bool,
    },
    MeshNetwork {
        hop_latency_ms: u32,
        max_hops: u8,
        partition_chance: f32,
    },
    Custom {
        latency_range_ms: (u32, u32),
        packet_loss: f32,
        reordering_chance: f32,
        duplication_chance: f32,
        corruption_chance: f32,
    },
}

impl From<NetworkProfileConfig> for NetworkProfile {
    fn from(config: NetworkProfileConfig) -> Self {
        match config {
            NetworkProfileConfig::Perfect => NetworkProfile::Perfect,
            NetworkProfileConfig::SlowWifi { latency_ms, jitter_ms } => 
                NetworkProfile::SlowWifi { latency_ms, jitter_ms },
            NetworkProfileConfig::Unreliable3G { packet_loss, reordering_chance, latency_ms, jitter_ms } => 
                NetworkProfile::Unreliable3G { packet_loss, reordering_chance, latency_ms, jitter_ms },
            NetworkProfileConfig::Satellite { latency_ms, jitter_ms, occasional_outages } => 
                NetworkProfile::Satellite { latency_ms, jitter_ms, occasional_outages },
            NetworkProfileConfig::MeshNetwork { hop_latency_ms, max_hops, partition_chance } => 
                NetworkProfile::MeshNetwork { hop_latency_ms, max_hops, partition_chance },
            NetworkProfileConfig::Custom { latency_range_ms, packet_loss, reordering_chance, duplication_chance, corruption_chance } => 
                NetworkProfile::Custom { latency_range_ms, packet_loss, reordering_chance, duplication_chance, corruption_chance },
        }
    }
}

/// Network topology configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkTopologyConfig {
    FullyConnected,
    Linear,
    Star { center: String }, // Peer name
    Custom { adjacency: HashMap<String, Vec<String>> }, // Peer names
}

impl NetworkTopologyConfig {
    pub fn to_network_topology(&self, peer_map: &HashMap<String, PeerId>) -> NetworkTopology {
        match self {
            NetworkTopologyConfig::FullyConnected => NetworkTopology::FullyConnected,
            NetworkTopologyConfig::Linear => NetworkTopology::Linear,
            NetworkTopologyConfig::Star { center } => {
                if let Some(&center_id) = peer_map.get(center) {
                    NetworkTopology::Star { center: center_id }
                } else {
                    NetworkTopology::FullyConnected // Fallback
                }
            }
            NetworkTopologyConfig::Custom { adjacency } => {
                let mut peer_adjacency = HashMap::new();
                for (peer_name, neighbors) in adjacency {
                    if let Some(&peer_id) = peer_map.get(peer_name) {
                        let neighbor_ids: Vec<PeerId> = neighbors
                            .iter()
                            .filter_map(|name| peer_map.get(name).copied())
                            .collect();
                        peer_adjacency.insert(peer_id, neighbor_ids);
                    }
                }
                NetworkTopology::Custom { adjacency: peer_adjacency }
            }
        }
    }
}

impl Default for NetworkTopologyConfig {
    fn default() -> Self {
        NetworkTopologyConfig::FullyConnected
    }
}

/// Dynamic network changes during scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkChange {
    /// When to apply this change (seconds from scenario start)
    pub at_time_seconds: f64,
    /// Type of change
    #[serde(flatten)]
    pub change: NetworkChangeType,
}

/// Types of network changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum NetworkChangeType {
    ChangeProfile { 
        profile: NetworkProfileConfig 
    },
    PartitionPeers { 
        peer1: String, 
        peer2: String 
    },
    HealPartition { 
        peer1: String, 
        peer2: String 
    },
    DisconnectPeer { 
        peer: String 
    },
    ReconnectPeer { 
        peer: String 
    },
}

/// Network logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLoggingConfig {
    /// Enable detailed packet logging
    #[serde(default)]
    pub enable_packet_logging: bool,
    /// Enable network statistics logging
    #[serde(default = "default_true")]
    pub enable_stats_logging: bool,
    /// Statistics logging interval in seconds
    #[serde(default = "default_stats_interval")]
    pub stats_interval_seconds: u32,
}

fn default_true() -> bool { true }
fn default_stats_interval() -> u32 { 10 }

impl Default for NetworkLoggingConfig {
    fn default() -> Self {
        Self {
            enable_packet_logging: false,
            enable_stats_logging: true,
            stats_interval_seconds: 10,
        }
    }
}

// ----------------------------------------------------------------------------
// Peer Configuration
// ----------------------------------------------------------------------------

/// Individual peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Unique peer name for reference
    pub name: String,
    /// Peer ID (optional, will be generated if not provided)
    pub peer_id: Option<String>, // Hex string representation
    /// Peer behavior configuration
    #[serde(default)]
    pub behavior: PeerBehaviorConfig,
    /// When this peer should start (seconds from scenario start)
    #[serde(default)]
    pub start_delay_seconds: f64,
    /// When this peer should stop (seconds from scenario start, optional)
    pub stop_at_seconds: Option<f64>,
}

/// Peer behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerBehaviorConfig {
    /// Auto-discovery behavior
    #[serde(default)]
    pub auto_discovery: bool,
    /// Automatic message sending patterns
    #[serde(default)]
    pub auto_messaging: Vec<AutoMessagePattern>,
    /// Response behaviors
    #[serde(default)]
    pub responses: Vec<ResponseBehavior>,
    /// Connection preferences
    #[serde(default)]
    pub connection_behavior: ConnectionBehaviorConfig,
}

impl Default for PeerBehaviorConfig {
    fn default() -> Self {
        Self {
            auto_discovery: true,
            auto_messaging: Vec::new(),
            responses: Vec::new(),
            connection_behavior: ConnectionBehaviorConfig::default(),
        }
    }
}

/// Automatic message sending pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMessagePattern {
    /// Target peer name (or "broadcast" for all peers)
    pub target: String,
    /// Message content template
    pub content: String,
    /// Sending interval in seconds
    pub interval_seconds: f64,
    /// Number of messages to send (optional, unlimited if not specified)
    pub count: Option<u32>,
    /// When to start this pattern (seconds from scenario start)
    #[serde(default)]
    pub start_at_seconds: f64,
    /// When to stop this pattern (seconds from scenario start, optional)
    pub stop_at_seconds: Option<f64>,
}

/// Response behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseBehavior {
    /// Trigger condition
    pub trigger: TriggerCondition,
    /// Response action
    pub action: ResponseAction,
    /// Response delay in seconds
    #[serde(default)]
    pub delay_seconds: f64,
}

/// Trigger conditions for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerCondition {
    MessageReceived { 
        from: Option<String>, // Peer name
        content_contains: Option<String>,
    },
    PeerConnected { 
        peer: String 
    },
    PeerDisconnected { 
        peer: String 
    },
    TimeElapsed { 
        seconds: f64 
    },
}

/// Response actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseAction {
    SendMessage { 
        target: String, 
        content: String 
    },
    ConnectToPeer { 
        peer: String 
    },
    DisconnectFromPeer { 
        peer: String 
    },
    StartDiscovery,
    StopDiscovery,
    LogMessage { 
        message: String 
    },
}

/// Connection behavior preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionBehaviorConfig {
    /// Preferred transports in order of preference
    #[serde(default)]
    pub preferred_transports: Vec<String>,
    /// Auto-connect to discovered peers
    #[serde(default = "default_true")]
    pub auto_connect: bool,
    /// Maximum concurrent connections
    #[serde(default)]
    pub max_connections: Option<u32>,
    /// Connection retry behavior
    #[serde(default)]
    pub retry_behavior: RetryBehaviorConfig,
}

impl Default for ConnectionBehaviorConfig {
    fn default() -> Self {
        Self {
            preferred_transports: vec!["mock".to_string()],
            auto_connect: true,
            max_connections: None,
            retry_behavior: RetryBehaviorConfig::default(),
        }
    }
}

/// Retry behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryBehaviorConfig {
    /// Enable automatic retries
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_attempts: u32,
    /// Initial retry delay in seconds
    #[serde(default = "default_initial_delay")]
    pub initial_delay_seconds: f64,
    /// Backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Maximum retry delay in seconds
    #[serde(default = "default_max_delay")]
    pub max_delay_seconds: f64,
}

fn default_max_retries() -> u32 { 3 }
fn default_initial_delay() -> f64 { 1.0 }
fn default_backoff_multiplier() -> f64 { 2.0 }
fn default_max_delay() -> f64 { 60.0 }

impl Default for RetryBehaviorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            initial_delay_seconds: 1.0,
            backoff_multiplier: 2.0,
            max_delay_seconds: 60.0,
        }
    }
}

// ----------------------------------------------------------------------------
// Test Sequence Definition
// ----------------------------------------------------------------------------

/// A single step in the test sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// Step name for identification
    pub name: String,
    /// When to execute this step (seconds from scenario start)
    pub at_time_seconds: f64,
    /// Step action
    #[serde(flatten)]
    pub action: TestAction,
}

/// Test actions that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum TestAction {
    SendMessage { 
        from: String, 
        to: String, 
        content: String 
    },
    SendBroadcast { 
        from: String, 
        content: String 
    },
    ConnectPeers { 
        peer1: String, 
        peer2: String 
    },
    DisconnectPeers { 
        peer1: String, 
        peer2: String 
    },
    StartDiscovery { 
        peer: String 
    },
    StopDiscovery { 
        peer: String 
    },
    ChangeNetworkProfile { 
        profile: NetworkProfileConfig 
    },
    PartitionNetwork { 
        peer1: String, 
        peer2: String 
    },
    HealPartition { 
        peer1: String, 
        peer2: String 
    },
    WaitForEvent { 
        event_type: String, 
        timeout_seconds: f64 
    },
    ValidateState { 
        validation: StateValidation 
    },
    LogCheckpoint { 
        message: String 
    },
    PauseScenario { 
        duration_seconds: f64 
    },
}

/// State validation checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateValidation {
    /// What to validate
    #[serde(flatten)]
    pub check: ValidationCheck,
}

/// Types of validation checks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ValidationCheck {
    MessageDelivered { 
        from: String, 
        to: String, 
        content: String 
    },
    PeerConnected { 
        peer1: String, 
        peer2: String 
    },
    PeerCount { 
        peer: String, 
        expected_count: u32 
    },
    SessionEstablished { 
        peer1: String, 
        peer2: String 
    },
    MessageCount { 
        peer: String, 
        expected_min: Option<u32>,
        expected_max: Option<u32>,
    },
    NetworkStats { 
        max_packet_loss: Option<f32>,
        min_delivery_rate: Option<f32>,
        max_avg_latency_ms: Option<u32>,
    },
}

// ----------------------------------------------------------------------------
// Validation Configuration
// ----------------------------------------------------------------------------

/// Overall validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Validation rules to check at scenario end
    #[serde(default)]
    pub final_checks: Vec<StateValidation>,
    /// Continuous validation rules (checked periodically)
    #[serde(default)]
    pub continuous_checks: Vec<ContinuousValidation>,
    /// Validation timeout settings
    #[serde(default)]
    pub timeouts: ValidationTimeouts,
}

/// Continuous validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousValidation {
    /// Validation check to perform
    pub check: ValidationCheck,
    /// How often to check (seconds)
    pub interval_seconds: f64,
    /// When to start checking (seconds from scenario start)
    #[serde(default)]
    pub start_at_seconds: f64,
    /// When to stop checking (seconds from scenario start, optional)
    pub stop_at_seconds: Option<f64>,
}

/// Validation timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTimeouts {
    /// Default timeout for message delivery validation
    #[serde(default = "default_message_timeout")]
    pub message_delivery_seconds: f64,
    /// Default timeout for connection establishment
    #[serde(default = "default_connection_timeout")]
    pub connection_establishment_seconds: f64,
    /// Default timeout for peer discovery
    #[serde(default = "default_discovery_timeout")]
    pub peer_discovery_seconds: f64,
}

fn default_message_timeout() -> f64 { 30.0 }
fn default_connection_timeout() -> f64 { 15.0 }
fn default_discovery_timeout() -> f64 { 10.0 }

impl Default for ValidationTimeouts {
    fn default() -> Self {
        Self {
            message_delivery_seconds: 30.0,
            connection_establishment_seconds: 15.0,
            peer_discovery_seconds: 10.0,
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            final_checks: Vec::new(),
            continuous_checks: Vec::new(),
            timeouts: ValidationTimeouts::default(),
        }
    }
}

// ----------------------------------------------------------------------------
// Performance Configuration
// ----------------------------------------------------------------------------

/// Performance expectations and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Expected message throughput (messages per second)
    pub expected_throughput: Option<f64>,
    /// Maximum acceptable latency (milliseconds)
    pub max_latency_ms: Option<u32>,
    /// Maximum acceptable packet loss rate (0.0-1.0)
    pub max_packet_loss: Option<f32>,
    /// Memory usage limits
    #[serde(default)]
    pub memory_limits: MemoryLimits,
    /// CPU usage limits
    #[serde(default)]
    pub cpu_limits: CpuLimits,
}

/// Memory usage limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLimits {
    /// Maximum memory usage per peer (MB)
    pub max_per_peer_mb: Option<u32>,
    /// Maximum total memory usage (MB)
    pub max_total_mb: Option<u32>,
    /// Memory leak detection threshold (MB growth per minute)
    pub leak_detection_mb_per_minute: Option<f32>,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_per_peer_mb: None,
            max_total_mb: None,
            leak_detection_mb_per_minute: None,
        }
    }
}

/// CPU usage limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuLimits {
    /// Maximum CPU usage per peer (0.0-100.0)
    pub max_per_peer_percent: Option<f32>,
    /// Maximum total CPU usage (0.0-100.0)
    pub max_total_percent: Option<f32>,
}

impl Default for CpuLimits {
    fn default() -> Self {
        Self {
            max_per_peer_percent: None,
            max_total_percent: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            expected_throughput: None,
            max_latency_ms: None,
            max_packet_loss: None,
            memory_limits: MemoryLimits::default(),
            cpu_limits: CpuLimits::default(),
        }
    }
}

// ----------------------------------------------------------------------------
// Scenario Loading and Utilities
// ----------------------------------------------------------------------------

impl ScenarioConfig {
    /// Load scenario from file (TOML format)
    #[allow(dead_code)]
    pub fn from_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: ScenarioConfig = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load scenario from TOML file
    pub fn from_toml_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: ScenarioConfig = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save scenario to TOML file
    #[allow(dead_code)]
    pub fn to_toml_file(&self, path: &std::path::Path) -> Result<(), anyhow::Error> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate scenario configuration
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        // Check for duplicate peer names
        let mut peer_names = std::collections::HashSet::new();
        for peer in &self.peers {
            if !peer_names.insert(&peer.name) {
                return Err(anyhow::anyhow!("Duplicate peer name: {}", peer.name));
            }
        }

        // Validate peer references in test steps
        for step in &self.sequence {
            match &step.action {
                TestAction::SendMessage { from, to, .. } |
                TestAction::ConnectPeers { peer1: from, peer2: to } |
                TestAction::DisconnectPeers { peer1: from, peer2: to } => {
                    if !peer_names.contains(from) {
                        return Err(anyhow::anyhow!("Unknown peer '{}' in step '{}'", from, step.name));
                    }
                    if !peer_names.contains(to) {
                        return Err(anyhow::anyhow!("Unknown peer '{}' in step '{}'", to, step.name));
                    }
                }
                TestAction::SendBroadcast { from, .. } |
                TestAction::StartDiscovery { peer: from } |
                TestAction::StopDiscovery { peer: from } => {
                    if !peer_names.contains(from) {
                        return Err(anyhow::anyhow!("Unknown peer '{}' in step '{}'", from, step.name));
                    }
                }
                _ => {} // Other actions don't reference peers
            }
        }

        // Validate time ordering
        let mut last_time = 0.0;
        for step in &self.sequence {
            if step.at_time_seconds < last_time {
                return Err(anyhow::anyhow!(
                    "Step '{}' at time {} is out of order (previous step was at {})",
                    step.name, step.at_time_seconds, last_time
                ));
            }
            last_time = step.at_time_seconds;
        }

        Ok(())
    }

    /// Get all peer names
    #[allow(dead_code)]
    pub fn peer_names(&self) -> Vec<&str> {
        self.peers.iter().map(|p| p.name.as_str()).collect()
    }

    /// Create peer ID mapping
    pub fn create_peer_mapping(&self) -> HashMap<String, PeerId> {
        let mut mapping = HashMap::new();
        for (i, peer) in self.peers.iter().enumerate() {
            let peer_id = if let Some(ref id_str) = peer.peer_id {
                // Parse hex string to PeerId
                if let Ok(bytes) = hex::decode(id_str) {
                    if bytes.len() == 8 {
                        let mut array = [0u8; 8];
                        array.copy_from_slice(&bytes);
                        PeerId::new(array)
                    } else {
                        // Fallback to generated ID
                        PeerId::new([(i + 1) as u8, 0, 0, 0, 0, 0, 0, 0])
                    }
                } else {
                    // Fallback to generated ID
                    PeerId::new([(i + 1) as u8, 0, 0, 0, 0, 0, 0, 0])
                }
            } else {
                // Generate ID based on index
                PeerId::new([(i + 1) as u8, 0, 0, 0, 0, 0, 0, 0])
            };
            mapping.insert(peer.name.clone(), peer_id);
        }
        mapping
    }

    /// Get scenario duration (maximum time from all steps and peer lifetimes)
    pub fn get_duration(&self) -> Duration {
        let mut max_time: f64 = 0.0;
        
        // Check test steps
        for step in &self.sequence {
            max_time = max_time.max(step.at_time_seconds);
        }
        
        // Check peer lifetimes
        for peer in &self.peers {
            if let Some(stop_time) = peer.stop_at_seconds {
                max_time = max_time.max(stop_time);
            }
        }
        
        // Check network changes
        for change in &self.network.changes {
            max_time = max_time.max(change.at_time_seconds);
        }
        
        // Add buffer if we have a metadata duration
        if let Some(expected) = self.metadata.duration_seconds {
            max_time = max_time.max(expected as f64);
        }
        
        Duration::from_secs_f64(max_time + 5.0) // Add 5 second buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_validation() {
        let config = ScenarioConfig {
            metadata: ScenarioMetadata {
                name: "test".to_string(),
                description: "test".to_string(),
                version: "1.0".to_string(),
                tags: vec![],
                duration_seconds: None,
                author: None,
            },
            network: NetworkConfig {
                profile: NetworkProfileConfig::Perfect,
                topology: NetworkTopologyConfig::FullyConnected,
                changes: vec![],
                logging: NetworkLoggingConfig::default(),
            },
            peers: vec![
                PeerConfig {
                    name: "peer1".to_string(),
                    peer_id: None,
                    behavior: PeerBehaviorConfig::default(),
                    start_delay_seconds: 0.0,
                    stop_at_seconds: None,
                },
                PeerConfig {
                    name: "peer2".to_string(),
                    peer_id: None,
                    behavior: PeerBehaviorConfig::default(),
                    start_delay_seconds: 0.0,
                    stop_at_seconds: None,
                },
            ],
            sequence: vec![
                TestStep {
                    name: "send_message".to_string(),
                    at_time_seconds: 1.0,
                    action: TestAction::SendMessage {
                        from: "peer1".to_string(),
                        to: "peer2".to_string(),
                        content: "hello".to_string(),
                    },
                },
            ],
            validation: ValidationConfig::default(),
            performance: PerformanceConfig::default(),
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_peer_reference() {
        let config = ScenarioConfig {
            metadata: ScenarioMetadata {
                name: "test".to_string(),
                description: "test".to_string(),
                version: "1.0".to_string(),
                tags: vec![],
                duration_seconds: None,
                author: None,
            },
            network: NetworkConfig {
                profile: NetworkProfileConfig::Perfect,
                topology: NetworkTopologyConfig::FullyConnected,
                changes: vec![],
                logging: NetworkLoggingConfig::default(),
            },
            peers: vec![
                PeerConfig {
                    name: "peer1".to_string(),
                    peer_id: None,
                    behavior: PeerBehaviorConfig::default(),
                    start_delay_seconds: 0.0,
                    stop_at_seconds: None,
                },
            ],
            sequence: vec![
                TestStep {
                    name: "send_message".to_string(),
                    at_time_seconds: 1.0,
                    action: TestAction::SendMessage {
                        from: "peer1".to_string(),
                        to: "nonexistent".to_string(),
                        content: "hello".to_string(),
                    },
                },
            ],
            validation: ValidationConfig::default(),
            performance: PerformanceConfig::default(),
        };

        assert!(config.validate().is_err());
    }
}