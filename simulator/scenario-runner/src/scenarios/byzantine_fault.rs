//! Byzantine fault tolerance scenario
//! 
//! Tests the system's resistance to malicious peer behavior

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run byzantine fault tolerance test
pub async fn run_byzantine_fault(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting byzantine fault tolerance test with {} clients...", client_type.name());

    // Start honest clients and one malicious client
    orchestrator.start_client_by_type(client_type, "honest_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "honest_b".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "malicious".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    info!("All clients started (2 honest, 1 malicious)");
    
    // Wait for honest clients to discover each other
    orchestrator.wait_for_peer_event("honest_a", "PeerDiscovered", "honest_b").await?;
    info!("Honest clients discovered each other");

    // Test normal communication between honest clients
    orchestrator.send_command("honest_a", "/send Legitimate message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    info!("Normal communication established");
    
    // Simulate malicious behavior (inject corrupted packets)
    orchestrator.send_command("malicious", "/inject-corrupted-packets 5").await?;
    info!("Malicious client attempting to inject corrupted packets");

    // Verify that honest communication continues despite malicious behavior
    orchestrator.send_command("honest_a", "/send Post-attack message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    info!("Communication continues successfully despite malicious behavior");

    info!("Byzantine fault tolerance test completed successfully");
    Ok(())
}