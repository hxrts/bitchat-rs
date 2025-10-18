//! Test utilities for deterministic testing of BitChat protocol
//!
//! This module provides mock implementations and utilities for testing
//! BitChat components in a deterministic, controllable environment.

use async_trait::async_trait;
use bitchat_core::transport::{Transport, TransportCapabilities, TransportType};
use bitchat_core::types::{PeerId, TimeSource, Timestamp};
use bitchat_core::{BitchatError, BitchatPacket, Result as BitchatResult};
use rand_chacha::ChaCha8Rng;
use rand_core::{CryptoRng, RngCore, SeedableRng};
use smallvec::SmallVec;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

// ----------------------------------------------------------------------------
// Mock Time Source
// ----------------------------------------------------------------------------

/// Mock time source for deterministic testing
///
/// This allows tests to control the flow of time precisely, making
/// time-dependent tests deterministic and fast.
#[derive(Debug, Clone)]
pub struct MockTimeSource {
    current_time: Arc<AtomicU64>,
}

impl MockTimeSource {
    /// Create a new mock time source starting at time 0
    pub fn new() -> Self {
        Self {
            current_time: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new mock time source starting at a specific time
    #[allow(dead_code)]
    pub fn new_at(start_time: u64) -> Self {
        Self {
            current_time: Arc::new(AtomicU64::new(start_time)),
        }
    }

    /// Advance time by the specified number of milliseconds
    pub fn advance(&self, millis: u64) {
        self.current_time.fetch_add(millis, Ordering::SeqCst);
    }

    /// Set the time to a specific value
    pub fn set_time(&self, millis: u64) {
        self.current_time.store(millis, Ordering::SeqCst);
    }

    /// Get the current mock time
    pub fn current_time(&self) -> u64 {
        self.current_time.load(Ordering::SeqCst)
    }
}

impl Default for MockTimeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeSource for MockTimeSource {
    fn now(&self) -> Timestamp {
        Timestamp::new(self.current_time.load(Ordering::SeqCst))
    }
}

// ----------------------------------------------------------------------------
// Deterministic RNG
// ----------------------------------------------------------------------------

/// Deterministic RNG for testing cryptographic operations
///
/// This provides a seeded pseudo-random number generator that produces
/// the same sequence of values across test runs, making crypto tests deterministic.
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    rng: ChaCha8Rng,
}

impl DeterministicRng {
    /// Generate a random f64 between 0.0 and 1.0
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
    /// Create a new deterministic RNG with a fixed seed
    pub fn new() -> Self {
        Self {
            rng: ChaCha8Rng::from_seed([42u8; 32]), // Fixed seed for determinism
        }
    }

    /// Create a new deterministic RNG with a custom seed
    #[allow(dead_code)]
    pub fn with_seed(seed: [u8; 32]) -> Self {
        Self {
            rng: ChaCha8Rng::from_seed(seed),
        }
    }
}

impl Default for DeterministicRng {
    fn default() -> Self {
        Self::new()
    }
}

impl RngCore for DeterministicRng {
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.rng.try_fill_bytes(dest)
    }
}

impl CryptoRng for DeterministicRng {}

// ----------------------------------------------------------------------------
// Mock Network for Transport Testing
// ----------------------------------------------------------------------------

/// Simulated network packet with delivery parameters
#[derive(Debug, Clone)]
pub struct SimulatedPacket {
    pub from: PeerId,
    pub to: Option<PeerId>, // None for broadcast
    pub packet: BitchatPacket,
    pub delivery_time: u64,
    pub should_drop: bool,
}

/// Configuration for network simulation
#[derive(Debug, Clone)]
pub struct NetworkSimConfig {
    /// Packet loss rate (0.0 = no loss, 1.0 = all packets lost)
    pub packet_loss_rate: f64,
    /// Maximum random delay in milliseconds
    pub max_delay: u64,
    /// Whether to reorder packets
    pub enable_reordering: bool,
    /// Maximum packets that can be reordered
    pub max_reorder_distance: usize,
}

