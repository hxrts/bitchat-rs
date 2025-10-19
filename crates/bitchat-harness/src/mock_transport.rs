//! Mock Transport for Testing
//!
//! Provides a deterministic mock transport implementation for testing without
//! hardware dependencies. Supports simulation of latency, packet loss, and
//! reconnection scenarios.

use alloc::vec::Vec;
use bitchat_core::{BitchatError, BitchatResult, PeerId};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    time::{sleep, Duration, Instant},
};

#[cfg(not(feature = "std"))]
use log::{debug, error, info};
#[cfg(feature = "std")]
use tracing::{debug, error, info};

// ----------------------------------------------------------------------------
// Mock Transport Configuration
// ----------------------------------------------------------------------------

/// Configuration for mock transport behavior
#[derive(Debug, Clone)]
pub struct MockTransportConfig {
    /// Simulated latency range (min, max) in milliseconds
    pub latency_range: (u64, u64),
    /// Packet loss rate (0.0 = no loss, 1.0 = all packets lost)
    pub packet_loss_rate: f64,
    /// Maximum packets in flight before backpressure
    pub max_in_flight: usize,
    /// Whether to simulate connection drops
    pub simulate_disconnects: bool,
    /// Disconnect probability per message (0.0 = never, 1.0 = always)
    pub disconnect_rate: f64,
    /// Whether to maintain message ordering
    pub preserve_order: bool,
    /// Transport capacity (messages per second)
    pub throughput_limit: Option<u64>,
    /// Jitter factor for latency variance (0.0 = no jitter, 1.0 = high jitter)
    pub jitter_factor: f64,
    /// Probability of packet duplication (0.0 = never, 1.0 = always)
    pub duplication_rate: f64,
    /// Probability of packet corruption (0.0 = never, 1.0 = always)
    pub corruption_rate: f64,
    /// Burst loss probability (simulate temporary network issues)
    pub burst_loss_rate: f64,
    /// Duration of burst loss events in milliseconds
    pub burst_loss_duration: u64,
    /// Bandwidth limit in bytes per second (None = unlimited)
    pub bandwidth_limit: Option<u64>,
    /// Maximum reorder distance for out-of-order delivery
    pub max_reorder_distance: usize,
}

impl Default for MockTransportConfig {
    fn default() -> Self {
        Self {
            latency_range: (10, 50), // 10-50ms latency
            packet_loss_rate: 0.0,   // No packet loss by default
            max_in_flight: 1000,     // High capacity
            simulate_disconnects: false,
            disconnect_rate: 0.0,
            preserve_order: true,
            throughput_limit: None,
            jitter_factor: 0.1,        // 10% jitter
            duplication_rate: 0.0,     // No duplication by default
            corruption_rate: 0.0,      // No corruption by default
            burst_loss_rate: 0.0,      // No burst loss by default
            burst_loss_duration: 1000, // 1 second burst duration
            bandwidth_limit: None,     // Unlimited bandwidth
            max_reorder_distance: 0,   // No reordering by default
        }
    }
}

impl MockTransportConfig {
    /// Create config for testing ideal network conditions
    pub fn ideal() -> Self {
        Self {
            latency_range: (1, 2),
            packet_loss_rate: 0.0,
            max_in_flight: 10000,
            simulate_disconnects: false,
            disconnect_rate: 0.0,
            preserve_order: true,
            throughput_limit: None,
            jitter_factor: 0.0,
            duplication_rate: 0.0,
            corruption_rate: 0.0,
            burst_loss_rate: 0.0,
            burst_loss_duration: 0,
            bandwidth_limit: None,
            max_reorder_distance: 0,
        }
    }

