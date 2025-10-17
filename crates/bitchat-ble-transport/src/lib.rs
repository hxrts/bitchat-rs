//! Bluetooth Low Energy transport implementation for BitChat
//!
//! This crate provides a BLE transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Bluetooth Low Energy.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bitchat_core::transport::{
    LatencyClass, ReliabilityClass, Transport, TransportCapabilities, TransportType,
};
use bitchat_core::{BitchatError, BitchatPacket, PeerId, Result as BitchatResult};
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::StreamExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{timeout, interval};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ----------------------------------------------------------------------------
// BLE Service and Characteristic UUIDs
// ----------------------------------------------------------------------------

/// BitChat BLE service UUID
pub const BITCHAT_SERVICE_UUID: Uuid = Uuid::from_u128(0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E);

/// BitChat BLE characteristic for sending data
pub const BITCHAT_TX_CHARACTERISTIC_UUID: Uuid =
    Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);

/// BitChat BLE characteristic for receiving data
pub const BITCHAT_RX_CHARACTERISTIC_UUID: Uuid =
    Uuid::from_u128(0x6E400003_B5A3_F393_E0A9_E50E24DCCA9E);

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

/// Configuration for BLE transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BleTransportConfig {
    /// Maximum time to wait for scanning
    pub scan_timeout: Duration,
    /// Maximum time to wait for connection
    pub connection_timeout: Duration,
    /// Maximum packet size (BLE has limitations)
    pub max_packet_size: usize,
    /// Device name prefix for peer discovery
    pub device_name_prefix: String,
    /// Whether to automatically reconnect on disconnection
    pub auto_reconnect: bool,
}

impl Default for BleTransportConfig {
    fn default() -> Self {
        Self {
            scan_timeout: Duration::from_secs(10),
            connection_timeout: Duration::from_secs(5),
            max_packet_size: 500, // Conservative limit for BLE
            device_name_prefix: "BitChat".to_string(),
            auto_reconnect: true,
        }
    }
}

// ----------------------------------------------------------------------------
// BLE Peer Information
// ----------------------------------------------------------------------------

/// Connection state for a BLE peer
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

/// Information about a discovered BLE peer
#[derive(Debug, Clone)]
pub struct BlePeer {
    /// BitChat peer ID
    pub peer_id: PeerId,
    /// BLE peripheral
    pub peripheral: Peripheral,
    /// Device name
    pub device_name: String,
    /// Connection state
    pub connection_state: ConnectionState,
    /// Last connection attempt timestamp
    pub last_connection_attempt: Option<std::time::Instant>,
    /// Connection retry count
    pub retry_count: u32,
}

impl BlePeer {
    /// Create a new BLE peer
    pub fn new(peer_id: PeerId, peripheral: Peripheral, device_name: String) -> Self {
        Self {
            peer_id,
            peripheral,
            device_name,
            connection_state: ConnectionState::Disconnected,
            last_connection_attempt: None,
            retry_count: 0,
        }
    }
    
    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.connection_state == ConnectionState::Connected
    }
    
    /// Check if peer can retry connection
    pub fn can_retry(&self) -> bool {
        self.retry_count < 3 && 
        self.last_connection_attempt
            .map(|t| t.elapsed() > Duration::from_secs(self.retry_count as u64 * 5))
            .unwrap_or(true)
    }
}

// ----------------------------------------------------------------------------
// BLE Transport Implementation
// ----------------------------------------------------------------------------

/// BLE transport for BitChat communication
pub struct BleTransport {
    /// Transport configuration
    config: BleTransportConfig,
    /// BLE adapter
    adapter: Option<Adapter>,
    /// Discovered peers
    peers: Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    /// Receiver for incoming packets
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>>,
    /// Sender for incoming packets (used internally)
    packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    /// Whether the transport is active
    active: Arc<RwLock<bool>>,
    /// Our own peer ID for identification
    local_peer_id: PeerId,
    /// Background task handles
    task_handles: Vec<JoinHandle<()>>,
    /// Cached discovered peers (non-blocking access)
    cached_peers: Arc<RwLock<Vec<PeerId>>>,
}

impl BleTransport {
    /// Create a new BLE transport
    pub fn new(local_peer_id: PeerId) -> Self {
        Self::with_config(local_peer_id, BleTransportConfig::default())
    }

