//! Protocol Version Compatibility Test Scenario
//!
//! Tests compatibility between different protocol versions (v1/v2)
//! and graceful fallback handling.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::test_runner::{TestResult, TestScenario};
use bitchat_core::{BitchatApp, BitchatMessage, ProtocolVersion};

pub struct VersionCompatibilityScenario;

#[async_trait::async_trait]
impl TestScenario for VersionCompatibilityScenario {
    fn name(&self) -> &'static str {
        "protocol-version-compatibility"
    }

    async fn run(&self) -> TestResult {
        info!("Starting protocol version compatibility test...");

        // Phase 1: Test v2 to v2 communication (baseline)
        info!("Phase 1: Testing v2 to v2 communication (baseline)");
        
        let mut client_v2a = BitchatApp::new_with_version(2).await?;
        let mut client_v2b = BitchatApp::new_with_version(2).await?;

        client_v2a.start().await?;
        client_v2b.start().await?;
        sleep(Duration::from_secs(3)).await;

        // Test version negotiation
        let negotiated_version = client_v2a.get_negotiated_version(client_v2b.peer_id()).await?;
        assert_eq!(negotiated_version, Some(2), "v2 clients should negotiate v2");

        // Test basic messaging
        let v2_msg = "v2 to v2 message";
        client_v2a.send_message(None, v2_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let messages_v2b = client_v2b.recent_messages().await;
        assert!(messages_v2b.iter().any(|m| m.content.contains("v2 to v2")));

        // Phase 2: Test v1 to v1 communication
        info!("Phase 2: Testing v1 to v1 communication");
        
        let mut client_v1a = BitchatApp::new_with_version(1).await?;
        let mut client_v1b = BitchatApp::new_with_version(1).await?;

        client_v1a.start().await?;
        client_v1b.start().await?;
        sleep(Duration::from_secs(3)).await;

        let v1_msg = "v1 to v1 message";
        client_v1a.send_message(None, v1_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let messages_v1b = client_v1b.recent_messages().await;
        assert!(messages_v1b.iter().any(|m| m.content.contains("v1 to v1")));

        // Phase 3: Test v2 to v1 compatibility (graceful fallback)
        info!("Phase 3: Testing v2 to v1 compatibility (graceful fallback)");
        
        let mut client_mixed_v2 = BitchatApp::new_with_version(2).await?;
        let mut client_mixed_v1 = BitchatApp::new_with_version(1).await?;

        client_mixed_v2.start().await?;
        client_mixed_v1.start().await?;
        sleep(Duration::from_secs(5)).await; // Extra time for version negotiation

        // Verify version negotiation falls back to v1
        let negotiated = client_mixed_v2.get_negotiated_version(client_mixed_v1.peer_id()).await?;
        assert_eq!(negotiated, Some(1), "Mixed v2/v1 should negotiate to v1");

        // Test bidirectional communication
        let mixed_msg_1 = "v2 client to v1 client";
        client_mixed_v2.send_message(None, mixed_msg_1.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let mixed_msg_2 = "v1 client to v2 client";
        client_mixed_v1.send_message(None, mixed_msg_2.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let messages_mixed_v1 = client_mixed_v1.recent_messages().await;
        let messages_mixed_v2 = client_mixed_v2.recent_messages().await;

        assert!(messages_mixed_v1.iter().any(|m| m.content.contains("v2 client to v1")));
        assert!(messages_mixed_v2.iter().any(|m| m.content.contains("v1 client to v2")));

        // Phase 4: Test v1 payload size limits are respected
        info!("Phase 4: Testing v1 payload size limits");
        
        // Create message that would fit in v2 but not v1
        let large_message = "x".repeat(70_000); // Exceeds v1 64KiB limit
        
        let send_result = client_mixed_v2.send_message(
            Some(client_mixed_v1.peer_id()),
            large_message
        ).await;
        
        assert!(send_result.is_err(), "Large message should be rejected when sending to v1 client");

        // Phase 5: Test legacy client handling (no version negotiation)
        info!("Phase 5: Testing legacy client handling");
        
        let mut legacy_client = BitchatApp::new_legacy().await?; // No version negotiation support
        let mut modern_client = BitchatApp::new_with_version(2).await?;

        legacy_client.start().await?;
        modern_client.start().await?;
        sleep(Duration::from_secs(5)).await;

        // Modern client should assume v1 for legacy clients
        let legacy_negotiated = modern_client.get_negotiated_version(legacy_client.peer_id()).await?;
        assert_eq!(legacy_negotiated, Some(1), "Legacy clients should be assumed v1");

        // Test communication with legacy client
        let legacy_msg = "Modern to legacy message";
        modern_client.send_message(None, legacy_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        let legacy_messages = legacy_client.recent_messages().await;
        assert!(legacy_messages.iter().any(|m| m.content.contains("Modern to legacy")));

        // Phase 6: Test version negotiation timeout handling
        info!("Phase 6: Testing version negotiation timeout");
        
        let mut timeout_client = BitchatApp::new_with_config(|config| {
            config.protocol.version_negotiation_timeout = Duration::from_millis(500);
        }).await?;
        
        let mut slow_client = BitchatApp::new_with_delayed_responses(Duration::from_secs(2)).await?;

        timeout_client.start().await?;
        slow_client.start().await?;
        sleep(Duration::from_secs(3)).await;

        // Should fallback to v1 after timeout
        let timeout_negotiated = timeout_client.get_negotiated_version(slow_client.peer_id()).await?;
        assert_eq!(timeout_negotiated, Some(1), "Should fallback to v1 after negotiation timeout");

        // Phase 7: Test unsupported version rejection
        info!("Phase 7: Testing unsupported version rejection");
        
        let mut future_client = BitchatApp::new_with_version(99).await?; // Hypothetical future version
        let mut current_client = BitchatApp::new_with_version(2).await?;

        future_client.start().await?;
        current_client.start().await?;
        sleep(Duration::from_secs(5)).await;

        // Should reject connection or fallback gracefully
        let future_negotiated = current_client.get_negotiated_version(future_client.peer_id()).await?;
        assert!(
            future_negotiated.is_none() || future_negotiated == Some(2),
            "Should reject unsupported version or fallback to supported version"
        );

        info!("Protocol version compatibility test completed successfully");
        TestResult::Success
    }
}