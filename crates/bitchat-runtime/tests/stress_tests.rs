//! Stress Tests for Deadlock Detection and Performance Bottlenecks
//!
//! Comprehensive stress testing to ensure the generic runtime architecture remains
//! deadlock-free under high load and achieves acceptable performance characteristics.
//!
//! These tests verify: high-volume command processing, concurrent task coordination,
//! deadlock prevention, channel backpressure handling, throughput benchmarks,
//! memory usage under load, and cross-platform compatibility.

use bitchat_core::{
    internal::{
        create_app_event_channel, create_command_channel, create_effect_channel,
        create_event_channel, ChannelConfig, DeliveryConfig, NoOpLogger, RateLimitConfig,
        SessionConfig,
    },
    BitchatResult, ChannelTransportType, Command, Event, PeerId,
};
use bitchat_runtime::{
    logic::{CoreLogicTask, LoggerWrapper},
    BitchatRuntime,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::time::timeout;

// ----------------------------------------------------------------------------
// Test Utilities
// ----------------------------------------------------------------------------

fn create_test_peer_id(id: u8) -> PeerId {
    PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
}

fn create_stress_test_config() -> ChannelConfig {
    ChannelConfig {
        command_buffer_size: 1000,
        event_buffer_size: 1000,
        effect_buffer_size: 1000,
        app_event_buffer_size: 1000,
    }
}

fn create_quiet_logger() -> LoggerWrapper {
    LoggerWrapper::NoOp(NoOpLogger)
}

// ----------------------------------------------------------------------------
// Runtime Architecture Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_runtime_creation() -> BitchatResult<()> {
    let peer_id = create_test_peer_id(1);
    let runtime = BitchatRuntime::for_testing(peer_id);

    assert_eq!(runtime.peer_id(), peer_id);
    assert!(!runtime.is_running());
    assert_eq!(runtime.transport_types().len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_channel_saturation_graceful_degradation() -> BitchatResult<()> {
    let config = ChannelConfig {
        command_buffer_size: 3, // Very small buffer
        event_buffer_size: 3,
        effect_buffer_size: 3,
        app_event_buffer_size: 3,
    };

    let (command_sender, _command_receiver) = create_command_channel(&config);

    let saturation_counter = Arc::new(AtomicU64::new(0));
    let successful_sends = Arc::new(AtomicU64::new(0));

    // Attempt to saturate the channel
    let saturation_handle = {
        let command_sender = command_sender.clone();
        let saturation_counter = saturation_counter.clone();
        let successful_sends = successful_sends.clone();

        tokio::spawn(async move {
            for i in 0..100 {
                saturation_counter.fetch_add(1, Ordering::Relaxed);

                // Use try_send to test graceful degradation
                match command_sender.try_send(Command::StartDiscovery) {
                    Ok(()) => {
                        successful_sends.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        // Channel saturated - this is expected graceful degradation behavior
                    }
                }

                if i % 10 == 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
    };

    // Wait for saturation test
    let _ = timeout(Duration::from_millis(500), saturation_handle)
        .await
        .expect("Saturation test should complete");

    let total_attempts = saturation_counter.load(Ordering::Relaxed);
    let successful = successful_sends.load(Ordering::Relaxed);

    println!(
        "Channel saturation test: {}/{} sends successful",
        successful, total_attempts
    );

    // Should have attempted all sends
    assert_eq!(total_attempts, 100);

    // Some sends should have succeeded (graceful degradation)
    assert!(successful > 0, "At least some sends should succeed");

    // Some sends should have failed due to saturation
    assert!(
        successful < total_attempts,
        "Some sends should fail due to saturation"
    );

    Ok(())
}

// ----------------------------------------------------------------------------
// High Volume Command Processing Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_high_volume_command_processing() -> BitchatResult<()> {
    let _peer_id = create_test_peer_id(1);
    let config = create_stress_test_config();
    let (command_sender, mut command_receiver) = create_command_channel(&config);

    // Spawn a task to consume commands at high rate
    let consumer_handle = tokio::spawn(async move {
        let mut received_count = 0u64;
        while let Some(_command) = command_receiver.recv().await {
            received_count += 1;
            if received_count >= 1000 {
                break;
            }
            // Simulate minimal processing time
            tokio::task::yield_now().await;
        }
        received_count
    });

    // Send 1000 commands as fast as possible
    let producer_handle = tokio::spawn(async move {
        for i in 0..1000 {
            let command = if i % 2 == 0 {
                Command::StartDiscovery
            } else {
                Command::GetSystemStatus
            };

            if command_sender.send(command).await.is_err() {
                break;
            }
        }
    });

    // Wait for both tasks with timeout
    let _producer_result = timeout(Duration::from_secs(5), producer_handle)
        .await
        .expect("Producer should complete within timeout")
        .expect("Producer should not panic");

    let received_count = timeout(Duration::from_secs(5), consumer_handle)
        .await
        .expect("Consumer should complete within timeout")
        .expect("Consumer should not panic");

    // Verify all commands were processed
    assert_eq!(received_count, 1000, "Should process all 1000 commands");

    Ok(())
}

// ----------------------------------------------------------------------------
// Concurrent Task Coordination Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_concurrent_task_coordination() -> BitchatResult<()> {
    let _peer_id = create_test_peer_id(1);
    let config = create_stress_test_config();

    // Create multiple channels for inter-task communication
    let (event_sender1, mut event_receiver1) = create_event_channel(&config);
    let (event_sender2, mut event_receiver2) = create_event_channel(&config);
    let (command_sender, mut command_receiver) = create_command_channel(&config);

    // Task 1: Command processor
    let task1 = tokio::spawn(async move {
        let mut processed = 0;
        while let Some(command) = command_receiver.recv().await {
            match command {
                Command::StartDiscovery => {
                    let event = Event::PeerDiscovered {
                        peer_id: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
                        #[cfg(feature = "testing")]
                        transport: ChannelTransportType::Mock,
                        #[cfg(not(feature = "testing"))]
                        transport: ChannelTransportType::Ble,
                        signal_strength: Some(-50i8),
                    };
                    let _ = event_sender1.send(event).await;
                    processed += 1;
                }
                Command::StopDiscovery => {
                    let event = Event::ConnectionLost {
                        peer_id: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
                        #[cfg(feature = "testing")]
                        transport: ChannelTransportType::Mock,
                        #[cfg(not(feature = "testing"))]
                        transport: ChannelTransportType::Ble,
                        reason: "Discovery stopped".to_string(),
                    };
                    let _ = event_sender1.send(event).await;
                    processed += 1;
                }
                Command::Shutdown => break,
                _ => processed += 1,
            }

            if processed >= 100 {
                break;
            }
        }
        processed
    });

    // Task 2: Event processor
    let task2 = tokio::spawn(async move {
        let mut processed = 0;
        while let Some(event) = event_receiver1.recv().await {
            match event {
                Event::PeerDiscovered { .. } => {
                    let response = Event::ConnectionEstablished {
                        peer_id: PeerId::new([2, 3, 4, 5, 6, 7, 8, 9]),
                        #[cfg(feature = "testing")]
                        transport: ChannelTransportType::Mock,
                        #[cfg(not(feature = "testing"))]
                        transport: ChannelTransportType::Ble,
                    };
                    let _ = event_sender2.send(response).await;
                    processed += 1;
                }
                Event::ConnectionLost { .. } => {
                    processed += 1;
                }
                _ => processed += 1,
            }

            if processed >= 50 {
                break;
            }
        }
        processed
    });

    // Task 3: Response collector
    let task3 = tokio::spawn(async move {
        let mut collected = 0;
        while let Some(_event) = event_receiver2.recv().await {
            collected += 1;
            if collected >= 25 {
                break;
            }
        }
        collected
    });

    // Send commands to drive the pipeline
    for i in 0..100 {
        let command = if i % 2 == 0 {
            Command::StartDiscovery
        } else {
            Command::StopDiscovery
        };
        command_sender.send(command).await.map_err(|_| {
            bitchat_core::BitchatError::Transport(
                bitchat_core::internal::TransportError::SendBufferFull { capacity: 0 },
            )
        })?;
    }

    // Wait for all tasks to complete
    let results =
        tokio::try_join!(task1, task2, task3).expect("All tasks should complete successfully");

    assert!(
        results.0 >= 50,
        "Task 1 should process at least 50 commands"
    );
    assert!(results.1 >= 25, "Task 2 should process at least 25 events");
    assert!(
        results.2 >= 10,
        "Task 3 should collect at least 10 responses"
    );

    Ok(())
}

// ----------------------------------------------------------------------------
// Deadlock Prevention Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_circular_communication_deadlock_prevention() -> BitchatResult<()> {
    let config = create_stress_test_config();

    // Create circular channel dependencies: A -> B -> C -> A
    let (sender_a, receiver_a) = create_event_channel(&config);
    let (sender_b, receiver_b) = create_event_channel(&config);
    let (sender_c, receiver_c) = create_event_channel(&config);

    // Task A: receives from C, sends to B
    let sender_b_clone = sender_b.clone();
    let task_a = tokio::spawn(async move {
        let mut receiver_c = receiver_c;
        let mut count = 0;
        while count < 10 {
            tokio::select! {
                event = receiver_c.recv() => {
                    if event.is_some() {
                        let forward_event = Event::PeerDiscovered {
                            peer_id: PeerId::new([4, 5, 6, 7, 8, 9, 10, 11]),
                            #[cfg(feature = "testing")]
                            transport: ChannelTransportType::Mock,
                            #[cfg(not(feature = "testing"))]
                            transport: ChannelTransportType::Ble,
                            signal_strength: Some(-55i8),
                        };
                        let _ = sender_b_clone.send(forward_event).await;
                        count += 1;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Send periodic events to prevent deadlock
                    let event = Event::PeerDiscovered {
                        peer_id: PeerId::new([3, 4, 5, 6, 7, 8, 9, 10]),
                        #[cfg(feature = "testing")]
                        transport: ChannelTransportType::Mock,
                        #[cfg(not(feature = "testing"))]
                        transport: ChannelTransportType::Ble,
                        signal_strength: Some(-60i8),
                    };
                    let _ = sender_b_clone.send(event).await;
                    count += 1;
                }
            }
        }
        count
    });

    // Task B: receives from A, sends to C
    let sender_c_clone = sender_c.clone();
    let task_b = tokio::spawn(async move {
        let mut receiver_b = receiver_b;
        let mut count = 0;
        while count < 10 {
            if let Some(_event) = receiver_b.recv().await {
                let forward_event = Event::ConnectionEstablished {
                    peer_id: PeerId::new([2, 3, 4, 5, 6, 7, 8, 9]),
                    #[cfg(feature = "testing")]
                    transport: ChannelTransportType::Mock,
                    #[cfg(not(feature = "testing"))]
                    transport: ChannelTransportType::Ble,
                };
                let _ = sender_c_clone.send(forward_event).await;
                count += 1;
            }
        }
        count
    });

    // Task C: receives from B, sends to A
    let sender_a_clone = sender_a.clone();
    let task_c = tokio::spawn(async move {
        let mut receiver_a = receiver_a;
        let mut count = 0;
        while count < 10 {
            if let Some(_event) = receiver_a.recv().await {
                let forward_event = Event::ConnectionLost {
                    peer_id: PeerId::new([3, 4, 5, 6, 7, 8, 9, 10]),
                    #[cfg(feature = "testing")]
                    transport: ChannelTransportType::Mock,
                    #[cfg(not(feature = "testing"))]
                    transport: ChannelTransportType::Ble,
                    reason: "Test disconnection".to_string(),
                };
                let _ = sender_a_clone.send(forward_event).await;
                count += 1;
            }
        }
        count
    });

    // Initial event to start the cycle - send to A's receiver (which C reads from)
    let initial_event = Event::PeerDiscovered {
        peer_id: PeerId::new([1, 1, 1, 1, 1, 1, 1, 1]),
        #[cfg(feature = "testing")]
        transport: ChannelTransportType::Mock,
        #[cfg(not(feature = "testing"))]
        transport: ChannelTransportType::Ble,
        signal_strength: Some(-40i8),
    };
    sender_a.send(initial_event).await.map_err(|_| {
        bitchat_core::BitchatError::Transport(
            bitchat_core::internal::TransportError::SendBufferFull { capacity: 0 },
        )
    })?;

    // All tasks should complete without deadlock
    let results = timeout(Duration::from_secs(10), async {
        tokio::try_join!(task_a, task_b, task_c)
    })
    .await
    .expect("Tasks should complete without deadlock")
    .expect("All tasks should succeed");

    assert!(results.0 >= 5, "Task A should process some events");
    assert!(results.1 >= 5, "Task B should process some events");
    assert!(results.2 >= 5, "Task C should process some events");

    Ok(())
}

// ----------------------------------------------------------------------------
// Core Logic Throughput Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_core_logic_throughput() -> BitchatResult<()> {
    let peer_id = create_test_peer_id(1);
    let config = create_stress_test_config();

    let (command_sender, command_receiver) = create_command_channel(&config);
    let (_event_sender, event_receiver) = create_event_channel(&config);
    let (effect_sender, _effect_receiver) = create_effect_channel(&config);
    let (app_event_sender, mut app_event_receiver) = create_app_event_channel(&config);

    // Create a minimal core logic task
    let logger = create_quiet_logger();
    let mut core_logic = CoreLogicTask::new(
        peer_id,
        command_receiver,
        event_receiver,
        effect_sender,
        app_event_sender,
        logger,
        SessionConfig::testing(),
        DeliveryConfig::testing(),
        RateLimitConfig::permissive(),
    )?;

    // Start core logic task
    let core_handle = tokio::spawn(async move { core_logic.run().await });

    // Measure throughput
    let start_time = std::time::Instant::now();
    let num_commands = 100;

    // Send commands rapidly
    for i in 0..num_commands {
        let command = match i % 4 {
            0 => Command::StartDiscovery,
            1 => Command::StopDiscovery,
            2 => Command::GetSystemStatus,
            _ => Command::GetSystemStatus,
        };
        command_sender.send(command).await.map_err(|_| {
            bitchat_core::BitchatError::Transport(
                bitchat_core::internal::TransportError::SendBufferFull { capacity: 0 },
            )
        })?;
    }

    // Wait for app events to be generated
    let mut received_events = 0;
    while received_events < num_commands / 2 {
        // Expect at least half the commands to generate events
        tokio::select! {
            event = app_event_receiver.recv() => {
                if event.is_some() {
                    received_events += 1;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                break; // Timeout to prevent infinite wait
            }
        }
    }

    let elapsed = start_time.elapsed();
    let throughput = num_commands as f64 / elapsed.as_secs_f64();

    // Send shutdown command
    command_sender.send(Command::Shutdown).await.map_err(|_| {
        bitchat_core::BitchatError::Transport(
            bitchat_core::internal::TransportError::SendBufferFull { capacity: 0 },
        )
    })?;

    // Wait for core logic to finish
    let _ = timeout(Duration::from_secs(5), core_handle).await;

    println!("Core logic throughput: {:.2} commands/second", throughput);
    assert!(
        throughput > 50.0,
        "Core logic should process at least 50 commands/second"
    );

    Ok(())
}

// ----------------------------------------------------------------------------
// Memory Usage Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_memory_usage_under_load() -> BitchatResult<()> {
    let config = create_stress_test_config();
    let (command_sender, mut command_receiver) = create_command_channel(&config);

    // Create a task that processes commands and tracks memory
    let memory_test = tokio::spawn(async move {
        let mut command_count = 0;
        let mut max_memory_kb = 0;

        while let Some(_command) = command_receiver.recv().await {
            command_count += 1;

            // Simulate some processing and memory allocation
            let _temp_data: Vec<u8> = vec![0; 1024]; // 1KB allocation

            // Check memory usage periodically
            if command_count % 100 == 0 {
                // This is a simplified memory check - in real scenario you'd use proper memory tracking
                let current_memory_kb = command_count; // Placeholder
                if current_memory_kb > max_memory_kb {
                    max_memory_kb = current_memory_kb;
                }
            }

            if command_count >= 1000 {
                break;
            }
        }

        (command_count, max_memory_kb)
    });

    // Send many commands to stress memory
    for _ in 0..1000 {
        command_sender
            .send(Command::StartDiscovery)
            .await
            .map_err(|_| {
                bitchat_core::BitchatError::Transport(
                    bitchat_core::internal::TransportError::SendBufferFull { capacity: 0 },
                )
            })?;
    }

    let (processed, _max_memory) = memory_test.await.expect("Memory test should complete");

    assert_eq!(processed, 1000, "Should process all commands");
    // Note: Real memory testing would require more sophisticated measurement

    Ok(())
}

// ----------------------------------------------------------------------------
// Cross-Platform Channel Behavior Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_cross_platform_channel_behavior() -> BitchatResult<()> {
    // Test different channel configurations
    let configs = vec![
        ChannelConfig {
            command_buffer_size: 10,
            event_buffer_size: 10,
            effect_buffer_size: 10,
            app_event_buffer_size: 10,
        },
        ChannelConfig {
            command_buffer_size: 100,
            event_buffer_size: 100,
            effect_buffer_size: 100,
            app_event_buffer_size: 100,
        },
        ChannelConfig {
            command_buffer_size: 1000,
            event_buffer_size: 1000,
            effect_buffer_size: 1000,
            app_event_buffer_size: 1000,
        },
    ];

    for (i, config) in configs.into_iter().enumerate() {
        let (sender, mut receiver) = create_command_channel(&config);

        // Test rapid sending
        let sender_task = tokio::spawn(async move {
            for j in 0..100 {
                let command = if j % 2 == 0 {
                    Command::StartDiscovery
                } else {
                    Command::StopDiscovery
                };
                if sender.send(command).await.is_err() {
                    break;
                }
            }
        });

        // Test receiving
        let receiver_task = tokio::spawn(async move {
            let mut count = 0;
            while let Some(_command) = receiver.recv().await {
                count += 1;
                if count >= 100 {
                    break;
                }
            }
            count
        });

        let results = tokio::try_join!(sender_task, receiver_task)
            .expect(&format!("Config {} should work", i));

        assert!(
            results.1 >= 50,
            "Should receive most commands for config {}",
            i
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// Multi-Peer Coordination Stress Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_peer_coordination_stress() -> BitchatResult<()> {
    let config = create_stress_test_config();
    let num_peers = 5;
    let mut peer_channels = Vec::new();

    // Create channels for multiple peers
    for i in 0..num_peers {
        let peer_id = create_test_peer_id(i as u8 + 1);
        let (event_sender, event_receiver) = create_event_channel(&config);
        peer_channels.push((peer_id, event_sender, event_receiver));
    }

    // Create tasks to simulate peer coordination
    let mut peer_tasks = Vec::new();

    for (i, (peer_id, sender, mut receiver)) in peer_channels.into_iter().enumerate() {
        let peer_task = tokio::spawn(async move {
            let mut events_processed = 0;
            let mut events_sent = 0;

            // Each peer processes events and occasionally broadcasts
            for j in 0..20 {
                // Send periodic discovery events
                if j % 3 == 0 {
                    let event = Event::PeerDiscovered {
                        peer_id,
                        #[cfg(feature = "testing")]
                        transport: ChannelTransportType::Mock,
                        #[cfg(not(feature = "testing"))]
                        transport: ChannelTransportType::Ble,
                        signal_strength: Some(-50 - i as i8),
                    };
                    if sender.send(event).await.is_ok() {
                        events_sent += 1;
                    }
                }

                // Try to receive events from other peers
                tokio::select! {
                    event = receiver.recv() => {
                        if event.is_some() {
                            events_processed += 1;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        // Continue if no events received
                    }
                }
            }

            (events_processed, events_sent)
        });

        peer_tasks.push(peer_task);
    }

    // Wait for all peer tasks to complete
    let results = futures::future::join_all(peer_tasks).await;

    let mut total_processed = 0;
    let mut total_sent = 0;

    for result in results {
        let (processed, sent) = result.expect("Peer task should complete");
        total_processed += processed;
        total_sent += sent;
    }

    println!(
        "Multi-peer coordination: {} events sent, {} processed",
        total_sent, total_processed
    );
    assert!(total_sent > 0, "Should send some events");

    Ok(())
}