    /// Create a new BLE transport with custom configuration
    pub fn with_config(local_peer_id: PeerId, config: BleTransportConfig) -> Self {
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();

        Self {
            config,
            adapter: None,
            peers: Arc::new(RwLock::new(HashMap::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            packet_tx,
            active: Arc::new(RwLock::new(false)),
            local_peer_id,
            task_handles: Vec::new(),
            cached_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize BLE adapter
    async fn initialize_adapter(&mut self) -> BitchatResult<()> {
        let manager = Manager::new().await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create BLE manager: {}", e))
        })?;

        let adapters = manager.adapters().await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get BLE adapters: {}", e))
        })?;

        if adapters.is_empty() {
            return Err(BitchatError::InvalidPacket(
                "No BLE adapters available".into(),
            ));
        }

        self.adapter = Some(adapters[0].clone());
        info!("BLE adapter initialized");
        Ok(())
    }

    /// Start scanning for BitChat peers
    async fn start_scanning(&self) -> BitchatResult<()> {
        let adapter = self
            .adapter
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("BLE adapter not initialized".into()))?;

        let scan_filter = ScanFilter {
            services: vec![BITCHAT_SERVICE_UUID],
        };

        adapter
            .start_scan(scan_filter)
            .await
            .map_err(|e| BitchatError::InvalidPacket(format!("Failed to start BLE scan: {}", e)))?;

