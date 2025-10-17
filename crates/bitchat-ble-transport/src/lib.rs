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
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, timeout};
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
#[derive(Debug, Clone)]
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

/// Information about a discovered BLE peer
#[derive(Debug, Clone)]
pub struct BlePeer {
    /// BitChat peer ID
    pub peer_id: PeerId,
    /// BLE peripheral
    pub peripheral: Peripheral,
    /// Device name
    pub device_name: String,
    /// Connection status
    pub connected: bool,
}

impl BlePeer {
    /// Create a new BLE peer
    pub fn new(peer_id: PeerId, peripheral: Peripheral, device_name: String) -> Self {
        Self {
            peer_id,
            peripheral,
            device_name,
            connected: false,
        }
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
    _packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    /// Whether the transport is active
    active: Arc<RwLock<bool>>,
    /// Our own peer ID for identification
    _local_peer_id: PeerId,
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
            _packet_tx: packet_tx,
            active: Arc::new(RwLock::new(false)),
            _local_peer_id: local_peer_id,
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

    /// Discover and process new peers
    async fn discover_peers(&self) -> BitchatResult<()> {
        let adapter = self
            .adapter
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("BLE adapter not initialized".into()))?;

        let peripherals = adapter.peripherals().await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get peripherals: {}", e))
        })?;

        for peripheral in peripherals {
            if let Ok(Some(properties)) = peripheral.properties().await {
                if let Some(name) = &properties.local_name {
                    if name.starts_with(&self.config.device_name_prefix) {
                        // Extract peer ID from device name or advertised data
                        if let Some(peer_id) = self.extract_peer_id_from_name(name) {
                            let ble_peer = BlePeer::new(peer_id, peripheral, name.clone());

                            let mut peers = self.peers.write().await;
                            if !peers.contains_key(&peer_id) {
                                debug!("Discovered new BitChat peer: {} ({})", peer_id, name);
                                peers.insert(peer_id, ble_peer);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract peer ID from device name
    /// Format: "BitChat-<hex_peer_id>"
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

    /// Connect to a specific peer
    async fn connect_to_peer(&self, peer_id: &PeerId) -> BitchatResult<()> {
        let mut peers = self.peers.write().await;
        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| BitchatError::InvalidPacket("Peer not found".into()))?;

        if peer.connected {
            return Ok(());
        }

        let connect_result =
            timeout(self.config.connection_timeout, peer.peripheral.connect()).await;

        match connect_result {
            Ok(Ok(_)) => {
                peer.connected = true;
                info!("Connected to peer: {}", peer_id);

                // Discover services and characteristics
                peer.peripheral.discover_services().await.map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to discover services: {}", e))
                })?;

                Ok(())
            }
            Ok(Err(e)) => {
                error!("Failed to connect to peer {}: {}", peer_id, e);
                Err(BitchatError::InvalidPacket(format!(
                    "Connection failed: {}",
                    e
                )))
            }
            Err(_) => {
                error!("Connection to peer {} timed out", peer_id);
                Err(BitchatError::InvalidPacket("Connection timeout".into()))
            }
        }
    }

    /// Send data to a connected peer
    async fn send_to_peer(&self, peer_id: &PeerId, data: &[u8]) -> BitchatResult<()> {
        let peers = self.peers.read().await;
        let peer = peers
            .get(peer_id)
            .ok_or_else(|| BitchatError::InvalidPacket("Peer not found".into()))?;

        if !peer.connected {
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
    async fn start_advertising(&self) -> BitchatResult<()> {
        // Note: btleplug doesn't currently support peripheral mode (advertising)
        // This would need platform-specific implementation or a different library
        // For now, we'll just log this as a limitation
        warn!("BLE advertising not yet implemented - this device won't be discoverable");
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
            // Ensure we're connected to the peer
            if !self.peers.read().await.contains_key(&peer_id) {
                return Err(BitchatError::InvalidPacket("Peer not discovered".into()));
            }

            self.connect_to_peer(&peer_id).await?;

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
            let peers: Vec<PeerId> = self.peers.read().await.keys().copied().collect();

            for peer_id in peers {
                if let Err(e) = self.send_to(peer_id, packet.clone()).await {
                    error!("Failed to send to peer {}: {}", peer_id, e);
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
        // This is a blocking call, but we need async access
        // In a real implementation, we'd use tokio::task::block_in_place or similar
        let peers = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.peers.read().await.keys().copied().collect() })
        });
        peers
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

            // Spawn background task for peer discovery
            let peers = Arc::clone(&self.peers);
            let adapter = self.adapter.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                loop {
                    if let Some(adapter) = &adapter {
                        // Discover new peers periodically
                        sleep(Duration::from_secs(5)).await;

                        if let Ok(peripherals) = adapter.peripherals().await {
                            for peripheral in peripherals {
                                // Process discovered peripherals
                                // (This is a simplified version - real implementation would be more robust)
                            }
                        }
                    }
                }
            });

            info!("BLE transport started");
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

            // Disconnect from all peers
            let mut peers = self.peers.write().await;
            for peer in peers.values_mut() {
                if peer.connected {
                    if let Err(e) = peer.peripheral.disconnect().await {
                        error!("Failed to disconnect from peer: {}", e);
                    }
                    peer.connected = false;
                }
            }

            info!("BLE transport stopped");
            Ok(())
        })
    }

    fn is_active(&self) -> bool {
        // This is a blocking call, but we need async access
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
