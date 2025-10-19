//! Transport failover scenario
//! 
//! Tests the ability to switch between transports (BLE → Nostr) when one fails

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run transport failover test
pub async fn run_transport_failover(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting transport failover test with {} clients...", client_type.name());

    // Start clients and wait for ready events
    orchestrator.start_client_by_type(client_type, "client_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    // Wait for peer discovery
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    info!("Peer discovery completed");

    // Send initial message over primary transport (BLE)
    orchestrator.send_command("client_a", "/send BLE message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    info!("Message sent successfully over primary transport");
    
    // Simulate transport failure by disabling BLE
    orchestrator.send_command("client_a", "/disable-transport ble").await?;
    orchestrator.wait_for_event("client_a", "TransportStatusChanged").await?;
    info!("BLE transport disabled, should failover to Nostr");
    
    // Send message over fallback transport (Nostr)
    orchestrator.send_command("client_a", "/send Nostr fallback message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    info!("Message sent successfully over fallback transport");

    info!("Transport failover test completed successfully");
    Ok(())
}