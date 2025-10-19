//! Cross-implementation compatibility test scenario
//! 
//! Tests compatibility between different client implementations

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run cross-implementation compatibility test between specified client types
pub async fn run_cross_implementation_test(
    orchestrator: &mut EventOrchestrator, 
    client1_type: ClientType, 
    client2_type: ClientType
) -> Result<()> {
    info!("Starting cross-implementation compatibility test ({} ↔ {})", 
          client1_type.name(), client2_type.name());

    // Generate client names based on client types
    let client1_name = format!("{}_alice", client1_type.identifier());
    let client2_name = format!("{}_bob", client2_type.identifier());

    // Start clients of specified types
    orchestrator.start_client_by_type(client1_type, client1_name.clone()).await?;
    orchestrator.start_client_by_type(client2_type, client2_name.clone()).await?;

    // Wait for both clients to be ready
    orchestrator.wait_for_all_ready().await?;
    info!("Both {} and {} clients are ready", client1_type.name(), client2_type.name());

    // Start discovery on both clients
    orchestrator.send_command(&client1_name, "discover").await?;
    orchestrator.send_command(&client2_name, "discover").await?;

    // Wait for cross-discovery
    let _client1_discovers_client2 = orchestrator
        .wait_for_peer_event(&client1_name, "PeerDiscovered", &client2_name)
        .await?;
    info!("{} client discovered {} client", client1_type.name(), client2_type.name());

    let _client2_discovers_client1 = orchestrator
        .wait_for_peer_event(&client2_name, "PeerDiscovered", &client1_name)
        .await?;
    info!("{} client discovered {} client", client2_type.name(), client1_type.name());

    // Test bidirectional messaging
    // Client1 → Client2
    let message1 = format!("Hello from {} to {}", client1_type.name(), client2_type.name());
    orchestrator.send_command(&client1_name, &format!("send {}", message1)).await?;
    let _client1_sent = orchestrator.wait_for_event(&client1_name, "MessageSent").await?;
    let client2_received = orchestrator.wait_for_event(&client2_name, "MessageReceived").await?;
    info!("{} → {} message successful", client1_type.name(), client2_type.name());

    // Client2 → Client1
    let message2 = format!("Hello from {} to {}", client2_type.name(), client1_type.name());
    orchestrator.send_command(&client2_name, &format!("send {}", message2)).await?;
    let _client2_sent = orchestrator.wait_for_event(&client2_name, "MessageSent").await?;
    let client1_received = orchestrator.wait_for_event(&client1_name, "MessageReceived").await?;
    info!("{} → {} message successful", client2_type.name(), client1_type.name());

    // Verify message contents
    if let Some(content) = client2_received.data.get("content").and_then(|v| v.as_str()) {
        if content != message1 {
            return Err(anyhow::anyhow!("Message content mismatch: expected '{}', got '{}'", message1, content));
        }
    }

    if let Some(content) = client1_received.data.get("content").and_then(|v| v.as_str()) {
        if content != message2 {
            return Err(anyhow::anyhow!("Message content mismatch: expected '{}', got '{}'", message2, content));
        }
    }

    info!("Cross-implementation compatibility test completed successfully");
    Ok(())
}