//! Transport Failover and Recovery Test Scenario
//!
//! Tests automatic failover from BLE to Nostr when BLE connection drops,
//! and seamless recovery when BLE becomes available again.

use anyhow::Result;
use tracing::info;

use crate::event_orchestrator::EventOrchestrator;

pub struct TransportFailoverScenario;

impl TransportFailoverScenario {
    pub async fn run(orchestrator: &mut EventOrchestrator) -> Result<()> {
        info!("Starting transport failover test...");

        // Phase 1: Start clients and wait for readiness
        info!("Phase 1: Starting clients with both BLE and Nostr");
        orchestrator.start_rust_client("client_a".to_string()).await?;
        orchestrator.start_rust_client("client_b".to_string()).await?;
        
        orchestrator.wait_for_all_ready().await?;
        
        // Phase 2: Wait for peer discovery via BLE
        info!("Phase 2: Waiting for BLE peer discovery");
        orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
        orchestrator.wait_for_peer_event("client_b", "PeerDiscovered", "client_a").await?;

        // Phase 3: Send message over BLE
        info!("Phase 3: Sending message over BLE");
        orchestrator.send_command("client_a", "/send Hello via BLE").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        // Phase 4: Simulate BLE failure and wait for Nostr fallback
        info!("Phase 4: Simulating BLE failure and testing Nostr fallback");
        orchestrator.send_command("client_a", "/disable-transport ble").await?;
        orchestrator.send_command("client_b", "/disable-transport ble").await?;
        
        // Wait for transport status change
        orchestrator.wait_for_event("client_a", "TransportStatusChanged").await?;
        orchestrator.wait_for_event("client_b", "TransportStatusChanged").await?;

        // Send message over Nostr
        orchestrator.send_command("client_a", "/send Hello via Nostr").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        // Phase 5: Restore BLE and verify recovery
        info!("Phase 5: Restoring BLE and testing recovery");
        orchestrator.send_command("client_a", "/enable-transport ble").await?;
        orchestrator.send_command("client_b", "/enable-transport ble").await?;
        
        // Wait for BLE to become active again
        orchestrator.wait_for_event("client_a", "TransportStatusChanged").await?;
        orchestrator.wait_for_event("client_b", "TransportStatusChanged").await?;

        // Send recovery message
        orchestrator.send_command("client_a", "/send Hello after recovery").await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;

        info!("Transport failover test completed successfully");
        Ok(())
    }
}