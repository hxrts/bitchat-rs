//! Session Rekey Under Load Test Scenario
//!
//! Tests automatic Noise session rekey under high message throughput
//! to ensure forward secrecy is maintained without message loss.

use anyhow::Result;
use tracing::info;

use crate::event_orchestrator::EventOrchestrator;

pub struct SessionRekeyScenario;

impl SessionRekeyScenario {
    pub async fn run(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Starting session rekey under load test...");

        // Phase 1: Start clients with low rekey threshold
        info!("Phase 1: Starting clients with aggressive rekey settings");
        orchestrator.start_rust_client("client_a".to_string()).await?;
        orchestrator.start_rust_client("client_b".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;
        
        // Configure low rekey threshold for testing
        orchestrator.send_command("client_a", "/configure rekey-threshold 20").await?;
        
        // Wait for peer discovery and session establishment
        orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
        orchestrator.wait_for_event("client_a", "SessionEstablished").await?;

        // Phase 2: Send burst of messages to trigger rekey
        info!("Phase 2: Sending high-frequency messages to trigger rekey");
        let mut messages_sent = 0;
        
        // Send messages to trigger rekey (25 messages > 20 threshold)
        for i in 0..25 {
            orchestrator.send_command("client_a", &format!("/send Load test message {}", i)).await?;
            
            // Wait for message receipt
            orchestrator.wait_for_event("client_b", "MessageReceived").await?;
            messages_sent += 1;
        }

        // Phase 3: Wait for automatic rekey event
        info!("Phase 3: Waiting for automatic session rekey");
        orchestrator.wait_for_event("client_a", "SessionRekeyed").await?;
        orchestrator.wait_for_event("client_b", "SessionRekeyed").await?;

        // Phase 4: Verify post-rekey communication
        info!("Phase 4: Testing post-rekey communication");
        orchestrator.send_command("client_a", "/send Post-rekey test message").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        // Phase 5: Send final burst to verify session stability
        info!("Phase 5: Testing session stability after rekey");
        for i in 0..5 {
            orchestrator.send_command("client_a", &format!("/send Post-rekey message {}", i)).await?;
            orchestrator.wait_for_event("client_b", "MessageReceived").await?;
        }

        info!("Session rekey test completed successfully");
        Ok(())
    }
}