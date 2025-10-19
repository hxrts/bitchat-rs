//! Deterministic Test Runner
//!
//! Centralized runner for all test scenarios using event-driven orchestration
//! instead of sleep()-based timing.

use anyhow::Result;
use tracing::info;

use crate::event_orchestrator::EventOrchestrator;

// Import all scenario implementations
use super::{
    transport_failover::TransportFailoverScenario,
    session_rekey::SessionRekeyScenario,
    byzantine_fault::ByzantineFaultScenario,
    // Add others as they are updated
};

pub struct DeterministicTestRunner;

impl DeterministicTestRunner {
    /// Run all test scenarios using event-driven orchestration
    pub async fn run_all_scenarios(relay_url: String) -> Result<()> {
        info!("Starting deterministic test runner with relay: {}", relay_url);
        
        let scenarios = vec![
            ("transport-failover", Self::run_transport_failover as fn(&mut EventOrchestrator) -> _),
            ("session-rekey", Self::run_session_rekey as fn(&mut EventOrchestrator) -> _),
            ("byzantine-fault", Self::run_byzantine_fault as fn(&mut EventOrchestrator) -> _),
            ("panic-recovery", Self::run_panic_recovery as fn(&mut EventOrchestrator) -> _),
            ("mesh-partition", Self::run_mesh_partition as fn(&mut EventOrchestrator) -> _),
            ("peer-scaling", Self::run_peer_scaling as fn(&mut EventOrchestrator) -> _),
            ("file-transfer-resume", Self::run_file_transfer_resume as fn(&mut EventOrchestrator) -> _),
            ("version-compatibility", Self::run_version_compatibility as fn(&mut EventOrchestrator) -> _),
        ];

        for (name, scenario_fn) in scenarios {
            info!("Running scenario: {}", name);
            
            let mut orchestrator = EventOrchestrator::new(relay_url.clone());
            
            match scenario_fn(&mut orchestrator).await {
                Ok(()) => {
                    info!("Scenario '{}' completed successfully", name);
                }
                Err(e) => {
                    eprintln!("Scenario '{}' failed: {}", name, e);
                    return Err(e);
                }
            }
            
            // Clean up all clients before next scenario
            orchestrator.stop_all_clients().await?;
            
            // Brief pause between scenarios for cleanup
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        info!("All scenarios completed successfully!");
        Ok(())
    }

    async fn run_transport_failover(orchestrator: &mut EventOrchestrator) -> Result<()> {
        TransportFailoverScenario::run(orchestrator).await
    }

    async fn run_session_rekey(orchestrator: &mut EventOrchestrator) -> Result<()> {
        SessionRekeyScenario::run(orchestrator).await
    }

    async fn run_byzantine_fault(orchestrator: &mut EventOrchestrator) -> Result<()> {
        ByzantineFaultScenario::run(orchestrator).await
    }

    async fn run_panic_recovery(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Phase 1: Starting clients for panic recovery test");
        orchestrator.start_rust_client("client_a".to_string()).await?;
        orchestrator.start_rust_client("client_b".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;
        orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
        
        orchestrator.send_command("client_a", "/send Baseline message").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        info!("Phase 2: Simulating panic and recovery");
        orchestrator.send_command("client_a", "/simulate-panic").await?;
        orchestrator.wait_for_event("client_a", "PanicRecovered").await?;

        orchestrator.send_command("client_a", "/send Recovery verification").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        Ok(())
    }

    async fn run_mesh_partition(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Phase 1: Starting mesh of 4 clients");
        orchestrator.start_rust_client("client_a".to_string()).await?;
        orchestrator.start_rust_client("client_b".to_string()).await?;
        orchestrator.start_rust_client("client_c".to_string()).await?;
        orchestrator.start_rust_client("client_d".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;

        // Wait for full mesh connectivity
        orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
        orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_c").await?;
        
        info!("Phase 2: Simulating network partition");
        orchestrator.send_command("client_a", "/partition client_c client_d").await?;
        orchestrator.wait_for_event("client_a", "NetworkPartitioned").await?;

        info!("Phase 3: Testing partition healing");
        orchestrator.send_command("client_a", "/heal-partition").await?;
        orchestrator.wait_for_event("client_a", "PartitionHealed").await?;

        Ok(())
    }

    async fn run_peer_scaling(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Testing peer scaling with event-driven synchronization");
        
        // Start initial client
        orchestrator.start_rust_client("client_base".to_string()).await?;
        orchestrator.wait_for_event("client_base", "Ready").await?;

        // Add peers incrementally with event synchronization
        for i in 1..=10 {
            let client_name = format!("client_{}", i);
            orchestrator.start_rust_client(client_name.clone()).await?;
            orchestrator.wait_for_event(&client_name, "Ready").await?;
            orchestrator.wait_for_peer_event("client_base", "PeerDiscovered", &client_name).await?;
        }

        // Test broadcast message delivery
        orchestrator.send_command("client_base", "/broadcast Scaling test message").await?;
        
        // Wait for all peers to receive the message
        for i in 1..=10 {
            let client_name = format!("client_{}", i);
            orchestrator.wait_for_event(&client_name, "MessageReceived").await?;
        }

        Ok(())
    }

    async fn run_file_transfer_resume(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Testing file transfer interruption and resume");
        
        orchestrator.start_rust_client("sender".to_string()).await?;
        orchestrator.start_rust_client("receiver".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;
        orchestrator.wait_for_peer_event("sender", "PeerDiscovered", "receiver").await?;

        info!("Starting file transfer");
        orchestrator.send_command("sender", "/send-file large_test_file.bin").await?;
        orchestrator.wait_for_event("receiver", "FileTransferStarted").await?;

        info!("Simulating connection interruption");
        orchestrator.send_command("sender", "/disconnect").await?;
        orchestrator.wait_for_event("sender", "ConnectionLost").await?;

        info!("Resuming transfer");
        orchestrator.send_command("sender", "/reconnect").await?;
        orchestrator.wait_for_event("sender", "ConnectionEstablished").await?;
        orchestrator.wait_for_event("receiver", "FileTransferCompleted").await?;

        Ok(())
    }

    async fn run_version_compatibility(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Testing protocol version compatibility");
        
        orchestrator.start_rust_client("client_v1".to_string()).await?;
        orchestrator.start_rust_client("client_v2".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;

        // Configure different protocol versions
        orchestrator.send_command("client_v1", "/set-protocol-version 1.0").await?;
        orchestrator.send_command("client_v2", "/set-protocol-version 2.0").await?;

        // Test version negotiation
        orchestrator.wait_for_peer_event("client_v1", "PeerDiscovered", "client_v2").await?;
        orchestrator.wait_for_event("client_v1", "VersionNegotiated").await?;
        
        // Test backward compatibility
        orchestrator.send_command("client_v1", "/send Version compatibility test").await?;
        orchestrator.wait_for_event("client_v2", "MessageReceived").await?;

        Ok(())
    }
}