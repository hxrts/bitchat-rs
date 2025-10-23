//! Common types shared between scenario-runner and emulator-rig

use serde::{Serialize, Deserialize};
use std::time::Duration;

/// Abstract action that can be executed in a scenario
///
/// These actions are platform-agnostic and implementation-agnostic.
/// Different executors implement them differently:
/// - SimulationExecutor: Direct protocol calls
/// - RealWorldExecutor: UI automation or protocol messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action")]
pub enum Action {
    /// Send a message from one peer to another
    SendMessage {
        from: String,
        to: String,
        content: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Send a broadcast message to all peers
    SendBroadcast {
        from: String,
        content: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Connect two peers
    ConnectPeer {
        initiator: String,
        target: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Connect two peers (alias for compatibility)
    ConnectPeers {
        peer1: String,
        peer2: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Disconnect a peer
    DisconnectPeer {
        peer: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Disconnect two peers
    DisconnectPeers {
        peer1: String,
        peer2: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Start peer discovery
    StartDiscovery {
        peer: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Stop peer discovery
    StopDiscovery {
        peer: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Log a checkpoint message
    LogCheckpoint {
        message: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Pause scenario execution
    PauseScenario {
        duration_seconds: f64,
    },
    
    /// Validate state
    ValidateState {
        validation: ValidationCheck,
    },
    
    /// Wait for an event
    WaitForEvent {
        event_type: String,
        timeout_seconds: f64,
    },
    
    /// Wait for a specified duration
    WaitFor {
        duration_seconds: f64,
    },
    
    /// Set network condition (simulation only)
    SetNetworkCondition {
        condition: NetworkCondition,
    },
    
    /// Partition network (isolate some peers)
    PartitionNetwork {
        isolated_peers: Vec<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
    
    /// Heal network partition
    HealNetwork {
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        at_time_seconds: Option<f64>,
    },
}

impl Action {
    /// Get the scheduled time for this action (if any)
    pub fn at_time(&self) -> Option<Duration> {
        let seconds = match self {
            Action::SendMessage { at_time_seconds, .. } => at_time_seconds,
            Action::SendBroadcast { at_time_seconds, .. } => at_time_seconds,
            Action::ConnectPeer { at_time_seconds, .. } => at_time_seconds,
            Action::ConnectPeers { at_time_seconds, .. } => at_time_seconds,
            Action::DisconnectPeer { at_time_seconds, .. } => at_time_seconds,
            Action::DisconnectPeers { at_time_seconds, .. } => at_time_seconds,
            Action::StartDiscovery { at_time_seconds, .. } => at_time_seconds,
            Action::StopDiscovery { at_time_seconds, .. } => at_time_seconds,
            Action::LogCheckpoint { at_time_seconds, .. } => at_time_seconds,
            Action::PartitionNetwork { at_time_seconds, .. } => at_time_seconds,
            Action::HealNetwork { at_time_seconds, .. } => at_time_seconds,
            _ => &None,
        };
        
        seconds.map(|s| Duration::from_secs_f64(s))
    }
}

/// Network condition for simulation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkCondition {
    /// Latency in milliseconds
    #[serde(default)]
    pub latency_ms: u64,
    
    /// Packet loss probability (0.0 to 1.0)
    #[serde(default)]
    pub packet_loss: f64,
    
    /// Bandwidth limit in bytes per second (optional)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth_bps: Option<u64>,
    
    /// Jitter in milliseconds (optional)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jitter_ms: Option<u64>,
}

impl Default for NetworkCondition {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            packet_loss: 0.0,
            bandwidth_bps: None,
            jitter_ms: None,
        }
    }
}

/// Validation check to perform
///
/// These checks are also abstract and implementation-agnostic.
/// Different executors validate differently:
/// - SimulationExecutor: Can check internal state
/// - RealWorldExecutor: Can only check observable behavior
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ValidationCheck {
    /// Verify a message was delivered
    MessageDelivered {
        from: String,
        to: String,
        content: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Verify two peers are connected
    PeerConnected {
        peer1: String,
        peer2: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Verify a peer is disconnected
    PeerDisconnected {
        peer: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Verify a peer reached a specific state
    StateReached {
        peer: String,
        state: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Verify message count
    MessageCount {
        peer: String,
        expected_count: usize,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Verify peer count in mesh
    PeerCount {
        peer: String,
        expected_count: usize,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    
    /// Custom check (for extension)
    Custom {
        name: String,
        parameters: serde_json::Value,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
}

fn default_timeout() -> u64 {
    30
}

impl ValidationCheck {
    /// Get the timeout for this check
    pub fn timeout(&self) -> Duration {
        let seconds = match self {
            ValidationCheck::MessageDelivered { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::PeerConnected { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::PeerDisconnected { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::StateReached { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::MessageCount { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::PeerCount { timeout_seconds, .. } => timeout_seconds,
            ValidationCheck::Custom { timeout_seconds, .. } => timeout_seconds,
        };
        
        Duration::from_secs(*seconds)
    }
}

/// Peer configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerConfig {
    /// Peer name/identifier
    pub name: String,
    
    /// Client type (cli, ios, android, web, kotlin)
    #[serde(default = "default_client_type")]
    pub client_type: String,
    
    /// Whether auto-discovery is enabled
    #[serde(default = "default_true")]
    pub auto_discovery: bool,
    
    /// Custom configuration
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

fn default_client_type() -> String {
    "cli".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_action_at_time() {
        let action = Action::SendMessage {
            from: "alice".to_string(),
            to: "bob".to_string(),
            content: "hello".to_string(),
            at_time_seconds: Some(2.5),
        };
        
        assert_eq!(action.at_time(), Some(Duration::from_secs_f64(2.5)));
    }
    
    #[test]
    fn test_validation_check_timeout() {
        let check = ValidationCheck::MessageDelivered {
            from: "alice".to_string(),
            to: "bob".to_string(),
            content: "hello".to_string(),
            timeout_seconds: 60,
        };
        
        assert_eq!(check.timeout(), Duration::from_secs(60));
    }
    
    #[test]
    fn test_network_condition_default() {
        let condition = NetworkCondition::default();
        assert_eq!(condition.latency_ms, 0);
        assert_eq!(condition.packet_loss, 0.0);
        assert!(condition.bandwidth_bps.is_none());
    }
}

/// Alias for compatibility with scenario configuration
pub type TestAction = Action;

