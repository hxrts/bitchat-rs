//! BitChat Scenario Runner
//!
//! Unified testing framework that bridges scenario-runner and emulator-rig
//!
//! A comprehensive testing framework for BitChat protocols that supports:
//!
//! - **Data-driven scenario configuration** via YAML/TOML files
//! - **Realistic network simulation** with configurable conditions
//! - **Automated test orchestration** with timing and validation
//! - **Performance monitoring** and metrics collection
//!
//! # Features
//!
//! ## Data-Driven Scenarios
//!
//! Define test scenarios declaratively in YAML or TOML:
//!
//! ```yaml
//! metadata:
//!   name: "Basic Messaging Test"
//!   description: "Tests message delivery between peers"
//!
//! network:
//!   profile:
//!     type: Unreliable3G
//!     packet_loss: 0.1
//!     latency_ms: 200
//!
//! peers:
//!   - name: "alice"
//!     behavior:
//!       auto_discovery: true
//!   - name: "bob"
//!     behavior:
//!       auto_discovery: true
//!
//! sequence:
//!   - name: "send_message"
//!     at_time_seconds: 2.0
//!     action: SendMessage
//!     from: "alice"
//!     to: "bob"
//!     content: "Hello Bob!"
//!
//! validation:
//!   final_checks:
//!     - type: MessageDelivered
//!       from: "alice"
//!       to: "bob"
//!       content: "Hello Bob!"
//! ```
//!
//! ## Network Simulation
//!
//! The simulator acts as the network itself, routing packets between peers
//! with realistic conditions:
//!
//! - **Latency and jitter** modeling
//! - **Packet loss, duplication, corruption**
//! - **Network partitions and healing**
//! - **Dynamic condition changes**
//! - **Mesh topology simulation**
//!
//! ## Usage
//!
//! ```ignore
//! use scenario_runner::{ScenarioConfig, ScenarioRunner};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), anyhow::Error> {
//!     // Load scenario from file
//!     let config = ScenarioConfig::from_yaml_file("scenarios/basic_test.yaml")?;
//!     
//!     // Create and run scenario
//!     let mut runner = ScenarioRunner::new(config).await?;
//!     runner.initialize().await?;
//!     
//!     let metrics = runner.run().await?;
//!     println!("Scenario completed: {:?}", metrics);
//!     
//!     Ok(())
//! }
//! ```

// Core abstractions for deterministic simulation
pub mod clock;
pub mod random;

// Simulation executor (implements ScenarioExecutor trait)
pub mod simulation_executor;

// Re-export key types for public API
pub use clock::{SimulationClock, SystemClock, VirtualClock, SimulationInstant};
pub use random::{RandomSource, SystemRandom, SeededRandom};

// Scenario execution
pub mod scenario_config;
pub mod network_router;
pub mod network_analysis;
// Legacy orchestration modules removed in favor of unified TOML-based architecture

use std::sync::Arc;

// Re-export main types for convenience
pub use scenario_config::{
    ScenarioConfig, NetworkProfileConfig, PeerConfig, TestStep, TestAction,
    ValidationCheck, NetworkConfig, PerformanceConfig
};
pub use network_router::{
    NetworkRouter, NetworkRouterConfig, NetworkProfile, NetworkPacket,
    MockNetworkHandle, NetworkStats
};
pub use network_analysis::{
    NetworkAnalyzer, AnalyzerConfig, NetworkMetrics, CapturedPacket,
    ComplianceStatus, AnalysisReport, CaptureSummary
};
// Re-export from shared crate for backwards compatibility
pub use bitchat_simulator_shared::{
    UniversalClientType, UniversalClientBridge, UniversalClient, ClientResponse,
    ScenarioExecutor, TestReport, ValidationResult, PerformanceMetrics, ExecutorError,
    Action, NetworkCondition,
    // Note: PeerConfig and ValidationCheck come from scenario_config to avoid conflicts
};
// CrossFrameworkOrchestrator removed - functionality moved to unified TOML scenarios
pub use simulation_executor::SimulationExecutor;

/// Version of the scenario runner
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Load and run a scenario from a file using the new unified interface
pub async fn run_scenario_file(
    path: &std::path::Path
) -> Result<(), anyhow::Error> {
    let local_config = crate::scenario_config::ScenarioConfig::from_toml_file(path)?;
    let shared_config = convert_to_shared_config(&local_config);
    let mut executor = crate::simulation_executor::SimulationExecutor::new();
    let _report = executor.execute_scenario(&shared_config).await?;
    Ok(())
}

/// Convert local ScenarioConfig to shared ScenarioConfig
pub fn convert_to_shared_config(local: &crate::scenario_config::ScenarioConfig) -> bitchat_simulator_shared::ScenarioConfig {
    use bitchat_simulator_shared::{ScenarioConfig, ScenarioMetadata, SharedPeerConfig, TestStep, ValidationConfig, StateValidation};
    
    ScenarioConfig {
        metadata: ScenarioMetadata {
            name: local.metadata.name.clone(),
            description: local.metadata.description.clone(),
            version: local.metadata.version.clone(),
        },
        peers: local.peers.iter().map(|p| SharedPeerConfig {
            name: p.name.clone(),
            platform: None, // TODO: Extract from local config if available
            start_delay_seconds: p.start_delay_seconds,
        }).collect(),
        sequence: local.sequence.iter().map(|s| TestStep {
            name: s.name.clone(),
            at_time_seconds: s.at_time_seconds,
            action: convert_action(&s.action),
        }).collect(),
        validation: ValidationConfig {
            final_checks: local.validation.final_checks.iter().map(|v| StateValidation {
                check: convert_validation_check(&v.check),
            }).collect(),
        },
    }
}

