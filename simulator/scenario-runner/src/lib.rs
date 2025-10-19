//! BitChat Scenario Runner
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
//! ```rust
//! use bitchat_scenario_runner::{ScenarioConfig, ScenarioRunner};
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

pub mod scenario_config;
pub mod scenario_runner;
pub mod network_router;
pub mod event_orchestrator;

// Re-export main types for convenience
pub use scenario_config::{
    ScenarioConfig, NetworkProfileConfig, PeerConfig, TestStep, TestAction,
    ValidationCheck, NetworkConfig, PerformanceConfig
};
pub use scenario_runner::ScenarioRunner;
pub use network_router::{
    NetworkRouter, NetworkRouterConfig, NetworkProfile, NetworkPacket,
    MockNetworkHandle, NetworkStats
};

/// Version of the scenario runner
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Load and run a scenario from a file
pub async fn run_scenario_file(
    path: &std::path::Path
) -> Result<(), anyhow::Error> {
    let config = ScenarioConfig::from_file(path)?;

    let mut runner = ScenarioRunner::new(config).await?;
    runner.initialize().await?;
    let _metrics = runner.run().await?;
    
    Ok(())
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

    #[test]
    fn test_scenario_validation() {
        let scenario = create_basic_scenario(
            "Valid Scenario",
            "A valid test scenario",
            vec!["peer1", "peer2"]
        );

        assert!(scenario.validate().is_ok());
    }
}