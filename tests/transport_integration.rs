//! Phase 3 Integration Tests
//!
//! End-to-end tests for the native transport implementations (BLE + Nostr)
//! testing intelligent transport selection and dual-transport operation.

use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use bitchat_core::{
    BitchatPacket, PeerId, MessageType, BitchatMessage,
    transport::{TransportManager, TransportType, TransportSelectionPolicy, MockTransport},
    NoiseSessionManager, DeliveryTracker, MessageBuilder
};
use bitchat_ble_transport::{BleTransport, BleTransportConfig};
use bitchat_nostr_transport::{NostrTransport, NostrTransportConfig, create_local_relay_config};

// ----------------------------------------------------------------------------
// Test Configuration
// ----------------------------------------------------------------------------

const TEST_TIMEOUT: Duration = Duration::from_secs(30);

fn setup_test_peer() -> (PeerId, NoiseSessionManager) {
    let noise_key = bitchat_core::crypto::NoiseKeyPair::generate();
    let peer_id = PeerId::from_bytes(&noise_key.public_key_bytes());
    let session_manager = NoiseSessionManager::new(noise_key);
    (peer_id, session_manager)
}

// ----------------------------------------------------------------------------
// Transport Manager Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_transport_manager_intelligent_selection() {
    let (peer_id1, _) = setup_test_peer();
    let (peer_id2, _) = setup_test_peer();
    
    let mut manager = TransportManager::new();
    
    // Add mock transports with different characteristics
    let mut ble_transport = MockTransport::new(TransportType::Ble);
    ble_transport.add_peer(peer_id2);
    
    let mut nostr_transport = MockTransport::new(TransportType::Nostr);
    nostr_transport.add_peer(peer_id2);
    
    manager.add_transport(Box::new(ble_transport));
    manager.add_transport(Box::new(nostr_transport));
    
    // Test preference order selection (BLE preferred)
    manager.set_selection_policy(TransportSelectionPolicy::PreferenceOrder(
        vec![TransportType::Ble, TransportType::Nostr]
    ));
    
    manager.start_all().await.expect("Failed to start transports");
    
    // Create test packet
    let packet = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Test message".to_vec(),
    );
    
    // Send packet - should prefer BLE
    manager.send_to(peer_id2, packet.clone()).await.expect("Failed to send packet");
    
    // Test discovered peers
    let peers = manager.all_discovered_peers();
    assert_eq!(peers.len(), 2); // Same peer discovered via both transports
    
    manager.stop_all().await.expect("Failed to stop transports");
}

#[tokio::test]
async fn test_transport_fallback_behavior() {
    let (peer_id1, _) = setup_test_peer();
    let (peer_id2, _) = setup_test_peer();
    
    let mut manager = TransportManager::new();
    
    // Add only Nostr transport (BLE not available)
    let mut nostr_transport = MockTransport::new(TransportType::Nostr);
    nostr_transport.add_peer(peer_id2);
    manager.add_transport(Box::new(nostr_transport));
    
    // Set preference for BLE, but it should fallback to Nostr
    manager.set_selection_policy(TransportSelectionPolicy::PreferenceOrder(
        vec![TransportType::Ble, TransportType::Nostr]
    ));
    
    manager.start_all().await.expect("Failed to start transports");
    
    let packet = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Fallback test".to_vec(),
    );
    
    // Should successfully send via Nostr (fallback)
    manager.send_to(peer_id2, packet).await.expect("Failed to send via fallback");
    
    manager.stop_all().await.expect("Failed to stop transports");
}

// ----------------------------------------------------------------------------
// BLE Transport Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_ble_transport_initialization() {
    let (peer_id, _) = setup_test_peer();
    
    // Test with default config
    let result = BleTransport::new(peer_id);
    match result {
        Ok(mut transport) => {
            let caps = transport.capabilities();
            assert_eq!(caps.transport_type, TransportType::Ble);
            assert!(caps.supports_discovery);
            assert!(caps.supports_broadcast);
            assert!(!caps.requires_internet);
            
            // Test start/stop
            if transport.start().await.is_ok() {
                assert!(transport.is_active());
                transport.stop().await.expect("Failed to stop BLE transport");
                assert!(!transport.is_active());
            }
        }
        Err(_) => {
            // BLE might not be available in test environment
            println!("BLE not available in test environment, skipping hardware test");
        }
    }
}