/// Convert local TestAction to shared TestAction
fn convert_action(local: &crate::scenario_config::TestAction) -> bitchat_simulator_shared::TestAction {
    use bitchat_simulator_shared::TestAction;
    use crate::scenario_config::TestAction as LocalTestAction;
    
    match local {
        LocalTestAction::SendMessage { from, to, content } => TestAction::SendMessage {
            from: from.clone(),
            to: to.clone(),
            content: content.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::SendBroadcast { from, content } => TestAction::SendBroadcast {
            from: from.clone(),
            content: content.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::ConnectPeers { peer1, peer2 } => TestAction::ConnectPeers {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::DisconnectPeers { peer1, peer2 } => TestAction::DisconnectPeers {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::StartDiscovery { peer } => TestAction::StartDiscovery {
            peer: peer.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::StopDiscovery { peer } => TestAction::StopDiscovery {
            peer: peer.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::LogCheckpoint { message } => TestAction::LogCheckpoint {
            message: message.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::PauseScenario { duration_seconds } => TestAction::PauseScenario {
            duration_seconds: *duration_seconds,
        },
        LocalTestAction::ValidateState { validation } => TestAction::ValidateState {
            validation: convert_validation_check(&validation.check),
        },
        _ => {
            // For unhandled actions, convert to a LogCheckpoint
            TestAction::LogCheckpoint {
                message: format!("Unhandled action: {:?}", local),
                at_time_seconds: None,
            }
        }
    }
}

/// Convert local ValidationCheck to shared ValidationCheck  
fn convert_validation_check(local: &crate::scenario_config::ValidationCheck) -> bitchat_simulator_shared::ValidationCheck {
    use bitchat_simulator_shared::ValidationCheck;
    use crate::scenario_config::ValidationCheck as LocalValidationCheck;
    
    match local {
        LocalValidationCheck::MessageDelivered { from, to, content } => ValidationCheck::MessageDelivered {
            from: from.clone(),
            to: to.clone(),
            content: content.clone(),
            timeout_seconds: 30, // Default timeout
        },
        LocalValidationCheck::PeerConnected { peer1, peer2 } => ValidationCheck::PeerConnected {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            timeout_seconds: 30,
        },
        LocalValidationCheck::MessageCount { peer, expected_min, expected_max: _ } => ValidationCheck::MessageCount {
            peer: peer.clone(),
            expected_count: expected_min.unwrap_or(0) as usize,
            timeout_seconds: 30,
        },
        _ => ValidationCheck::Custom {
            name: format!("Unhandled validation: {:?}", local),
            parameters: serde_json::Value::Null,
            timeout_seconds: 30,
        }
    }
}

/// Create a simple scenario programmatically
pub fn create_basic_scenario(
    name: &str,
    description: &str,
    peer_names: Vec<&str>,
) -> ScenarioConfig {
    use scenario_config::*;

    let peers = peer_names.into_iter().map(|name| PeerConfig {
        name: name.to_string(),
        peer_id: None,
        behavior: PeerBehaviorConfig::default(),
        start_delay_seconds: 0.0,
        stop_at_seconds: None,
    }).collect();

    ScenarioConfig {
        metadata: ScenarioMetadata {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0".to_string(),
            tags: vec!["basic".to_string()],
            duration_seconds: Some(30),
            author: Some("BitChat Scenario Runner".to_string()),
        },
        network: NetworkConfig {
            profile: NetworkProfileConfig::Perfect,
            topology: NetworkTopologyConfig::FullyConnected,
            changes: vec![],
            logging: NetworkLoggingConfig::default(),
        },
        peers,
        sequence: vec![],
        validation: ValidationConfig::default(),
        performance: PerformanceConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_basic_scenario() {
        let scenario = create_basic_scenario(
            "Test Scenario",
            "A basic test scenario",
            vec!["alice", "bob"]
        );

        assert_eq!(scenario.metadata.name, "Test Scenario");
        assert_eq!(scenario.peers.len(), 2);
        assert_eq!(scenario.peers[0].name, "alice");
        assert_eq!(scenario.peers[1].name, "bob");
    }

    #[tokio::test]
    async fn test_scenario_validation() {
        let local_scenario = create_basic_scenario(
            "Valid Scenario",
            "A valid test scenario",
            vec!["peer1", "peer2"]
        );

        assert!(local_scenario.validate().is_ok());
        
        // Test the new unified interface
        let shared_scenario = convert_to_shared_config(&local_scenario);
        let mut executor = crate::simulation_executor::SimulationExecutor::new();
        let result = executor.execute_scenario(&shared_scenario).await;
        assert!(result.is_ok());
    }
}