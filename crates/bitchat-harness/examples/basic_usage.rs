//! Basic Test Harness Usage Example
//!
//! This example demonstrates how to use the BitChat test harness
//! for comprehensive protocol testing.

use bitchat_core::PeerId;
use bitchat_harness::{MockTransportConfig, TestHarness};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    println!("BitChat Test Harness - Basic Usage Example");
    println!("==========================================");

    // Example 1: Basic ideal network testing
    println!("\n1. Testing with ideal network conditions...");
    basic_ideal_test().await?;

    // Example 2: Testing with lossy network
    println!("\n2. Testing with lossy network conditions...");
    lossy_network_test().await?;

    // Example 3: Testing with custom configuration
    println!("\n3. Testing with custom network configuration...");
    custom_config_test().await?;

    // Example 4: Network partition testing
    println!("\n4. Testing network partition and healing...");
    partition_test().await?;

    println!("\nAll tests completed successfully!");
    Ok(())
}

async fn basic_ideal_test() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new().await;

    // Add a peer
    let peer_id = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
    harness.add_peer_and_wait(peer_id).await?;

    // Send a message
    harness
        .send_message_to_peer(peer_id, b"Hello, BitChat!".to_vec())
        .await?;

    // Verify message was queued for network delivery
    let outgoing = harness
        .network
        .expect_outgoing_timeout(Duration::from_secs(1))
        .await;
    assert!(outgoing.is_some(), "Message should be queued for delivery");

    println!("   [OK] Message sent and queued successfully");

    harness.shutdown().await?;
    Ok(())
}

async fn lossy_network_test() -> Result<(), Box<dyn std::error::Error>> {
    let harness = TestHarness::lossy().await;

    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    harness.network.add_peer(peer_id).await?;

    // Send multiple messages to test reliability
    for i in 0..10 {
        let message = format!("Message {}", i);
        harness
            .send_message_to_peer(peer_id, message.into_bytes())
            .await?;
    }

    // Check statistics
    let stats = harness.get_network_stats();
    let sent = stats
        .messages_sent
        .load(std::sync::atomic::Ordering::Relaxed);
    let dropped = stats
        .messages_dropped
        .load(std::sync::atomic::Ordering::Relaxed);

    println!(
        "   [OK] Sent: {}, Dropped: {} ({}% loss)",
        sent,
        dropped,
        if sent > 0 { (dropped * 100) / sent } else { 0 }
    );

    harness.shutdown().await?;
    Ok(())
}

async fn custom_config_test() -> Result<(), Box<dyn std::error::Error>> {
    let config = MockTransportConfig {
        latency_range: (100, 200), // 100-200ms latency
        packet_loss_rate: 0.05,    // 5% packet loss
        jitter_factor: 0.2,        // 20% jitter
        duplication_rate: 0.02,    // 2% duplication
        ..Default::default()
    };

    let harness = TestHarness::with_config(config).await;

    let peer_id = PeerId::new([0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81]);
    harness.network.add_peer(peer_id).await?;

    // Send a test message
    harness
        .send_message_to_peer(peer_id, b"Custom config test".to_vec())
        .await?;

    let avg_latency = harness.get_average_latency();
    println!("   [OK] Average latency: {:.2}ms", avg_latency);

    harness.shutdown().await?;
    Ok(())
}

async fn partition_test() -> Result<(), Box<dyn std::error::Error>> {
    let harness = TestHarness::new().await;

    let peer1 = PeerId::new([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11]);
    let peer2 = PeerId::new([0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22]);

    // Add peers
    harness.network.add_peer(peer1).await?;
    harness.network.add_peer(peer2).await?;

    // Send initial messages
    harness
        .send_message_to_peer(peer1, b"Before partition".to_vec())
        .await?;

    // Simulate network partition
    harness.network.simulate_partition().await?;
    println!("   [OK] Network partition simulated");

    // Try to send during partition (should fail or be dropped)
    harness
        .send_message_to_peer(peer1, b"During partition".to_vec())
        .await?;

    // Heal the network
    harness.network.simulate_healing(&[peer1, peer2]).await?;
    println!("   [OK] Network healed");

    // Send message after healing
    harness
        .send_message_to_peer(peer1, b"After healing".to_vec())
        .await?;

    harness.shutdown().await?;
    Ok(())
}