    /// Create config for testing lossy network conditions
    pub fn lossy() -> Self {
        Self {
            latency_range: (50, 200),
            packet_loss_rate: 0.1, // 10% packet loss
            max_in_flight: 100,
            simulate_disconnects: true,
            disconnect_rate: 0.01, // 1% chance per message
            preserve_order: false,
            throughput_limit: Some(100),      // 100 messages/sec
            jitter_factor: 0.3,               // 30% jitter
            duplication_rate: 0.02,           // 2% duplication
            corruption_rate: 0.001,           // 0.1% corruption
            burst_loss_rate: 0.05,            // 5% burst loss
            burst_loss_duration: 2000,        // 2 second bursts
            bandwidth_limit: Some(1_000_000), // 1MB/s
            max_reorder_distance: 5,          // Allow up to 5 packets out of order
        }
    }

    /// Create config for testing high-latency conditions
    pub fn high_latency() -> Self {
        Self {
            latency_range: (500, 1000),
            packet_loss_rate: 0.05,
            max_in_flight: 50,
            simulate_disconnects: false,
            disconnect_rate: 0.0,
            preserve_order: true,
            throughput_limit: Some(10), // 10 messages/sec
            jitter_factor: 0.5,         // 50% jitter for satellite-like conditions
            duplication_rate: 0.0,
            corruption_rate: 0.0,
            burst_loss_rate: 0.0,
            burst_loss_duration: 0,
            bandwidth_limit: Some(100_000), // 100KB/s (slow connection)
            max_reorder_distance: 0,
        }
    }

    /// Create config for testing mobile network conditions
    pub fn mobile() -> Self {
        Self {
            latency_range: (100, 300),
            packet_loss_rate: 0.05, // 5% packet loss
            max_in_flight: 200,
            simulate_disconnects: true,
            disconnect_rate: 0.005, // 0.5% chance per message
            preserve_order: false,
            throughput_limit: Some(50),     // 50 messages/sec
            jitter_factor: 0.4,             // High jitter
            duplication_rate: 0.01,         // 1% duplication
            corruption_rate: 0.002,         // 0.2% corruption
            burst_loss_rate: 0.1,           // 10% burst loss (handoffs, etc.)
            burst_loss_duration: 5000,      // 5 second bursts
            bandwidth_limit: Some(500_000), // 500KB/s
            max_reorder_distance: 10,       // Mobile networks can reorder
        }
    }

    /// Create config for testing adversarial network conditions
    pub fn adversarial() -> Self {
        Self {
            latency_range: (10, 2000), // Highly variable latency
            packet_loss_rate: 0.2,     // 20% packet loss
            max_in_flight: 50,
            simulate_disconnects: true,
            disconnect_rate: 0.02, // 2% chance per message
            preserve_order: false,
            throughput_limit: Some(20),    // 20 messages/sec
            jitter_factor: 0.8,            // Very high jitter
            duplication_rate: 0.1,         // 10% duplication
            corruption_rate: 0.05,         // 5% corruption
            burst_loss_rate: 0.2,          // 20% burst loss
            burst_loss_duration: 10000,    // 10 second bursts
            bandwidth_limit: Some(50_000), // 50KB/s
            max_reorder_distance: 20,      // Heavy reordering
        }
    }
}

// ----------------------------------------------------------------------------
// Mock Transport Implementation
// ----------------------------------------------------------------------------

/// Mock transport for deterministic testing
pub struct MockTransport {
    /// Transport identity
    peer_id: PeerId,

    /// Configuration
    config: MockTransportConfig,

    /// Network simulation
    message_queue: Arc<Mutex<VecDeque<PendingMessage>>>,
    pub connected_peers: Arc<RwLock<HashMap<PeerId, MockPeer>>>,

    /// Statistics
    stats: Arc<MockTransportStats>,

    /// Enhanced network simulation state
    burst_loss_state: Arc<Mutex<BurstLossState>>,
    bandwidth_tracker: Arc<Mutex<BandwidthTracker>>,
    reorder_buffer: Arc<Mutex<VecDeque<PendingMessage>>>,

    /// Control channels
    _control_sender: Option<mpsc::UnboundedSender<MockControl>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug)]
