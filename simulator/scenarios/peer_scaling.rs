//! Massive Peer Discovery and Connection Scaling Test Scenario
//!
//! Tests protocol behavior with 100+ peers in discovery range
//! to verify resource management and connection limits.

use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

use crate::test_runner::{TestResult, TestScenario};
use bitchat_core::{BitchatApp, BitchatMessage};

pub struct PeerScalingScenario;

#[async_trait::async_trait]
impl TestScenario for PeerScalingScenario {
    fn name(&self) -> &'static str {
        "massive-peer-discovery-scaling"
    }

    async fn run(&self) -> TestResult {
        info!("Starting massive peer discovery and scaling test...");

        const PEER_COUNT: usize = 50; // Scaled down for CI, real test would use 100+
        const CONNECTION_LIMIT: usize = 20; // Typical BLE connection limit

        // Phase 1: Create large number of advertising peers
        info!("Phase 1: Creating {} advertising peers", PEER_COUNT);
        
        let start_time = Instant::now();
        let mut advertising_peers = Vec::new();
        
        for i in 0..PEER_COUNT {
            let mut peer = BitchatApp::new_with_name(&format!("Peer{:03}", i)).await?;
            peer.set_discovery_mode_only(true).await?; // Only advertise, don't actively connect
            peer.start().await?;
            advertising_peers.push(peer);
            
            // Stagger startup to avoid resource contention
            if i % 10 == 0 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        let setup_duration = start_time.elapsed();
        info!("Created {} peers in {:?}", PEER_COUNT, setup_duration);

        // Phase 2: Test central client discovery performance
        info!("Phase 2: Testing central client discovery performance");
        
        let mut central_client = BitchatApp::new_with_config(|config| {
            config.ble.max_connections = CONNECTION_LIMIT;
            config.ble.discovery_timeout = Duration::from_secs(30);
            config.ble.connection_retry_limit = 3;
        }).await?;

        central_client.start().await?;
        
        // Monitor discovery progress
        let discovery_start = Instant::now();
        let mut discovered_count = 0;
        let mut last_count = 0;
        
        for attempt in 0..30 { // 30 second discovery window
            sleep(Duration::from_secs(1)).await;
            
            let current_peers = central_client.discovered_peers().await;
            discovered_count = current_peers.len();
            
            if discovered_count != last_count {
                info!("Discovery progress: {}/{} peers found", discovered_count, PEER_COUNT);
                last_count = discovered_count;
            }
            
            // Stop early if we've discovered most peers
            if discovered_count >= PEER_COUNT - 5 {
                break;
            }
        }

        let discovery_duration = discovery_start.elapsed();
        info!("Discovered {}/{} peers in {:?}", discovered_count, PEER_COUNT, discovery_duration);
        
        // Should discover a significant portion of peers
        assert!(
            discovered_count >= PEER_COUNT / 2,
            "Should discover at least 50% of peers: {}/{}",
            discovered_count, PEER_COUNT
        );

        // Phase 3: Test connection resource management
        info!("Phase 3: Testing connection resource management");
        
        let connection_stats = central_client.get_connection_stats().await?;
        assert!(
            connection_stats.active_connections <= CONNECTION_LIMIT,
            "Should respect connection limit: {} <= {}",
            connection_stats.active_connections, CONNECTION_LIMIT
        );

        // Test connection cycling (LRU eviction)
        let initial_connected_peers: Vec<_> = central_client.connected_peers().await
            .into_iter().take(5).collect();

        // Force new connections by sending messages to undiscovered peers
        for peer in advertising_peers.iter().take(25) {
            central_client.attempt_connection(peer.peer_id()).await?;
            sleep(Duration::from_millis(100)).await;
        }

        let final_connected_peers = central_client.connected_peers().await;
        assert!(
            final_connected_peers.len() <= CONNECTION_LIMIT,
            "Connection count should still respect limit after cycling"
        );

        // Phase 4: Test broadcast message scaling
        info!("Phase 4: Testing broadcast message scaling with {} peers", discovered_count);
        
        let broadcast_msg = "Scaling test broadcast message";
        let broadcast_start = Instant::now();
        
        central_client.send_message(None, broadcast_msg.to_string()).await?;
        
        // Monitor message propagation
        sleep(Duration::from_secs(5)).await; // Allow propagation
        
        let mut received_count = 0;
        for peer in &advertising_peers {
            let messages = peer.recent_messages().await;
            if messages.iter().any(|m| m.content.contains("Scaling test broadcast")) {
                received_count += 1;
            }
        }

        let broadcast_duration = broadcast_start.elapsed();
        info!("Broadcast reached {}/{} peers in {:?}", received_count, PEER_COUNT, broadcast_duration);
        
        // Should reach a significant portion of peers
        assert!(
            received_count >= discovered_count / 3, // At least 1/3 of discovered peers
            "Broadcast should reach reasonable number of peers: {}/{}",
            received_count, discovered_count
        );

        // Phase 5: Test memory and CPU usage under load
        info!("Phase 5: Testing resource usage under peer load");
        
        let memory_before = central_client.get_memory_usage().await?;
        let cpu_before = central_client.get_cpu_usage().await?;

        // Generate load: rapid discovery attempts
        for _ in 0..100 {
            central_client.force_discovery_scan().await?;
            sleep(Duration::from_millis(10)).await;
        }

        let memory_after = central_client.get_memory_usage().await?;
        let cpu_after = central_client.get_cpu_usage().await?;

        let memory_increase = memory_after - memory_before;
        info!("Memory usage increase: {} KB", memory_increase / 1024);
        
        // Memory should not increase dramatically (no major leaks)
        assert!(
            memory_increase < 50 * 1024 * 1024, // Less than 50MB increase
            "Memory usage should not increase dramatically under load"
        );

        // Phase 6: Test graceful degradation under overload
        info!("Phase 6: Testing graceful degradation under overload");
        
        // Simulate overload by attempting connections to all peers simultaneously
        let overload_start = Instant::now();
        let mut connection_attempts = Vec::new();
        
        for peer in advertising_peers.iter().take(40) {
            let attempt = central_client.attempt_connection(peer.peer_id());
            connection_attempts.push(attempt);
        }

        // Wait for attempts to complete or timeout
        let results = futures::future::join_all(connection_attempts).await;
        let successful_connections = results.iter().filter(|r| r.is_ok()).count();
        let failed_connections = results.iter().filter(|r| r.is_err()).count();

        info!(
            "Overload test: {} successful, {} failed connections",
            successful_connections, failed_connections
        );

        // Should handle overload gracefully (not crash)
        assert!(central_client.is_responsive().await?, "Client should remain responsive under overload");

        // Phase 7: Test cleanup and resource release
        info!("Phase 7: Testing cleanup and resource release");
        
        central_client.disconnect_all_peers().await?;
        sleep(Duration::from_secs(2)).await;

        let final_stats = central_client.get_connection_stats().await?;
        assert_eq!(
            final_stats.active_connections, 0,
            "All connections should be cleaned up"
        );

        // Stop all advertising peers
        for peer in &mut advertising_peers {
            peer.stop().await?;
        }

        let total_duration = start_time.elapsed();
        info!("Peer scaling test completed successfully in {:?}", total_duration);
        TestResult::Success
    }
}