        info!("Started BLE scanning for BitChat peers");
        Ok(())
    }

    /// Stop scanning for peers
    async fn stop_scanning(&self) -> BitchatResult<()> {
        if let Some(adapter) = &self.adapter {
            adapter.stop_scan().await.map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to stop BLE scan: {}", e))
            })?;
        }
        Ok(())
    }

    /// Process discovery events from BLE adapter
    async fn process_discovery_event(&self, event: CentralEvent) -> BitchatResult<()> {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if let Some(adapter) = &self.adapter {
                    if let Ok(peripheral) = adapter.peripheral(&id).await {
                        if let Ok(Some(properties)) = peripheral.properties().await {
                            if let Some(name) = &properties.local_name {
                                if name.starts_with(&self.config.device_name_prefix) {
                                    if let Some(peer_id) = self.extract_peer_id_from_name(name) {
                                        let ble_peer = BlePeer::new(peer_id, peripheral, name.clone());
                                        
                                        let mut peers = self.peers.write().await;
                                        if !peers.contains_key(&peer_id) {
                                            debug!("Discovered new BitChat peer: {} ({})", peer_id, name);
                                            peers.insert(peer_id, ble_peer);
                                            
                                            // Update cached peers list
                                            let mut cached = self.cached_peers.write().await;
                                            cached.push(peer_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            CentralEvent::DeviceDisconnected(id) => {
                // Find peer by peripheral ID and mark as disconnected
                let mut peers = self.peers.write().await;
                for peer in peers.values_mut() {
                    if peer.peripheral.id() == id {
                        peer.connection_state = ConnectionState::Disconnected;
                        debug!("Peer {} disconnected", peer.peer_id);
                        break;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    
    /// Background task for managing connections
    async fn connection_manager(&self) {
        let mut reconnect_interval = interval(Duration::from_secs(10));
        
        loop {
            reconnect_interval.tick().await;
            
            if !*self.active.read().await {
                break;
            }
            
            // Auto-reconnect to failed peers if enabled
            if self.config.auto_reconnect {
                let peer_ids: Vec<PeerId> = {
                    let peers = self.peers.read().await;
                    peers.iter()
                        .filter(|(_, peer)| {
                            peer.connection_state == ConnectionState::Disconnected ||
                            peer.connection_state == ConnectionState::Failed
                        })
                        .filter(|(_, peer)| peer.can_retry())
                        .map(|(id, _)| *id)
                        .collect()
                };
                
                for peer_id in peer_ids {
                    if let Err(e) = self.connect_to_peer(&peer_id).await {
                        debug!("Auto-reconnect failed for peer {}: {}", peer_id, e);
                    }
                }
            }
        }
    }

    /// Extract peer ID from device name
    /// Format: "BitChat-<hex_peer_id>"
    #[allow(dead_code)]
    fn extract_peer_id_from_name(&self, name: &str) -> Option<PeerId> {
        if let Some(hex_part) = name.strip_prefix(&format!("{}-", self.config.device_name_prefix)) {
            if hex_part.len() == 16 {
                // 8 bytes = 16 hex chars
                if let Ok(bytes) = hex::decode(hex_part) {
                    if bytes.len() == 8 {
                        let mut peer_id_bytes = [0u8; 8];
                        peer_id_bytes.copy_from_slice(&bytes);
                        return Some(PeerId::new(peer_id_bytes));
                    }
                }
            }
        }
        None
    }

    /// Connect to a specific peer with proper state management
    async fn connect_to_peer(&self, peer_id: &PeerId) -> BitchatResult<()> {
        let mut peers = self.peers.write().await;
        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| BitchatError::InvalidPacket("Peer not found".into()))?;

        if peer.connection_state == ConnectionState::Connected {
            return Ok(());
        }
        
        if peer.connection_state == ConnectionState::Connecting {
            return Err(BitchatError::InvalidPacket("Connection in progress".into()));
        }
        
        if !peer.can_retry() {
            return Err(BitchatError::InvalidPacket("Too many failed connection attempts".into()));
        }

        peer.connection_state = ConnectionState::Connecting;
        peer.last_connection_attempt = Some(std::time::Instant::now());
        peer.retry_count += 1;

        let connect_result =
            timeout(self.config.connection_timeout, peer.peripheral.connect()).await;

        match connect_result {
            Ok(Ok(_)) => {
                peer.connection_state = ConnectionState::Connected;
                peer.retry_count = 0; // Reset on successful connection
                info!("Connected to peer: {}", peer_id);

                // Discover services and characteristics
                if let Err(e) = peer.peripheral.discover_services().await {
                    error!("Failed to discover services for peer {}: {}", peer_id, e);
                    peer.connection_state = ConnectionState::Failed;
                    return Err(BitchatError::InvalidPacket(format!("Failed to discover services: {}", e)));
                }

                // Start receiving data from this peer
                self.start_receiving_from_peer(peer_id, &peer.peripheral).await?;

                Ok(())
            }
            Ok(Err(e)) => {
                peer.connection_state = ConnectionState::Failed;
                error!("Failed to connect to peer {}: {}", peer_id, e);
                Err(BitchatError::InvalidPacket(format!("Connection failed: {}", e)))
            }
            Err(_) => {
                peer.connection_state = ConnectionState::Failed;
                error!("Connection to peer {} timed out", peer_id);
                Err(BitchatError::InvalidPacket("Connection timeout".into()))
            }
        }
    }
    
    /// Start receiving data from a connected peer
    async fn start_receiving_from_peer(&self, peer_id: &PeerId, peripheral: &Peripheral) -> BitchatResult<()> {
        // Find the RX characteristic
        let characteristics = peripheral.characteristics();
        let rx_char = characteristics
            .iter()
            .find(|c| c.uuid == BITCHAT_RX_CHARACTERISTIC_UUID)
            .ok_or_else(|| BitchatError::InvalidPacket("RX characteristic not found".into()))?;

        // Subscribe to notifications
        peripheral.subscribe(rx_char).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to subscribe to notifications: {}", e))
        })?;

        // Start notification handler
        let packet_tx = self.packet_tx.clone();
        let peer_id_copy = *peer_id;
        let mut notifications = peripheral.notifications().await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get notifications stream: {}", e))
        })?;

        tokio::spawn(async move {
            while let Some(data) = notifications.next().await {
                if data.uuid == BITCHAT_RX_CHARACTERISTIC_UUID {
                    // Deserialize packet
                    match bincode::deserialize::<BitchatPacket>(&data.value) {
                        Ok(packet) => {
                            if let Err(e) = packet_tx.send((peer_id_copy, packet)) {
                                error!("Failed to send received packet to channel: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to deserialize packet from peer {}: {}", peer_id_copy, e);
                        }
                    }
                }
            }
            debug!("Notification handler for peer {} ended", peer_id_copy);
        });

        Ok(())
    }

    /// Send data to a connected peer
    async fn send_to_peer(&self, peer_id: &PeerId, data: &[u8]) -> BitchatResult<()> {
        let peers = self.peers.read().await;
        let peer = peers
            .get(peer_id)
            .ok_or_else(|| BitchatError::InvalidPacket("Peer not found".into()))?;

        if !peer.is_connected() {
            return Err(BitchatError::InvalidPacket("Peer not connected".into()));
        }

        // Find the TX characteristic
        let characteristics = peer.peripheral.characteristics();
        let tx_char = characteristics
            .iter()
            .find(|c| c.uuid == BITCHAT_TX_CHARACTERISTIC_UUID)
            .ok_or_else(|| BitchatError::InvalidPacket("TX characteristic not found".into()))?;

        // Split data into chunks if necessary (BLE MTU limitations)
        const BLE_MTU: usize = 244; // Conservative MTU size
        let chunks: Vec<&[u8]> = data.chunks(BLE_MTU).collect();

        for chunk in chunks {
            peer.peripheral
                .write(tx_char, chunk, WriteType::WithoutResponse)
                .await
                .map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to write to characteristic: {}", e))
                })?;
        }

        debug!("Sent {} bytes to peer {}", data.len(), peer_id);
        Ok(())
    }

    /// Start advertising as a BitChat peer
    /// 
    /// LIMITATION: btleplug does not support peripheral mode (advertising).
    /// This significantly limits the usefulness of this transport as peers
    /// cannot discover this device. For production use, consider:
    /// 
    /// 1. Using platform-specific libraries (e.g., bluer for Linux)
    /// 2. Contributing peripheral mode support to btleplug
    /// 3. Implementing a bridge/relay that handles advertising
    async fn start_advertising(&self) -> BitchatResult<()> {
        warn!(
            "BLE advertising not supported by btleplug - this device won't be discoverable. \
            This is a critical limitation for peer-to-peer networking."
        );
        Ok(())
    }
}

impl Transport for BleTransport {
    fn send_to(
        &mut self,
        peer_id: PeerId,
        packet: BitchatPacket,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            // Check if peer exists and is connected
            {
                let peers = self.peers.read().await;
                let peer = peers.get(&peer_id)
                    .ok_or_else(|| BitchatError::InvalidPacket("Peer not discovered".into()))?;
                
                if !peer.is_connected() {
                    return Err(BitchatError::InvalidPacket("Peer not connected".into()));
                }
            }

            // Serialize packet
            let data = bincode::serialize(&packet).map_err(|e| BitchatError::Serialization(e))?;

            if data.len() > self.config.max_packet_size {
                return Err(BitchatError::InvalidPacket(
                    "Packet too large for BLE".into(),
                ));
            }

            self.send_to_peer(&peer_id, &data).await
        })
    }

    fn broadcast(
        &mut self,
        packet: BitchatPacket,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            // Get connected peers only
            let connected_peers: Vec<PeerId> = {
                let peers = self.peers.read().await;
                peers.iter()
                    .filter(|(_, peer)| peer.is_connected())
                    .map(|(id, _)| *id)
                    .collect()
            };
            
            if connected_peers.is_empty() {
                warn!("No connected peers for broadcast");
                return Ok(());
            }

            // Serialize packet once
            let data = bincode::serialize(&packet).map_err(|e| BitchatError::Serialization(e))?;

            if data.len() > self.config.max_packet_size {
                return Err(BitchatError::InvalidPacket(
                    "Packet too large for BLE broadcast".into(),
                ));
            }

            // Send to all connected peers sequentially
            // Note: Due to BLE limitations, concurrent sends might not work reliably
            for peer_id in connected_peers {
                if let Err(e) = self.send_to_peer(&peer_id, &data).await {
                    error!("Failed to broadcast to peer {}: {}", peer_id, e);
                }
            }

            Ok(())
        })
    }

    fn receive(
        &mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = BitchatResult<(PeerId, BitchatPacket)>> + Send + '_>,
    > {
        let packet_rx = Arc::clone(&self.packet_rx);

        Box::pin(async move {
            let mut rx = packet_rx.lock().await;
            rx.recv()
                .await
                .ok_or_else(|| BitchatError::InvalidPacket("Receive channel closed".into()))
        })
    }

    fn discovered_peers(&self) -> Vec<PeerId> {
        // Use cached peers list for non-blocking access
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.cached_peers.read().await.clone() })
        })
    }

    fn start(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            *self.active.write().await = true;

            // Initialize BLE adapter
            self.initialize_adapter().await?;

            // Start advertising (if supported)
            self.start_advertising().await?;

            // Start scanning for peers
            self.start_scanning().await?;

            // Start event-driven peer discovery
            if let Some(adapter) = &self.adapter {
                let mut events = adapter.events().await.map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to get BLE events: {}", e))
                })?;
                
                // Clone necessary data for the discovery task
                let peers = Arc::clone(&self.peers);
                let _cached_peers = Arc::clone(&self.cached_peers);
                let active = Arc::clone(&self.active);
                let _config = self.config.clone();
                let _packet_tx = self.packet_tx.clone();
                
                // Discovery event handler
                let discovery_handle = tokio::spawn(async move {
                    while let Some(event) = events.next().await {
                        if !*active.read().await {
                            break;
                        }
                        
                        match event {
                            CentralEvent::DeviceDiscovered(id) => {
                                // Handle device discovery
                                debug!("Device discovered: {:?}", id);
                            }
                            CentralEvent::DeviceDisconnected(id) => {
                                // Mark peer as disconnected
                                let mut peers_lock = peers.write().await;
                                for peer in peers_lock.values_mut() {
                                    if peer.peripheral.id() == id {
                                        peer.connection_state = ConnectionState::Disconnected;
                                        debug!("Peer {} disconnected", peer.peer_id);
                                        break;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    debug!("Discovery event handler ended");
                });
                
                self.task_handles.push(discovery_handle);
            }

            // Start background connection manager
            let peers = Arc::clone(&self.peers);
            let active = Arc::clone(&self.active);
            let config = self.config.clone();
            let _packet_tx = self.packet_tx.clone();
            
            let connection_manager_handle = tokio::spawn(async move {
                let mut reconnect_interval = interval(Duration::from_secs(10));
                
                while *active.read().await {
                    reconnect_interval.tick().await;
                    
                    // Auto-reconnect to failed peers if enabled
                    if config.auto_reconnect {
                        let peer_ids: Vec<PeerId> = {
                            let peers_lock = peers.read().await;
                            peers_lock.iter()
                                .filter(|(_, peer)| {
                                    (peer.connection_state == ConnectionState::Disconnected ||
                                     peer.connection_state == ConnectionState::Failed) &&
                                    peer.can_retry()
                                })
                                .map(|(id, _)| *id)
                                .collect()
                        };
                        
                        for peer_id in peer_ids {
                            // Attempt reconnection logic here
                            debug!("Attempting reconnection to peer: {}", peer_id);
                        }
                    }
                }
                debug!("Connection manager ended");
            });
            
            self.task_handles.push(connection_manager_handle);

            info!("BLE transport started with event-driven architecture");
            Ok(())
        })
    }

    fn stop(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            *self.active.write().await = false;

            // Stop scanning
            self.stop_scanning().await?;
            
            // Cancel all background tasks
            for handle in self.task_handles.drain(..) {
                handle.abort();
            }

            // Disconnect from all peers
            let mut peers = self.peers.write().await;
            for peer in peers.values_mut() {
                if peer.is_connected() {
                    if let Err(e) = peer.peripheral.disconnect().await {
                        error!("Failed to disconnect from peer: {}", e);
                    }
                    peer.connection_state = ConnectionState::Disconnected;
                }
            }
            
            // Clear cached peers
            self.cached_peers.write().await.clear();

            info!("BLE transport stopped");
            Ok(())
        })
    }

    fn is_active(&self) -> bool {
        // Use a more efficient non-blocking approach when possible
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { *self.active.read().await })
        })
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            transport_type: TransportType::Ble,
            max_packet_size: self.config.max_packet_size,
            supports_discovery: true,
            supports_broadcast: true,
            requires_internet: false,
            latency_class: LatencyClass::Low,
            reliability_class: ReliabilityClass::Medium,
        }
    }
}