struct PendingMessage {
    recipient: PeerId,
    _sender: PeerId,
    _content: Vec<u8>,
    send_time: Instant,
    delivery_time: Instant,
    _sequence: u64,
    _is_duplicate: bool,
    _is_corrupted: bool,
    _original_size: usize,
}

#[derive(Debug)]
struct BurstLossState {
    in_burst: bool,
    burst_start: Option<Instant>,
    next_burst_check: Instant,
}

#[derive(Debug)]
struct BandwidthTracker {
    _bytes_sent_this_second: u64,
    _current_second: u64,
    _total_bytes_sent: u64,
}

#[derive(Debug, Clone)]
pub struct MockPeer {
    pub peer_id: PeerId,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub message_count: u64,
}

#[derive(Debug, Default)]
pub struct MockTransportStats {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub messages_dropped: AtomicU64,
    pub connections_established: AtomicU64,
    pub connections_lost: AtomicU64,
    pub total_latency_ms: AtomicU64,
    pub max_latency_ms: AtomicU64,
}

#[derive(Debug)]
#[allow(dead_code)]
enum MockControl {
    AddPeer(PeerId),
    RemovePeer(PeerId),
    SimulateDisconnect(PeerId),
    ChangeConfig(MockTransportConfig),
    GetStats,
}

