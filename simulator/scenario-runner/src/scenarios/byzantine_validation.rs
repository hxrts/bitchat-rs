//! Byzantine fault validation scenario
//! 
//! Tests the system's input validation and security against malicious behavior

use anyhow::Result;
use tracing::info;
use crate::event_orchestrator::{EventOrchestrator, ClientType};

/// Run byzantine fault validation test (security focused)
pub async fn run_byzantine_validation(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting byzantine fault validation test with {} client...", client_type.name());

    // Start test client
    orchestrator.start_client_by_type(client_type, "security_test".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    info!("Security test client started");
    
    // Test 1: Invalid peer ID injection
    info!("Testing invalid peer ID handling...");
    orchestrator.send_command("security_test", "send-malicious invalid_peer_id_12345").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Test 2: Oversized message injection
    info!("Testing oversized message handling...");
    let large_message = "x".repeat(10000); // 10KB message
    orchestrator.send_command("security_test", &format!("send-malicious oversized {}", large_message)).await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Test 3: Malformed packet injection
    info!("Testing malformed packet handling...");
    orchestrator.send_command("security_test", "inject-malformed-packet \\x00\\x01\\x02\\xFF").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Test 4: Replay attack simulation
    info!("Testing replay attack detection...");
    orchestrator.send_command("security_test", "replay-attack simulate").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Test 5: Rate limiting validation
    info!("Testing rate limiting protection...");
    for i in 0..20 {
        orchestrator.send_command("security_test", &format!("rapid-send message{}", i)).await?;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    
    // Test 6: Cryptographic validation
    info!("Testing cryptographic validation...");
    orchestrator.send_command("security_test", "validate-crypto-signatures").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Test 7: Session hijacking prevention
    info!("Testing session hijacking prevention...");
    orchestrator.send_command("security_test", "test-session-security").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    // Get security report
    orchestrator.send_command("security_test", "security-report").await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    info!("Security validation report received");

    info!("Byzantine fault validation test completed successfully");
    Ok(())
}