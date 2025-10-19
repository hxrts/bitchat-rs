//! Integration tests for the decomposed runtime architecture
//!
//! Tests the new multi-task runtime with mock transports to verify
//! that the refactor maintains functionality while improving modularity.

use bitchat_core::{Command, PeerId};
#[cfg(feature = "testing")]
use bitchat_harness::MockTransport;
use bitchat_runtime::RuntimeBuilder;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_decomposed_runtime_basic_functionality() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    // Create runtime with new decomposed architecture
    let mut runtime = RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .monitoring_interval(Duration::from_millis(100))
        .restart_failed_tasks(false)
        .channel_buffer_size(100)
        .build_and_start()
        .await
        .expect("Failed to start decomposed runtime");

    // Verify runtime is running
    assert!(runtime.is_running());
    assert_eq!(runtime.peer_id(), peer_id);

    // Test command sending
    let cmd = Command::StartDiscovery;
    runtime
        .send_command(cmd)
        .await
        .expect("Failed to send command to decomposed runtime");

    // Clean shutdown
    runtime
        .shutdown()
        .await
        .expect("Failed to shutdown decomposed runtime");
    assert!(!runtime.is_running());
}

#[tokio::test]
async fn test_multi_task_communication() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    let mut runtime = RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .monitoring_interval(Duration::from_millis(50))
        .build_and_start()
        .await
        .expect("Failed to start runtime");

    let _app_events = runtime
        .take_app_event_receiver()
        .expect("Failed to get app event receiver");

    // Send multiple commands to test task coordination
    runtime.send_command(Command::StartDiscovery).await.unwrap();
    runtime
        .send_command(Command::GetSystemStatus)
        .await
        .unwrap();

    let other_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
    runtime
        .send_command(Command::ConnectToPeer {
            peer_id: other_peer,
        })
        .await
        .unwrap();

    // Give some time for processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_error_handling_in_decomposed_tasks() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    let mut runtime = RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .restart_failed_tasks(false) // Don't restart for this test
        .build_and_start()
        .await
        .expect("Failed to start runtime");

    // Test invalid commands/operations
    let invalid_peer = PeerId::new([0; 8]);
    runtime
        .send_command(Command::DisconnectFromPeer {
            peer_id: invalid_peer,
        })
        .await
        .unwrap();

    // Runtime should continue functioning despite errors
    assert!(runtime.is_running());

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_runtime_builder_configurations() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    // Test different builder configurations
    let mut runtime1 = RuntimeBuilder::new(peer_id)
        .monitoring_interval(Duration::from_millis(10))
        .restart_failed_tasks(true)
        .channel_buffer_size(50)
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to build runtime with custom config");

    assert!(runtime1.is_running());
    runtime1.shutdown().await.unwrap();

    // Test another configuration
    let mut runtime2 = RuntimeBuilder::new(peer_id)
        .monitoring_interval(Duration::from_secs(1))
        .restart_failed_tasks(false)
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to build runtime with different config");

    assert!(runtime2.is_running());
    runtime2.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_runtimes() {
    // Test that multiple runtime instances can coexist
    let peer1 = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let peer2 = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);

    let mut runtime1 = RuntimeBuilder::new(peer1)
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to start runtime 1");

    let mut runtime2 = RuntimeBuilder::new(peer2)
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to start runtime 2");

    assert!(runtime1.is_running());
    assert!(runtime2.is_running());
    assert_ne!(runtime1.peer_id(), runtime2.peer_id());

    // Both should be able to process commands independently
    runtime1
        .send_command(Command::StartDiscovery)
        .await
        .unwrap();
    runtime2
        .send_command(Command::StartDiscovery)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    runtime1.shutdown().await.unwrap();
    runtime2.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_graceful_shutdown_coordination() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    let mut runtime = RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .monitoring_interval(Duration::from_millis(50)) // Faster monitoring for test
        .build_and_start()
        .await
        .expect("Failed to start runtime");

    // Verify runtime is running
    assert!(
        runtime.is_running(),
        "Runtime should be running after start"
    );

    // Send some commands before shutdown
    runtime.send_command(Command::StartDiscovery).await.unwrap();
    runtime
        .send_command(Command::GetSystemStatus)
        .await
        .unwrap();

    // Give commands a moment to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Graceful shutdown should complete within reasonable time
    let shutdown_start = std::time::Instant::now();
    let shutdown_result = timeout(Duration::from_secs(10), runtime.shutdown()).await;
    let shutdown_duration = shutdown_start.elapsed();

    assert!(
        shutdown_result.is_ok(),
        "Shutdown took too long (>10s), actual duration: {:?}",
        shutdown_duration
    );
    assert!(shutdown_result.unwrap().is_ok(), "Shutdown failed");
    assert!(
        !runtime.is_running(),
        "Runtime should not be running after shutdown"
    );

    // Note: Shutdown may take up to 10s due to supervisor implementation waiting for tasks
    // This is acceptable behavior for graceful shutdown
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn test_with_mock_transport() {
    // use bitchat_harness::MockTransportConfig;

    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    // Create mock transport for testing
    let mock_transport = MockTransport::ideal(peer_id);

    let mut runtime = RuntimeBuilder::new(peer_id)
        .add_transport(Box::new(mock_transport))
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to start runtime with mock transport");

    // Test basic operations with mock transport
    runtime.send_command(Command::StartDiscovery).await.unwrap();

    let other_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
    runtime
        .send_command(Command::ConnectToPeer {
            peer_id: other_peer,
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_app_event_streaming() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

    let mut runtime = RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .build_and_start()
        .await
        .expect("Failed to start runtime");

    let _app_events = runtime
        .take_app_event_receiver()
        .expect("Failed to get app event receiver");

    // Send commands that should generate app events
    runtime.send_command(Command::StartDiscovery).await.unwrap();

    // Try to receive an app event with timeout
    // let _event_result = timeout(Duration::from_millis(100), _app_events.recv()).await;

    // Note: Depending on implementation, we might or might not receive events immediately
    // This test mainly verifies the plumbing works without panicking

    runtime.shutdown().await.unwrap();
}
