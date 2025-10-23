//! High-Level Test Harness
//!
//! Provides a simplified interface for testing BitChat protocols without
//! the boilerplate of setting up channels, runtime, and mock transports.

use crate::{MockTransport, MockTransportConfig};
use bitchat_core::{BitchatResult, PeerId};

use alloc::vec::Vec;
#[cfg(feature = "testing")]
#[allow(unused_imports)]
use bitchat_core::{AppEvent, Command};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, Mutex};

#[cfg(not(feature = "std"))]
use log::{debug, info, warn};
#[cfg(feature = "std")]
use tracing::{debug, info, warn};

/// High-level test harness that encapsulates all setup for BitChat testing
pub struct TestHarness {
    /// Send commands to the BitChat system (simplified for testing)
    pub command_sender: mpsc::UnboundedSender<String>,
    /// Receive application events from the system (simplified for testing)
    pub app_event_receiver: mpsc::UnboundedReceiver<String>,
    /// Handle for controlling mock network behavior
    pub network: MockNetworkHandle,

    // Internal state
    runtime_handle: tokio::task::JoinHandle<BitchatResult<()>>,
    peer_id: PeerId,
}

impl TestHarness {
    /// Create a new test harness with ideal network conditions
    pub async fn new() -> Self {
        Self::with_config(MockTransportConfig::ideal()).await
    }

    /// Create a test harness with lossy network conditions
    pub async fn lossy() -> Self {
        Self::with_config(MockTransportConfig::lossy()).await
    }

    /// Create a test harness with high latency network conditions
    pub async fn high_latency() -> Self {
        Self::with_config(MockTransportConfig::high_latency()).await
    }

    /// Create a test harness with mobile network conditions
    pub async fn mobile() -> Self {
        Self::with_config(MockTransportConfig::mobile()).await
    }

    /// Create a test harness with adversarial network conditions
    pub async fn adversarial() -> Self {
        Self::with_config(MockTransportConfig::adversarial()).await
    }

    /// Create a test harness with custom network configuration
    pub async fn with_config(network_config: MockTransportConfig) -> Self {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]); // Default test peer ID

        // Create mock transport
        let mock_transport = MockTransport::new(peer_id, network_config);

        // Create the network handle before consuming the transport
        let network = MockNetworkHandle::new_from_transport(mock_transport.clone());

        // Create mock channels for testing
        let (command_sender, mut command_receiver) = mpsc::unbounded_channel::<String>();
        let (app_event_sender, app_event_receiver) = mpsc::unbounded_channel::<String>();

        // Create a proper mock runtime that processes commands
        let join_handle = tokio::spawn(async move {
            info!("Mock runtime starting for peer {:?}", peer_id);
            let mut shutdown_requested = false;

            while !shutdown_requested {
                tokio::select! {
                    // Process incoming commands
                    command = command_receiver.recv() => {
                        match command {
                            Some(cmd) => {
                                debug!("Processing command: {}", cmd);

                                // Process basic commands
                                if cmd == "Shutdown" {
                                    info!("Shutdown command received");
                                    shutdown_requested = true;
                                } else if cmd.starts_with("/send") {
                                    // Mock processing send commands
                                    let _ = app_event_sender.send(format!("MessageSent: {}", cmd));
                                } else if cmd.starts_with("/connect") {
                                    // Mock processing connect commands
                                    let _ = app_event_sender.send(format!("ConnectionAttempt: {}", cmd));
                                } else if cmd.starts_with("/start_discovery") {
                                    // Mock processing discovery commands
                                    let _ = app_event_sender.send("DiscoveryStarted".to_string());
                                } else if cmd.starts_with("/stop_discovery") {
                                    // Mock processing discovery stop commands
                                    let _ = app_event_sender.send("DiscoveryStopped".to_string());
                                } else {
                                    // Generic command processing
                                    let _ = app_event_sender.send(format!("CommandProcessed: {}", cmd));
                                }
                            }
                            None => {
                                debug!("Command channel closed");
                                break;
                            }
                        }
                    }

                    // Timeout to prevent infinite loops in testing
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Periodic processing - only send heartbeat occasionally
                        if !shutdown_requested {
                            let _ = app_event_sender.send("Heartbeat".to_string());
                        }
                    }
                }
            }

            info!("Mock runtime shutting down for peer {:?}", peer_id);
            Ok(())
        });

        info!("Test harness created for peer {:?}", peer_id);

        Self {
            command_sender,
            app_event_receiver,
            network,
            runtime_handle: join_handle,
            peer_id,
        }
    }

    /// Get the peer ID for this test harness
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Wait for a specific application event
    pub async fn wait_for_event(&mut self) -> Option<String> {
        self.app_event_receiver.recv().await
    }

    /// Wait for a specific type of event with timeout
    pub async fn wait_for_event_timeout(&mut self, timeout: Duration) -> Option<String> {
        tokio::time::timeout(timeout, self.app_event_receiver.recv())
            .await
            .ok()
            .flatten()
    }

    /// Send a command and wait for completion
    pub async fn send_command(&self, command: String) -> BitchatResult<()> {
        self.command_sender
            .send(command)
            .map_err(|e| bitchat_core::BitchatError::Channel {
                message: format!("Failed to send command: {}", e),
            })
    }

    /// Send a broadcast message and verify it was queued for network delivery
    pub async fn send_broadcast_message(&self, content: Vec<u8>) -> BitchatResult<()> {
        self.send_command(format!("SendBroadcast {:?}", content))
            .await
    }

    /// Send a message to a specific peer and verify it was queued
    pub async fn send_message_to_peer(
        &self,
        peer_id: PeerId,
        content: Vec<u8>,
    ) -> BitchatResult<()> {
        self.send_command(format!("SendMessage {:?} {:?}", peer_id, content))
            .await
    }

    /// Add a peer and wait for the peer discovery event
    pub async fn add_peer_and_wait(&mut self, peer_id: PeerId) -> BitchatResult<()> {
        self.network.add_peer(peer_id).await?;

        // Wait for peer discovered event (simplified)
        loop {
            if let Some(event) = self.wait_for_event_timeout(Duration::from_secs(5)).await {
                if event.contains("PeerDiscovered") && event.contains(&format!("{:?}", peer_id)) {
                    return Ok(());
                }
            } else {
                return Err(bitchat_core::BitchatError::Channel {
                    message: "Timeout waiting for peer discovery".to_string(),
                });
            }
        }
    }

    /// Wait for multiple events matching a predicate
    pub async fn wait_for_events<P>(
        &mut self,
        count: usize,
        timeout: Duration,
        predicate: P,
    ) -> Vec<String>
    where
        P: Fn(&String) -> bool,
    {
        let mut events = Vec::new();
        let start = std::time::Instant::now();

        while events.len() < count && start.elapsed() < timeout {
            if let Some(event) = self.wait_for_event_timeout(timeout - start.elapsed()).await {
                if predicate(&event) {
                    events.push(event);
                }
            } else {
                break;
            }
        }

        events
    }

    /// Get network performance statistics
    pub fn get_network_stats(&self) -> crate::MockTransportStats {
        self.network.get_stats()
    }

    /// Get average network latency
    pub fn get_average_latency(&self) -> f64 {
        self.network.average_latency_ms()
    }

    /// Check if any outgoing packets are pending
    pub async fn has_pending_outgoing(&self) -> bool {
        self.network.has_outgoing().await
    }

    /// Drain all pending outgoing packets
    pub async fn drain_outgoing_packets(&self) -> Vec<crate::NetworkPacket> {
        self.network.drain_outgoing().await
    }

    /// Shutdown the test harness
    pub async fn shutdown(self) -> BitchatResult<()> {
        // Send shutdown command
        let _ = self.command_sender.send("Shutdown".to_string());

        // Wait for runtime to finish
        match self.runtime_handle.await {
            Ok(result) => result,
            Err(e) => {
                warn!("Runtime task panicked: {:?}", e);
                Ok(())
            }
        }
    }
}

