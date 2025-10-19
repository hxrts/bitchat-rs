//! All client types compatibility test scenario
//! 
//! Tests compatibility between all available client implementations

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run all client types compatibility test
pub async fn run_all_client_types_test(orchestrator: &mut EventOrchestrator) -> Result<()> {
    info!("Starting all client types compatibility test");

    // Start clients of each type
    orchestrator.start_client_by_type(ClientType::RustCli, "cli_peer".to_string()).await?;
    orchestrator.start_client_by_type(ClientType::Wasm, "wasm_peer".to_string()).await?;

    // Note: Swift and Kotlin clients would be started here if their implementations
    // support automation mode. For now, we focus on CLI and WASM.
    // orchestrator.start_client_by_type(ClientType::Swift, "swift_peer".to_string()).await?;
    // orchestrator.start_client_by_type(ClientType::Kotlin, "kotlin_peer".to_string()).await?;

    // Wait for all clients to be ready
    orchestrator.wait_for_all_ready().await?;
    info!("All clients are ready");

    // Get clients by type for verification
    let clients_by_type = orchestrator.get_clients_by_type();
    info!("Active client types: {:?}", clients_by_type);

    // Verify we have the expected client types
    assert!(clients_by_type.contains_key(&ClientType::RustCli), "CLI client should be running");
    assert!(clients_by_type.contains_key(&ClientType::Wasm), "WASM client should be running");

    // Start discovery on all clients
    for client_name in orchestrator.running_clients() {
        orchestrator.send_command(&client_name, "discover").await?;
    }

    // Wait for discovery between different client types
    orchestrator.wait_for_peer_event("cli_peer", "PeerDiscovered", "wasm_peer").await?;
    orchestrator.wait_for_peer_event("wasm_peer", "PeerDiscovered", "cli_peer").await?;
    info!("Cross-type peer discovery completed");

    // Test messaging between different implementation types
    orchestrator.send_command("cli_peer", "send Multi-client test message").await?;
    orchestrator.wait_for_event("wasm_peer", "MessageReceived").await?;
    info!("Multi-client messaging successful");

    info!("All client types compatibility test completed successfully");
    Ok(())
}