impl Default for NetworkSimConfig {
    fn default() -> Self {
        Self {
            packet_loss_rate: 0.0,
            max_delay: 0,
            enable_reordering: false,
            max_reorder_distance: 0,
        }
    }
}

/// Mock network for simulating real network conditions in tests
#[derive(Debug)]
pub struct MockNetwork {
    /// Configuration for network simulation
    config: NetworkSimConfig,
    /// Time source for scheduling packet delivery
    time_source: MockTimeSource,
    /// RNG for simulating network behavior
    rng: DeterministicRng,
    /// Queue of packets waiting for delivery
    packet_queue: Arc<Mutex<VecDeque<SimulatedPacket>>>,
    /// Connected transports
    transports: Arc<Mutex<Vec<Arc<Mutex<MockTransport>>>>>,
}

impl MockNetwork {
    /// Create a new mock network
    pub fn new(time_source: MockTimeSource) -> Self {
        Self {
            config: NetworkSimConfig::default(),
            time_source,
            rng: DeterministicRng::new(),
            packet_queue: Arc::new(Mutex::new(VecDeque::new())),
            transports: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Configure network simulation parameters
    pub fn configure(&mut self, config: NetworkSimConfig) {
        self.config = config;
    }

    /// Add a transport to the network
    pub async fn add_transport(&self, transport: Arc<Mutex<MockTransport>>) {
        let mut transports = self.transports.lock().await;
        transports.push(transport);
    }

    /// Send a packet through the network
    pub async fn send_packet(&mut self, from: PeerId, to: Option<PeerId>, packet: BitchatPacket) {
        // Simulate packet loss
        if self.rng.next_f64() < self.config.packet_loss_rate {
            return; // Packet lost
        }

        // Calculate delivery time with random delay
        let delay = if self.config.max_delay > 0 {
            (self.rng.next_u64() % self.config.max_delay) + 1
        } else {
            0
        };

        let delivery_time = self.time_source.current_time() + delay;

        let simulated_packet = SimulatedPacket {
            from,
            to,
            packet,
            delivery_time,
            should_drop: false,
        };

        let mut queue = self.packet_queue.lock().await;

        if self.config.enable_reordering && queue.len() < self.config.max_reorder_distance {
            // Insert packet at random position for reordering
            let pos = if queue.is_empty() {
                0
            } else {
                (self.rng.next_u64() as usize) % (queue.len() + 1)
            };
            queue.insert(pos, simulated_packet);
        } else {
            queue.push_back(simulated_packet);
        }
    }

    /// Process packets ready for delivery at current time
    pub async fn tick(&self) -> BitchatResult<()> {
        let current_time = self.time_source.current_time();
        let mut queue = self.packet_queue.lock().await;
        let transports = self.transports.lock().await;

        // Find packets ready for delivery
        let mut ready_packets = Vec::new();
        while let Some(packet) = queue.front() {
            if packet.delivery_time <= current_time {
                ready_packets.push(queue.pop_front().unwrap());
            } else {
                break;
            }
        }

        drop(queue);
        drop(transports);

        // Deliver ready packets
        for sim_packet in ready_packets {
            if sim_packet.should_drop {
                continue;
            }

            // Deliver to appropriate transports
            let transports = self.transports.lock().await;
            for transport in transports.iter() {
                let transport = transport.lock().await;

                // Check if this transport should receive the packet
                if let Some(recipient) = sim_packet.to {
                    if transport.local_peer_id == recipient {
                        transport
                            .deliver_packet(sim_packet.from, sim_packet.packet.clone())
                            .await?;
                    }
                } else {
                    // Broadcast packet - deliver to all transports except sender
                    if transport.local_peer_id != sim_packet.from {
                        transport
                            .deliver_packet(sim_packet.from, sim_packet.packet.clone())
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the number of packets in the queue
    #[allow(dead_code)]
    pub async fn queue_size(&self) -> usize {
        self.packet_queue.lock().await.len()
    }
}

// ----------------------------------------------------------------------------
// Mock Transport
// ----------------------------------------------------------------------------

/// Mock transport implementation for testing
#[derive(Debug)]
pub struct MockTransport {
    /// Local peer ID for this transport
    pub local_peer_id: PeerId,
    /// Whether the transport is active
    is_active: bool,
    /// Discovered peers
    discovered_peers: Vec<PeerId>,
    /// Channel for receiving packets
    packet_receiver: Option<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>,
    /// Channel for sending packets (internal)
    packet_sender: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    /// Reference to the mock network
    network: Option<Arc<Mutex<MockNetwork>>>,
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new(local_peer_id: PeerId) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            local_peer_id,
            is_active: false,
            discovered_peers: Vec::new(),
            packet_receiver: Some(rx),
            packet_sender: tx,
            network: None,
        }
    }

    /// Connect this transport to a mock network
    pub fn with_network(mut self, network: Arc<Mutex<MockNetwork>>) -> Self {
        self.network = Some(network);
        self
    }

    /// Add a discovered peer
    #[allow(dead_code)]
    pub fn add_discovered_peer(&mut self, peer_id: PeerId) {
        if !self.discovered_peers.contains(&peer_id) {
            self.discovered_peers.push(peer_id);
        }
    }

    /// Deliver a packet to this transport (called by MockNetwork)
    pub async fn deliver_packet(&self, from: PeerId, packet: BitchatPacket) -> BitchatResult<()> {
        self.packet_sender
            .send((from, packet))
            .map_err(|_| BitchatError::Transport {
                message: "Failed to deliver packet to transport".to_string(),
            })?;
        Ok(())
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> BitchatResult<()> {
        if !self.is_active {
            return Err(BitchatError::Transport {
                message: "Transport not active".to_string(),
            });
        }

        if let Some(network) = &self.network {
            let mut network = network.lock().await;
            network
                .send_packet(self.local_peer_id, Some(peer_id), packet)
                .await;
        }

        Ok(())
    }

    async fn broadcast(&mut self, packet: BitchatPacket) -> BitchatResult<()> {
        if !self.is_active {
            return Err(BitchatError::Transport {
                message: "Transport not active".to_string(),
            });
        }

        if let Some(network) = &self.network {
            let mut network = network.lock().await;
            network.send_packet(self.local_peer_id, None, packet).await;
        }

        Ok(())
    }

    async fn receive(&mut self) -> BitchatResult<(PeerId, BitchatPacket)> {
        if let Some(ref mut receiver) = self.packet_receiver {
            receiver
                .recv()
                .await
                .ok_or_else(|| BitchatError::Transport {
                    message: "Receive channel closed".to_string(),
                })
        } else {
            Err(BitchatError::Transport {
                message: "No packet receiver available".to_string(),
            })
        }
    }

    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        SmallVec::from_vec(self.discovered_peers.clone())
    }

    async fn start(&mut self) -> BitchatResult<()> {
        self.is_active = true;
        Ok(())
    }

    async fn stop(&mut self) -> BitchatResult<()> {
        self.is_active = false;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            transport_type: TransportType::Mock,
            max_packet_size: 1024,
            supports_discovery: true,
            supports_broadcast: true,
            requires_internet: false,
            latency_class: bitchat_core::transport::LatencyClass::Low,
            reliability_class: bitchat_core::transport::ReliabilityClass::High,
        }
    }
}

// ----------------------------------------------------------------------------
// Test Helper Functions
// ----------------------------------------------------------------------------

/// Create a deterministic test environment with mock time and network
pub fn create_test_environment() -> (MockTimeSource, Arc<Mutex<MockNetwork>>) {
    let time_source = MockTimeSource::new();
    let network = Arc::new(Mutex::new(MockNetwork::new(time_source.clone())));
    (time_source, network)
}

/// Create a test peer with a deterministic ID
pub fn create_test_peer(index: u8) -> PeerId {
    let mut bytes = [0u8; 8];
    bytes[0] = index;
    PeerId::new(bytes)
}

/// Create multiple test peers
pub fn create_test_peers(count: u8) -> Vec<PeerId> {
    (0..count).map(create_test_peer).collect()
}