impl Clone for MockTransport {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id,
            config: self.config.clone(),
            message_queue: Arc::clone(&self.message_queue),
            connected_peers: Arc::clone(&self.connected_peers),
            stats: Arc::clone(&self.stats),
            burst_loss_state: Arc::clone(&self.burst_loss_state),
            bandwidth_tracker: Arc::clone(&self.bandwidth_tracker),
            reorder_buffer: Arc::clone(&self.reorder_buffer),
            _control_sender: None, // Clone doesn't preserve control channels
            running: Arc::clone(&self.running),
        }
    }
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new(peer_id: PeerId, config: MockTransportConfig) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            config,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(MockTransportStats::default()),
            burst_loss_state: Arc::new(Mutex::new(BurstLossState {
                in_burst: false,
                burst_start: None,
                next_burst_check: now,
            })),
            bandwidth_tracker: Arc::new(Mutex::new(BandwidthTracker {
                _bytes_sent_this_second: 0,
                _current_second: now.elapsed().as_secs(),
                _total_bytes_sent: 0,
            })),
            reorder_buffer: Arc::new(Mutex::new(VecDeque::new())),
            _control_sender: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Create a mock transport with ideal network conditions
    pub fn ideal(peer_id: PeerId) -> Self {
        Self::new(peer_id, MockTransportConfig::ideal())
    }

    /// Create a mock transport with lossy network conditions
    pub fn lossy(peer_id: PeerId) -> Self {
        Self::new(peer_id, MockTransportConfig::lossy())
    }

    /// Create a mock transport with high latency
    pub fn high_latency(peer_id: PeerId) -> Self {
        Self::new(peer_id, MockTransportConfig::high_latency())
    }

    /// Add a peer to the network
    pub async fn add_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        let mut peers = self.connected_peers.write().await;
        let now = Instant::now();

        peers.insert(
            peer_id,
            MockPeer {
                peer_id,
                connected_at: now,
                last_seen: now,
                message_count: 0,
            },
        );

        self.stats
            .connections_established
            .fetch_add(1, Ordering::Relaxed);

        debug!("Mock transport: Added peer {:?}", peer_id);
        Ok(())
    }

    /// Remove a peer from the network
    pub async fn remove_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        let mut peers = self.connected_peers.write().await;

        if peers.remove(&peer_id).is_some() {
            self.stats.connections_lost.fetch_add(1, Ordering::Relaxed);
            debug!("Mock transport: Removed peer {:?}", peer_id);
        }

        Ok(())
    }

    /// Simulate network delivery of pending messages
    async fn process_message_queue(&self) -> BitchatResult<()> {
        let mut queue = self.message_queue.lock().await;
        let now = Instant::now();

        // Process messages that are ready for delivery
        while let Some(msg) = queue.front() {
            if now >= msg.delivery_time {
                let msg = queue.pop_front().unwrap();

                // Check if recipient is still connected
                let peers = self.connected_peers.read().await;
                if !peers.contains_key(&msg.recipient) {
                    self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                drop(peers);

                // Simulate packet loss
                if self.should_drop_packet().await {
                    self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
                    debug!("Mock transport: Dropped message due to packet loss");
                    continue;
                }

                // Simulate message delivery (simplified)
                self.stats.messages_received.fetch_add(1, Ordering::Relaxed);

                // Update latency stats
                let latency_ms = now.duration_since(msg.send_time).as_millis() as u64;
                self.stats
                    .total_latency_ms
                    .fetch_add(latency_ms, Ordering::Relaxed);

                let max_latency = self.stats.max_latency_ms.load(Ordering::Relaxed);
                if latency_ms > max_latency {
                    self.stats
                        .max_latency_ms
                        .store(latency_ms, Ordering::Relaxed);
                }

                debug!(
                    "Mock transport: Delivered message with {}ms latency",
                    latency_ms
                );
            } else {
                break; // Queue is ordered by delivery time
            }
        }

        Ok(())
    }

    /// Send a message through the mock network
    #[allow(dead_code)]
    async fn send_message(&self, recipient: PeerId, mut content: Vec<u8>) -> BitchatResult<()> {
        // Check if recipient is connected
        let peers = self.connected_peers.read().await;
        if !peers.contains_key(&recipient) {
            return Err(BitchatError::Channel {
                message: format!("Peer not found: {:?}", recipient),
            });
        }
        drop(peers);

        let original_size = content.len();

        // Check bandwidth limit
        if !self.check_bandwidth_limit(original_size).await {
            debug!("Mock transport: Message dropped due to bandwidth limit");
            self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }

        // Simulate throughput limiting
        if let Some(limit) = self.config.throughput_limit {
            let messages_sent = self.stats.messages_sent.load(Ordering::Relaxed);
            let should_throttle = messages_sent % limit == 0 && messages_sent > 0;
            if should_throttle {
                sleep(Duration::from_millis(1000 / limit)).await;
            }
        }

        // Calculate delivery time based on simulated latency
        let latency_ms = self.calculate_latency();
        let now = Instant::now();
        let delivery_time = now + Duration::from_millis(latency_ms);

        let sequence = self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);

        // Simulate corruption
        let is_corrupted = self.should_corrupt();
        if is_corrupted {
            content = self.corrupt_payload(content);
            debug!("Mock transport: Corrupted message payload");
        }

        let create_message = |is_duplicate: bool| PendingMessage {
            recipient,
            _sender: self.peer_id,
            _content: content.clone(),
            send_time: now,
            delivery_time,
            _sequence: sequence,
            _is_duplicate: is_duplicate,
            _is_corrupted: is_corrupted,
            _original_size: original_size,
        };

        let pending_msg = create_message(false);

        // Add to queue
        let mut queue = self.message_queue.lock().await;

        // Handle reordering
        if self.config.max_reorder_distance > 0 && !self.config.preserve_order {
            let mut reorder_buffer = self.reorder_buffer.lock().await;
            reorder_buffer.push_back(pending_msg);

            // Randomly release messages from reorder buffer
            while reorder_buffer.len() > self.config.max_reorder_distance
                || (!reorder_buffer.is_empty() && fastrand::f64() < 0.3)
            {
                if let Some(msg) = reorder_buffer.pop_front() {
                    queue.push_back(msg);
                }
            }
        } else if self.config.preserve_order {
            queue.push_back(pending_msg);
        } else {
            // Insert at random position for unordered delivery
            let pos = fastrand::usize(..=queue.len());
            queue.insert(pos, pending_msg);
        }

        // Simulate duplication
        if self.should_duplicate() {
            let duplicate_msg = create_message(true);
            // Add duplicate with slight delay
            let mut dup_delivery = delivery_time;
            dup_delivery += Duration::from_millis(fastrand::u64(10..=100));
            let mut dup_msg = duplicate_msg;
            dup_msg.delivery_time = dup_delivery;

            if self.config.preserve_order {
                queue.push_back(dup_msg);
            } else {
                let pos = fastrand::usize(..=queue.len());
                queue.insert(pos, dup_msg);
            }
            debug!("Mock transport: Duplicated message");
        }

        // Check capacity
        if queue.len() > self.config.max_in_flight {
            queue.pop_front(); // Drop oldest message
            self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
        }

        debug!(
            "Mock transport: Queued message to {:?} with {}ms latency",
            recipient, latency_ms
        );
        Ok(())
    }

    #[allow(dead_code)]
    fn calculate_latency(&self) -> u64 {
        let (min, max) = self.config.latency_range;
        let base_latency = fastrand::u64(min..=max);

        // Apply jitter
        if self.config.jitter_factor > 0.0 {
            let jitter_amount = (base_latency as f64 * self.config.jitter_factor) as u64;
            let jitter = fastrand::u64(0..=jitter_amount * 2);
            base_latency
                .saturating_add(jitter)
                .saturating_sub(jitter_amount)
        } else {
            base_latency
        }
    }

    async fn should_drop_packet(&self) -> bool {
        // Check for burst loss first
        let mut burst_state = self.burst_loss_state.lock().await;
        let now = Instant::now();

        // Check if we should start a burst
        if !burst_state.in_burst && now >= burst_state.next_burst_check {
            if fastrand::f64() < self.config.burst_loss_rate {
                burst_state.in_burst = true;
                burst_state.burst_start = Some(now);
                debug!("Started burst loss event");
            }
            // Schedule next burst check
            burst_state.next_burst_check =
                now + Duration::from_millis(self.config.burst_loss_duration * 2);
        }

        // Check if we should end a burst
        if burst_state.in_burst {
            if let Some(start) = burst_state.burst_start {
                if now.duration_since(start).as_millis() as u64 >= self.config.burst_loss_duration {
                    burst_state.in_burst = false;
                    burst_state.burst_start = None;
                    debug!("Ended burst loss event");
                }
            }
        }

        // If in burst, drop all packets
        if burst_state.in_burst {
            return true;
        }

        drop(burst_state);

        // Normal packet loss
        fastrand::f64() < self.config.packet_loss_rate
    }

    #[allow(dead_code)]
    fn should_duplicate(&self) -> bool {
        fastrand::f64() < self.config.duplication_rate
    }

    #[allow(dead_code)]
    fn should_corrupt(&self) -> bool {
        fastrand::f64() < self.config.corruption_rate
    }

    #[allow(dead_code)]
    fn corrupt_payload(&self, mut payload: Vec<u8>) -> Vec<u8> {
        if payload.is_empty() {
            return payload;
        }

        // Corrupt 1-3 random bytes
        let corruption_count = fastrand::usize(1..=3.min(payload.len()));
        for _ in 0..corruption_count {
            let index = fastrand::usize(..payload.len());
            payload[index] = fastrand::u8(..);
        }

        payload
    }

    #[allow(dead_code)]
    async fn check_bandwidth_limit(&self, message_size: usize) -> bool {
        if let Some(limit) = self.config.bandwidth_limit {
            let mut tracker = self.bandwidth_tracker.lock().await;
            let now_sec = Instant::now().elapsed().as_secs();

            // Reset counter if we're in a new second
            if now_sec != tracker._current_second {
                tracker._current_second = now_sec;
                tracker._bytes_sent_this_second = 0;
            }

            // Check if adding this message would exceed the limit
            if tracker._bytes_sent_this_second + message_size as u64 > limit {
                return false; // Would exceed bandwidth limit
            }

            tracker._bytes_sent_this_second += message_size as u64;
            tracker._total_bytes_sent += message_size as u64;
        }

        true
    }

    #[allow(dead_code)]
    fn should_disconnect(&self) -> bool {
        self.config.simulate_disconnects && fastrand::f64() < self.config.disconnect_rate
    }

    /// Get transport statistics
    pub fn get_stats(&self) -> MockTransportStats {
        MockTransportStats {
            messages_sent: AtomicU64::new(self.stats.messages_sent.load(Ordering::Relaxed)),
            messages_received: AtomicU64::new(self.stats.messages_received.load(Ordering::Relaxed)),
            messages_dropped: AtomicU64::new(self.stats.messages_dropped.load(Ordering::Relaxed)),
            connections_established: AtomicU64::new(
                self.stats.connections_established.load(Ordering::Relaxed),
            ),
            connections_lost: AtomicU64::new(self.stats.connections_lost.load(Ordering::Relaxed)),
            total_latency_ms: AtomicU64::new(self.stats.total_latency_ms.load(Ordering::Relaxed)),
            max_latency_ms: AtomicU64::new(self.stats.max_latency_ms.load(Ordering::Relaxed)),
        }
    }

    /// Get average latency in milliseconds
    pub fn average_latency_ms(&self) -> f64 {
        let total = self.stats.total_latency_ms.load(Ordering::Relaxed) as f64;
        let count = self.stats.messages_received.load(Ordering::Relaxed) as f64;
        if count > 0.0 {
            total / count
        } else {
            0.0
        }
    }
}

