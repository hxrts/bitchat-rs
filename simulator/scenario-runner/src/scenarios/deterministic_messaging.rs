//! Deterministic messaging scenario
//! 
//! Tests basic message exchange between two clients without timeouts or sleep calls

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run deterministic messaging test
pub async fn run_deterministic_messaging(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting deterministic messaging test with {} clients...", client_type.name());

    // Start clients and wait for ready events (NO SLEEP)
    orchestrator.start_client_by_type(client_type, "alice".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "bob".to_string()).await?;

    // Wait for both clients to be ready - deterministic, no timeouts
    orchestrator.wait_for_all_ready().await?;

    // Wait for peer discovery (EVENT-DRIVEN)
    let _discovery_event = orchestrator
        .wait_for_peer_event("alice", "PeerDiscovered", "bob")
        .await?;
    info!("Alice discovered Bob");

    orchestrator
        .wait_for_peer_event("bob", "PeerDiscovered", "alice")
        .await?;
    info!("Bob discovered Alice");

    // Wait for session establishment (EVENT-DRIVEN)
    orchestrator
        .wait_for_peer_event("alice", "SessionEstablished", "bob")
        .await?;
    orchestrator
        .wait_for_peer_event("bob", "SessionEstablished", "alice")
        .await?;
    info!("Bidirectional sessions established");

    // Send message and verify delivery (EVENT-DRIVEN)
    orchestrator.send_command("alice", "send Hello from Alice").await?;
    
    // Wait for Alice's MessageSent event
    let _sent_event = orchestrator
        .wait_for_event("alice", "MessageSent")
        .await?;

    // Wait for Bob's MessageReceived event
    let received_event = orchestrator
        .wait_for_event("bob", "MessageReceived")
        .await?;
    
    // Verify message content matches
    let received_content = received_event.data.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No content in received event"))?;
    
    if received_content != "Hello from Alice" {
        return Err(anyhow::anyhow!(
            "Message content mismatch: expected 'Hello from Alice', got '{}'",
            received_content
        ));
    }

    info!("Message '{}' delivered successfully", received_content);
    info!("Deterministic messaging test completed successfully");
    Ok(())
}