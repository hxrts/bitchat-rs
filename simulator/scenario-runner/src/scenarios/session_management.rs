//! Session management validation scenario
//! 
//! Tests session lifecycle management without requiring peer discovery

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run session management validation test
pub async fn run_session_management(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting session management validation test with {} client...", client_type.name());

    // Start single client and wait for ready event
    orchestrator.start_client_by_type(client_type, "session_test_client".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    info!("Client started successfully");
    
    // Test session configuration commands
    orchestrator.send_command("session_test_client", "configure session-timeout 300").await?;
    // Wait a moment for processing
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Configured session timeout");
    
    orchestrator.send_command("session_test_client", "configure rekey-threshold 1000").await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Configured rekey threshold");
    
    orchestrator.send_command("session_test_client", "configure max-sessions 50").await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Configured maximum sessions");
    
    // Test session status and reporting
    orchestrator.send_command("session_test_client", "sessions").await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Requested session list");
    
    orchestrator.send_command("session_test_client", "session-stats").await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Requested session statistics");
    
    // Test session cleanup commands
    orchestrator.send_command("session_test_client", "cleanup-sessions").await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Triggered session cleanup");
    
    // Get final status report
    orchestrator.send_command("session_test_client", "status").await?;
    orchestrator.wait_for_event("session_test_client", "SystemStatusReport").await?;
    info!("Final status report received");

    info!("Session management validation test completed successfully");
    Ok(())
}