/// Handle for controlling mock network behavior during tests
#[derive(Clone)]
pub struct MockNetworkHandle {
    outgoing_packets: Arc<Mutex<VecDeque<NetworkPacket>>>,
    incoming_packets: Arc<Mutex<VecDeque<NetworkPacket>>>,
    transport: Arc<MockTransport>,
}

impl MockNetworkHandle {
    fn new_from_transport(transport: MockTransport) -> Self {
        Self {
            outgoing_packets: Arc::new(Mutex::new(VecDeque::new())),
            incoming_packets: Arc::new(Mutex::new(VecDeque::new())),
            transport: Arc::new(transport),
        }
    }

    /// Add a peer to the mock network
    pub async fn add_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        self.transport.add_peer(peer_id).await
    }

    /// Remove a peer from the mock network
    pub async fn remove_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        self.transport.remove_peer(peer_id).await
    }

    /// Inject an incoming packet from a peer
    pub async fn inject_incoming(&self, packet: NetworkPacket) {
        let mut packets = self.incoming_packets.lock().await;
        packets.push_back(packet);
    }

    /// Wait for an outgoing packet to be sent
    pub async fn expect_outgoing(&self) -> NetworkPacket {
        self.expect_outgoing_timeout(Duration::from_secs(5))
            .await
            .expect("Timeout waiting for outgoing packet")
    }

    /// Wait for an outgoing packet with timeout
    pub async fn expect_outgoing_timeout(&self, timeout: Duration) -> Option<NetworkPacket> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            {
                let mut packets = self.outgoing_packets.lock().await;
                if let Some(packet) = packets.pop_front() {
                    return Some(packet);
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        None
    }

    /// Get all pending outgoing packets
    pub async fn drain_outgoing(&self) -> Vec<NetworkPacket> {
        let mut packets = self.outgoing_packets.lock().await;
        packets.drain(..).collect()
    }

    /// Check if there are any pending outgoing packets
    pub async fn has_outgoing(&self) -> bool {
        let packets = self.outgoing_packets.lock().await;
        !packets.is_empty()
    }

    /// Get transport statistics
    pub fn get_stats(&self) -> crate::MockTransportStats {
        self.transport.get_stats()
    }

    /// Get average network latency
    pub fn average_latency_ms(&self) -> f64 {
        self.transport.average_latency_ms()
    }

    /// Simulate network partition (disconnect all peers)
    pub async fn simulate_partition(&self) -> BitchatResult<()> {
        let peers: Vec<PeerId> = {
            let connected_peers = self.transport.connected_peers.read().await;
            connected_peers.keys().cloned().collect()
        };

        for peer_id in peers {
            self.transport.remove_peer(peer_id).await?;
        }

        debug!("Simulated network partition");
        Ok(())
    }

    /// Simulate network healing (reconnect to specified peers)
    pub async fn simulate_healing(&self, peers: &[PeerId]) -> BitchatResult<()> {
        for &peer_id in peers {
            self.transport.add_peer(peer_id).await?;
        }

        debug!("Simulated network healing for {} peers", peers.len());
        Ok(())
    }
}

