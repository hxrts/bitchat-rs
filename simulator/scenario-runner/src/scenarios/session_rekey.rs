//! Session rekey scenario
//! 
//! Tests automatic session rekeying under high message load

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run session rekey test
pub async fn run_session_rekey(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting session rekey test with {} clients...", client_type.name());

    // Start clients and wait for ready events
    orchestrator.start_client_by_type(client_type, "client_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    // Wait for peer discovery and session establishment
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    info!("Peer discovery completed");

    // Configure low rekey threshold for testing
    orchestrator.send_command("client_a", "/configure rekey-threshold 5").await?;
    info!("Configured rekey threshold to 5 messages");
    
    // Send multiple messages to trigger rekey
    for i in 0..10 {
        orchestrator.send_command("client_a", &format!("/send Message {}", i)).await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;
        info!("Message {} delivered", i);
    }
    
    // Wait for session rekey event
    orchestrator.wait_for_event("client_a", "SessionRekeyed").await?;
    info!("Session rekey completed successfully");

    info!("Session rekey test completed successfully");
    Ok(())
}