// Simplified implementation without complex transport dependencies
impl MockTransport {
    /// Start the mock transport (simplified)
    pub async fn start(&mut self) -> BitchatResult<()> {
        info!("Mock transport starting for peer {:?}", self.peer_id);
        self.running.store(true, Ordering::Relaxed);

        // Main message processing loop
        let mut message_interval = tokio::time::interval(Duration::from_millis(10));

        while self.running.load(Ordering::Relaxed) {
            tokio::select! {
                // Process pending messages
                _ = message_interval.tick() => {
                    if let Err(e) = self.process_message_queue().await {
                        error!("Error processing message queue: {:?}", e);
                    }
                }

                else => {
                    debug!("Mock transport stopped");
                    break;
                }
            }
        }

        info!("Mock transport stopped");
        Ok(())
    }

    /// Stop the mock transport
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        debug!("Mock transport shutdown requested");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_transport_creation() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let transport = MockTransport::ideal(peer_id);

        assert_eq!(transport.peer_id, peer_id);
    }

    #[tokio::test]
    async fn test_peer_management() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let other_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        let transport = MockTransport::ideal(peer_id);

        // Add peer
        transport.add_peer(other_peer).await.unwrap();
        let peers = transport.connected_peers.read().await;
        assert!(peers.contains_key(&other_peer));
        drop(peers);

        // Remove peer
        transport.remove_peer(other_peer).await.unwrap();
        let peers = transport.connected_peers.read().await;
        assert!(!peers.contains_key(&other_peer));
    }

    #[tokio::test]
    async fn test_configuration_presets() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        let ideal = MockTransport::ideal(peer_id);
        assert_eq!(ideal.config.packet_loss_rate, 0.0);
        assert_eq!(ideal.config.latency_range, (1, 2));

        let lossy = MockTransport::lossy(peer_id);
        assert_eq!(lossy.config.packet_loss_rate, 0.1);
        assert!(lossy.config.simulate_disconnects);

        let high_latency = MockTransport::high_latency(peer_id);
        assert_eq!(high_latency.config.latency_range, (500, 1000));
        assert_eq!(high_latency.config.throughput_limit, Some(10));
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let transport = MockTransport::ideal(peer_id);

        let stats = transport.get_stats();
        assert_eq!(stats.messages_sent.load(Ordering::Relaxed), 0);
        assert_eq!(stats.messages_received.load(Ordering::Relaxed), 0);

        let avg_latency = transport.average_latency_ms();
        assert_eq!(avg_latency, 0.0);
    }
}