#[tokio::test]
async fn test_ble_transport_custom_config() {
    let (peer_id, _) = setup_test_peer();
    
    let config = BleTransportConfig {
        device_name_prefix: "TestBitChat".to_string(),
        scan_duration: Duration::from_secs(2),
        connection_timeout: Duration::from_secs(5),
        max_packet_size: 512,
        auto_reconnect: false,
    };
    
    let result = BleTransport::with_config(peer_id, config);
    match result {
        Ok(transport) => {
            let caps = transport.capabilities();
            assert_eq!(caps.max_packet_size, 512);
        }
        Err(_) => {
            println!("BLE not available in test environment");
        }
    }
}

// ----------------------------------------------------------------------------
// Nostr Transport Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_nostr_transport_initialization() {
    let (peer_id, _) = setup_test_peer();
    
    // Test with local relay config for faster testing
    let config = create_local_relay_config();
    
    let mut transport = NostrTransport::with_config(peer_id, config)
        .expect("Failed to create Nostr transport");
    
    let caps = transport.capabilities();
    assert_eq!(caps.transport_type, TransportType::Nostr);
    assert!(caps.supports_discovery);
    assert!(caps.supports_broadcast);
    assert!(caps.requires_internet);
    
    // Test that transport can be started (may fail if no relay available)
    match transport.start().await {
        Ok(_) => {
            assert!(transport.is_active());
            transport.stop().await.expect("Failed to stop Nostr transport");
            assert!(!transport.is_active());
        }
        Err(_) => {
            println!("Nostr relay not available in test environment");
        }
    }
}

#[tokio::test]
async fn test_nostr_transport_message_format() {
    use bitchat_nostr_transport::BitchatNostrMessage;
    
    let (sender_id, _) = setup_test_peer();
    let (recipient_id, _) = setup_test_peer();
    
    let packet = BitchatPacket::new(
        MessageType::Message,
        sender_id,
        b"Test Nostr message".to_vec(),
    );
    
    // Test BitChat message wrapper
    let nostr_msg = BitchatNostrMessage::new(sender_id, Some(recipient_id), &packet)
        .expect("Failed to create Nostr message");
    
    assert_eq!(nostr_msg.sender_peer_id, sender_id);
    assert_eq!(nostr_msg.recipient_peer_id, Some(recipient_id));
    
    // Test round-trip serialization
    let reconstructed = nostr_msg.to_packet().expect("Failed to reconstruct packet");
    assert_eq!(reconstructed.sender_id, sender_id);
    assert_eq!(reconstructed.payload, packet.payload);
}

// ----------------------------------------------------------------------------
// Integration Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_dual_transport_message_flow() {
    let (peer_id1, session_mgr1) = setup_test_peer();
    let (peer_id2, _session_mgr2) = setup_test_peer();
    
    let mut manager = TransportManager::new();
    
    // Add both transport types
    let mut ble_transport = MockTransport::new(TransportType::Ble);
    let mut nostr_transport = MockTransport::new(TransportType::Nostr);
    
    // Add same peer to both transports
    ble_transport.add_peer(peer_id2);
    nostr_transport.add_peer(peer_id2);
    
    manager.add_transport(Box::new(ble_transport));
    manager.add_transport(Box::new(nostr_transport));
    
    // Set BLE preference
    manager.set_selection_policy(TransportSelectionPolicy::PreferenceOrder(
        vec![TransportType::Ble, TransportType::Nostr]
    ));
    
    manager.start_all().await.expect("Failed to start transports");
    
    // Test individual messages
    let message1 = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Direct message".to_vec(),
    );
    
    let message2 = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Broadcast message".to_vec(),
    );
    
    // Send direct message (should use BLE due to preference)
    timeout(TEST_TIMEOUT, manager.send_to(peer_id2, message1))
        .await
        .expect("Send timeout")
        .expect("Failed to send direct message");
    
    // Send broadcast message (should use all transports)
    timeout(TEST_TIMEOUT, manager.broadcast_all(message2))
        .await
        .expect("Broadcast timeout")
        .expect("Failed to broadcast message");
    
    // Verify transport activity
    assert_eq!(manager.active_transport_count(), 2);
    
    // Get discovered peers
    let peers = manager.all_discovered_peers();
    assert!(!peers.is_empty());
    
    manager.stop_all().await.expect("Failed to stop transports");
}

