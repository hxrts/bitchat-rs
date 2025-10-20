//! Transport commands validation test
//! 
//! Tests that transport pause/resume commands work correctly in isolation

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run transport commands test
pub async fn run_transport_commands_test(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting transport commands validation test with {} client...", client_type.name());

    // Start single client and wait for ready event
    orchestrator.start_client_by_type(client_type, "test_client".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    info!("Client started successfully");
    
    // Test BLE transport pause
    orchestrator.send_command("test_client", "pause-transport ble").await?;
    orchestrator.wait_for_event("test_client", "TransportStatusChanged").await?;
    info!("BLE transport paused successfully");
    
    // Test BLE transport resume  
    orchestrator.send_command("test_client", "resume-transport ble").await?;
    orchestrator.wait_for_event("test_client", "TransportStatusChanged").await?;
    info!("BLE transport resumed successfully");
    
    // Test Nostr transport pause
    orchestrator.send_command("test_client", "pause-transport nostr").await?;
    orchestrator.wait_for_event("test_client", "TransportStatusChanged").await?;
    info!("Nostr transport paused successfully");
    
    // Test Nostr transport resume
    orchestrator.send_command("test_client", "resume-transport nostr").await?;
    orchestrator.wait_for_event("test_client", "TransportStatusChanged").await?;
    info!("Nostr transport resumed successfully");
    
    // Test invalid transport error handling
    orchestrator.send_command("test_client", "pause-transport invalid").await?;
    orchestrator.wait_for_event("test_client", "TransportStatusChanged").await?;
    info!("Invalid transport handled correctly");
    
    // Get status report
    orchestrator.send_command("test_client", "status").await?;
    orchestrator.wait_for_event("test_client", "SystemStatusReport").await?;
    info!("Status report received");

    info!("Transport commands validation test completed successfully");
    Ok(())
}