// ----------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------

/// Generate a BLE-compatible device name for this peer
pub fn generate_device_name(peer_id: &PeerId, prefix: &str) -> String {
    format!("{}-{}", prefix, hex::encode(peer_id.as_bytes()))
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_extraction() {
        let transport = BleTransport::new(PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));

        // Valid peer ID
        let name = "BitChat-0102030405060708";
        let peer_id = transport.extract_peer_id_from_name(name);
        assert!(peer_id.is_some());
        assert_eq!(peer_id.unwrap(), PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));

        // Invalid format
        let name = "SomeOtherDevice";
        let peer_id = transport.extract_peer_id_from_name(name);
        assert!(peer_id.is_none());

        // Invalid hex
        let name = "BitChat-invalid_hex";
        let peer_id = transport.extract_peer_id_from_name(name);
        assert!(peer_id.is_none());
    }

    #[test]
    fn test_device_name_generation() {
        let peer_id = PeerId::new([0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A]);
        let name = generate_device_name(&peer_id, "BitChat");
        assert_eq!(name, "BitChat-abcdef123456789a");
    }

    #[test]
    fn test_transport_capabilities() {
        let transport = BleTransport::new(PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));
        let caps = transport.capabilities();

        assert_eq!(caps.transport_type, TransportType::Ble);
        assert!(caps.supports_discovery);
        assert!(caps.supports_broadcast);
        assert!(!caps.requires_internet);
        assert_eq!(caps.latency_class, LatencyClass::Low);
        assert_eq!(caps.reliability_class, ReliabilityClass::Medium);
    }
}