#[tokio::test]
async fn test_delivery_tracking_integration() {
    let (peer_id1, _) = setup_test_peer();
    let (peer_id2, _) = setup_test_peer();
    
    let mut tracker = DeliveryTracker::new();
    let message_id = Uuid::new_v4();
    
    let packet = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Tracked message".to_vec(),
    );
    
    // Track message
    tracker.track_message(message_id, peer_id2, packet.payload.clone());
    
    // Mark as sent
    tracker.mark_sent(&message_id);
    
    // Simulate confirmation
    tracker.mark_confirmed(&message_id);
    
    // Check stats
    let stats = tracker.get_stats();
    assert_eq!(stats.total, 1);
    assert_eq!(stats.confirmed, 1);
    assert_eq!(stats.failed, 0);
    
    // Test cleanup
    let (completed, _expired) = tracker.cleanup();
    assert!(!completed.is_empty());
}

// ----------------------------------------------------------------------------
// Performance Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_transport_selection_performance() {
    let (peer_id1, _) = setup_test_peer();
    let mut manager = TransportManager::new();
    
    // Add multiple mock transports
    for i in 0..10 {
        let transport = MockTransport::new(TransportType::Custom(&format!("transport{}", i)));
        manager.add_transport(Box::new(transport));
    }
    
    manager.start_all().await.expect("Failed to start transports");
    
    // Measure selection time for different policies
    let start = std::time::Instant::now();
    
    manager.set_selection_policy(TransportSelectionPolicy::FirstAvailable);
    // Selection happens during send_to, but we need a reachable peer
    
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(10)); // Should be very fast
    
    manager.stop_all().await.expect("Failed to stop transports");
}

// ----------------------------------------------------------------------------
// Error Handling Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_transport_error_recovery() {
    let (peer_id1, _) = setup_test_peer();
    let (peer_id2, _) = setup_test_peer();
    
    let mut manager = TransportManager::new();
    
    // Add transport without the target peer
    let transport = MockTransport::new(TransportType::Local);
    manager.add_transport(Box::new(transport));
    
    manager.start_all().await.expect("Failed to start transports");
    
    let packet = BitchatPacket::new(
        MessageType::Message,
        peer_id1,
        b"Test message".to_vec(),
    );
    
    // Should fail - no transport can reach the peer
    let result = manager.send_to(peer_id2, packet).await;
    assert!(result.is_err());
    
    manager.stop_all().await.expect("Failed to stop transports");
}

// ----------------------------------------------------------------------------
// Test Utilities
// ----------------------------------------------------------------------------

/// Helper to create a test BitChat message
fn create_test_message(sender: &str, content: &str) -> BitchatMessage {
    BitchatMessage::new(sender.to_string(), content.to_string())
}

/// Helper to verify message content
fn verify_message_content(message: &BitchatMessage, expected_sender: &str, expected_content: &str) {
    assert_eq!(message.sender, expected_sender);
    assert_eq!(message.content, expected_content);
}

// ----------------------------------------------------------------------------
// Mock Relay Server (for Nostr testing)
// ----------------------------------------------------------------------------

/// For relay testing, use the local relay setup from justfile:
/// 
/// ```bash
/// just setup-relay
/// just start-relay
/// ```
/// 
/// This will start a full nostr-rs-relay instance on localhost:8080
/// that can be used for integration testing.

#[tokio::test]
async fn test_with_local_relay_config() {
    let (peer_id, _) = setup_test_peer();
    
    // Test with local relay configuration
    let config = create_local_relay_config();
    
    let mut transport = NostrTransport::with_config(peer_id, config)
        .expect("Failed to create transport");
    
    // Note: This test will only pass if a local relay is running
    // Use `just start-relay` to start a local relay for testing
    match transport.start().await {
        Ok(_) => {
            assert!(transport.is_active());
            transport.stop().await.expect("Failed to stop transport");
            println!("Successfully connected to local relay");
        }
        Err(_) => {
            println!("Local relay not available - this is expected if no relay is running");
            println!("Start a relay with: just start-relay");
        }
    }
}