//! Panic Action and State Recovery Test Scenario
//!
//! Tests emergency state wipe (triple-tap panic) and clean recovery,
//! critical for activist/journalist security model.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::test_runner::{TestResult, TestScenario};
use bitchat_core::{BitchatApp, BitchatMessage};

pub struct PanicRecoveryScenario;

#[async_trait::async_trait]
impl TestScenario for PanicRecoveryScenario {
    fn name(&self) -> &'static str {
        "panic-action-recovery"
    }

    async fn run(&self) -> TestResult {
        info!("Starting panic action and recovery test...");

        // Phase 1: Setup normal operation with sensitive data
        info!("Phase 1: Establishing normal operation with sensitive data");
        
        let mut client_a = BitchatApp::new().await?;
        let mut client_b = BitchatApp::new().await?;

        client_a.start().await?;
        client_b.start().await?;
        sleep(Duration::from_secs(3)).await;

        // Create sensitive messages and state
        let sensitive_msgs = vec![
            "Meeting at location X tomorrow",
            "Contact encrypted: sensitive-info@example.com",
            "Financial details: account 123456789"
        ];

        for msg in &sensitive_msgs {
            client_a.send_message(None, msg.to_string()).await?;
            sleep(Duration::from_millis(500)).await;
        }

        // Establish favorites and trusted peers
        let peer_b_id = client_b.peer_id();
        client_a.add_favorite_peer(peer_b_id, "TrustedContact").await?;
        client_a.set_peer_verification(peer_b_id, true).await?;

        // Verify state exists
        let pre_panic_messages = client_a.recent_messages().await;
        let pre_panic_favorites = client_a.get_favorite_peers().await?;
        let pre_panic_sessions = client_a.active_noise_sessions().await?;

        assert!(!pre_panic_messages.is_empty(), "Should have messages before panic");
        assert!(!pre_panic_favorites.is_empty(), "Should have favorites before panic");
        assert!(!pre_panic_sessions.is_empty(), "Should have active sessions before panic");

        // Phase 2: Trigger panic action
        info!("Phase 2: Triggering panic action (triple-tap simulation)");
        
        client_a.trigger_panic_action().await?;

        // Brief pause for panic action to complete
        sleep(Duration::from_millis(500)).await;

        // Phase 3: Verify complete state wipe
        info!("Phase 3: Verifying complete state wipe");
        
        let post_panic_messages = client_a.recent_messages().await;
        let post_panic_favorites = client_a.get_favorite_peers().await?;
        let post_panic_sessions = client_a.active_noise_sessions().await?;
        let post_panic_keys = client_a.get_identity_keys().await?;

        assert!(post_panic_messages.is_empty(), "All messages should be wiped");
        assert!(post_panic_favorites.is_empty(), "All favorites should be wiped");
        assert!(post_panic_sessions.is_empty(), "All sessions should be closed");
        assert!(post_panic_keys.is_none(), "Identity keys should be regenerated");

        // Verify transports are disconnected
        let transport_status = client_a.get_transport_status().await?;
        assert!(!transport_status.ble_active, "BLE should be disconnected");
        assert!(!transport_status.nostr_active, "Nostr should be disconnected");

        // Phase 4: Test clean recovery
        info!("Phase 4: Testing clean recovery after panic");
        
        // Should be able to restart cleanly with new identity
        client_a.restart().await?;
        sleep(Duration::from_secs(2)).await;

        let new_peer_id = client_a.peer_id();
        assert_ne!(new_peer_id, peer_b_id, "Should have new peer ID after panic");

        // Should be able to establish new connections
        let discovery_result = client_a.discover_peers().await?;
        assert!(discovery_result.is_ok(), "Should be able to discover peers after recovery");

        // Phase 5: Verify old data is unrecoverable
        info!("Phase 5: Verifying old data is unrecoverable");
        
        // Try to recover old sessions - should fail
        let recovery_attempt = client_a.recover_session(peer_b_id).await;
        assert!(recovery_attempt.is_err(), "Old sessions should be unrecoverable");

        // Try to access old messages - should be empty
        let recovered_messages = client_a.recent_messages().await;
        assert!(recovered_messages.is_empty(), "Old messages should be unrecoverable");

        // Phase 6: Test new communication works
        info!("Phase 6: Testing new communication after recovery");
        
        // Re-establish connection with Client B (new handshake required)
        sleep(Duration::from_secs(5)).await; // Allow discovery

        let test_msg = "New communication after panic recovery";
        client_a.send_message(None, test_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let client_b_messages = client_b.recent_messages().await;
        assert!(
            client_b_messages.iter().any(|m| m.content.contains("New communication")),
            "Should be able to communicate after recovery"
        );

        info!("Panic action and recovery test completed successfully");
        TestResult::Success
    }
}