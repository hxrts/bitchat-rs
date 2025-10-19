//! Mesh Partitioning and Healing Test Scenario
//!
//! Tests mesh network behavior when split into isolated clusters
//! and verifies proper healing when connectivity is restored.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::test_runner::{TestResult, TestScenario};
use bitchat_core::{BitchatApp, BitchatMessage};

pub struct MeshPartitionScenario;

#[async_trait::async_trait]
impl TestScenario for MeshPartitionScenario {
    fn name(&self) -> &'static str {
        "mesh-partition-healing"
    }

    async fn run(&self) -> TestResult {
        info!("Starting mesh partitioning and healing test...");

        // Phase 1: Create connected mesh topology
        info!("Phase 1: Creating 6-node mesh topology");
        
        let mut nodes = Vec::new();
        for i in 0..6 {
            let mut node = BitchatApp::new_with_name(&format!("Node{}", i)).await?;
            node.start().await?;
            nodes.push(node);
        }

        // Allow full mesh discovery
        sleep(Duration::from_secs(8)).await;

        // Verify full connectivity
        for (i, node) in nodes.iter().enumerate() {
            let peers = node.discovered_peers().await;
            assert!(
                peers.len() >= 4, // Should see most other nodes
                "Node {} should discover most peers in full mesh", i
            );
        }

        // Phase 2: Send messages across the mesh
        info!("Phase 2: Testing message propagation across full mesh");
        
        let test_msg = "Pre-partition broadcast message";
        nodes[0].send_message(None, test_msg.to_string()).await?;
        sleep(Duration::from_secs(3)).await;

        // Verify all nodes received the broadcast
        for (i, node) in nodes.iter().enumerate() {
            let messages = node.recent_messages().await;
            assert!(
                messages.iter().any(|m| m.content.contains("Pre-partition")),
                "Node {} should receive broadcast in full mesh", i
            );
        }

        // Phase 3: Create partition (split into two clusters)
        info!("Phase 3: Creating mesh partition (Cluster A: 0,1,2 | Cluster B: 3,4,5)");
        
        // Simulate partition by blocking connections between clusters
        for i in 0..3 {
            for j in 3..6 {
                nodes[i].block_peer_connection(nodes[j].peer_id()).await?;
                nodes[j].block_peer_connection(nodes[i].peer_id()).await?;
            }
        }

        sleep(Duration::from_secs(5)).await; // Allow partition to stabilize

        // Phase 4: Test isolated cluster communication
        info!("Phase 4: Testing communication within isolated clusters");
        
        // Send message in Cluster A
        let cluster_a_msg = "Cluster A isolated message";
        nodes[1].send_message(None, cluster_a_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        // Send message in Cluster B  
        let cluster_b_msg = "Cluster B isolated message";
        nodes[4].send_message(None, cluster_b_msg.to_string()).await?;
        sleep(Duration::from_secs(2)).await;

        // Verify cluster isolation
        let node2_messages = nodes[2].recent_messages().await;
        let node5_messages = nodes[5].recent_messages().await;

        assert!(
            node2_messages.iter().any(|m| m.content.contains("Cluster A")),
            "Node 2 should receive Cluster A message"
        );
        assert!(
            !node2_messages.iter().any(|m| m.content.contains("Cluster B")),
            "Node 2 should NOT receive Cluster B message"
        );
        assert!(
            node5_messages.iter().any(|m| m.content.contains("Cluster B")),
            "Node 5 should receive Cluster B message"
        );
        assert!(
            !node5_messages.iter().any(|m| m.content.contains("Cluster A")),
            "Node 5 should NOT receive Cluster A message"
        );

        // Phase 5: Heal the partition
        info!("Phase 5: Healing mesh partition");
        
        // Restore connections between clusters
        for i in 0..3 {
            for j in 3..6 {
                nodes[i].unblock_peer_connection(nodes[j].peer_id()).await?;
                nodes[j].unblock_peer_connection(nodes[i].peer_id()).await?;
            }
        }

        sleep(Duration::from_secs(8)).await; // Allow mesh to heal

        // Phase 6: Test mesh healing via GCS sync
        info!("Phase 6: Testing mesh healing and state synchronization");
        
        // Force sync to propagate missed messages
        for node in &mut nodes {
            node.force_sync_request().await?;
        }
        
        sleep(Duration::from_secs(5)).await;

        // Verify all nodes now have both cluster messages
        for (i, node) in nodes.iter().enumerate() {
            let messages = node.recent_messages().await;
            assert!(
                messages.iter().any(|m| m.content.contains("Cluster A")),
                "Node {} should have Cluster A message after healing", i
            );
            assert!(
                messages.iter().any(|m| m.content.contains("Cluster B")),
                "Node {} should have Cluster B message after healing", i
            );
        }

        // Phase 7: Test post-healing communication
        info!("Phase 7: Testing communication after mesh healing");
        
        let post_heal_msg = "Post-healing broadcast test";
        nodes[0].send_message(None, post_heal_msg.to_string()).await?;
        sleep(Duration::from_secs(3)).await;

        // Verify all nodes receive new broadcast
        for (i, node) in nodes.iter().enumerate() {
            let messages = node.recent_messages().await;
            assert!(
                messages.iter().any(|m| m.content.contains("Post-healing")),
                "Node {} should receive post-healing broadcast", i
            );
        }

        // Verify mesh topology is fully connected again
        for (i, node) in nodes.iter().enumerate() {
            let peers = node.discovered_peers().await;
            assert!(
                peers.len() >= 4,
                "Node {} should rediscover most peers after healing", i
            );
        }

        info!("Mesh partitioning and healing test completed successfully");
        TestResult::Success
    }
}