/// Represents a network packet for testing
#[derive(Debug, Clone)]
pub struct NetworkPacket {
    /// Source peer ID
    pub from: PeerId,
    /// Destination peer ID  
    pub to: PeerId,
    /// Packet payload
    pub payload: Vec<u8>,
    /// Timestamp when packet was created
    pub timestamp: Instant,
    /// Optional metadata
    pub metadata: PacketMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct PacketMetadata {
    /// Simulated signal strength
    pub signal_strength: Option<i32>,
    /// Packet sequence number
    pub sequence: Option<u64>,
    /// Whether this is a retransmission
    pub is_retry: bool,
    /// Transport-specific identifier
    pub transport_id: Option<String>,
}

impl NetworkPacket {
    /// Create a new network packet
    pub fn new(from: PeerId, to: PeerId, payload: Vec<u8>) -> Self {
        Self {
            from,
            to,
            payload,
            timestamp: Instant::now(),
            metadata: PacketMetadata::default(),
        }
    }

    /// Create a packet with metadata
    pub fn with_metadata(
        from: PeerId,
        to: PeerId,
        payload: Vec<u8>,
        metadata: PacketMetadata,
    ) -> Self {
        Self {
            from,
            to,
            payload,
            timestamp: Instant::now(),
            metadata,
        }
    }

    /// Get the age of this packet
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }

    /// Check if this packet matches expected criteria
    pub fn matches(
        &self,
        from: Option<PeerId>,
        to: Option<PeerId>,
        payload_prefix: Option<&[u8]>,
    ) -> bool {
        if let Some(expected_from) = from {
            if self.from != expected_from {
                return false;
            }
        }

        if let Some(expected_to) = to {
            if self.to != expected_to {
                return false;
            }
        }

        if let Some(prefix) = payload_prefix {
            if !self.payload.starts_with(prefix) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness_creation() {
        let harness = TestHarness::new().await;
        assert_eq!(harness.peer_id(), PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));

        // Clean shutdown
        harness.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_network_handle_peer_management() {
        let harness = TestHarness::new().await;
        let peer_id = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);

        // Add peer
        harness.network.add_peer(peer_id).await.unwrap();

        // Remove peer
        harness.network.remove_peer(peer_id).await.unwrap();

        harness.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_packet_creation_and_matching() {
        let from = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let to = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        let payload = b"test message".to_vec();

        let packet = NetworkPacket::new(from, to, payload);

        assert_eq!(packet.from, from);
        assert_eq!(packet.to, to);
        assert_eq!(packet.payload, b"test message");

        // Test matching
        assert!(packet.matches(Some(from), Some(to), Some(b"test")));
        assert!(!packet.matches(Some(to), Some(from), Some(b"test")));
        assert!(packet.matches(None, None, Some(b"test")));
        assert!(!packet.matches(None, None, Some(b"other")));
    }

    #[tokio::test]
    async fn test_network_partition_and_healing() {
        let harness = TestHarness::new().await;
        let peer1 = PeerId::new([1, 0, 0, 0, 0, 0, 0, 0]);
        let peer2 = PeerId::new([2, 0, 0, 0, 0, 0, 0, 0]);

        // Add peers
        harness.network.add_peer(peer1).await.unwrap();
        harness.network.add_peer(peer2).await.unwrap();

        // Simulate partition
        harness.network.simulate_partition().await.unwrap();

        // Heal network
        harness
            .network
            .simulate_healing(&[peer1, peer2])
            .await
            .unwrap();

        harness.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_harness_configurations() {
        // Test different network conditions
        let ideal = TestHarness::new().await;
        let lossy = TestHarness::lossy().await;
        let high_latency = TestHarness::high_latency().await;

        // All should be creatable
        assert_eq!(ideal.peer_id(), lossy.peer_id());
        assert_eq!(lossy.peer_id(), high_latency.peer_id());

        // Clean shutdown
        ideal.shutdown().await.unwrap();
        lossy.shutdown().await.unwrap();
        high_latency.shutdown().await.unwrap();
    }
}
