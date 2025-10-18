//! Main BLE transport implementation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use smallvec::SmallVec;

use bitchat_core::transport::{
    LatencyClass, ReliabilityClass, Transport, TransportCapabilities, TransportType,
};
use bitchat_core::{BitchatError, BitchatPacket, PeerId, Result as BitchatResult};
use btleplug::api::Central;
use futures::stream::StreamExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::config::BleTransportConfig;
use crate::connection::BleConnection;
use crate::discovery::BleDiscovery;
use crate::peer::{BlePeer, ConnectionState};

// ----------------------------------------------------------------------------
// Main BLE Transport Implementation
// ----------------------------------------------------------------------------

/// BLE transport for BitChat communication
pub struct BleTransport {
    /// Transport configuration
    config: BleTransportConfig,
    /// Discovery manager
    discovery: BleDiscovery,
    /// Connection manager
    connection: BleConnection,
    /// Discovered peers
    peers: Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    /// Receiver for incoming packets
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>>,
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

        let discovery = BleDiscovery::new(config.clone());
        let connection = BleConnection::new(config.clone(), packet_tx);

        Self {
            config,
            discovery,
            connection,
            peers: Arc::new(RwLock::new(HashMap::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            active: Arc::new(RwLock::new(false)),
            local_peer_id,
            task_handles: Vec::new(),
            cached_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start background connection manager
    async fn start_connection_manager(&mut self) -> BitchatResult<()> {
        let peers = Arc::clone(&self.peers);
        let active = Arc::clone(&self.active);
        let config = self.config.clone();
        let connection = BleConnection::new(config.clone(), mpsc::unbounded_channel().0); // Dummy sender for manager

        let connection_manager_handle = tokio::spawn(async move {
            let mut reconnect_interval = interval(Duration::from_secs(10));

            while *active.read().await {
                reconnect_interval.tick().await;

                // Auto-reconnect to failed peers if enabled
                if config.auto_reconnect {
                    let peer_ids: Vec<PeerId> = {
                        let peers_lock = peers.read().await;
                        peers_lock
                            .iter()
                            .filter(|(_, peer)| {
                                (peer.connection_state == ConnectionState::Disconnected
                                    || peer.connection_state == ConnectionState::Failed)
                                    && peer.can_retry()
                            })
                            .map(|(id, _)| *id)
                            .collect()
                    };

                    for peer_id in peer_ids {
                        if let Err(e) = connection.connect_to_peer(&peer_id, &peers).await {
                            debug!("Auto-reconnect failed for peer {}: {}", peer_id, e);
                        }
                    }
                }
            }
            debug!("Connection manager ended");
        });

        self.task_handles.push(connection_manager_handle);
        Ok(())
    }

    /// Start discovery event processing
    async fn start_discovery_events(&mut self) -> BitchatResult<()> {
        if let Some(adapter) = self.discovery.adapter() {
            let mut events = adapter
                .events()
                .await
                .map_err(|e| BitchatError::Transport {
                    message: format!("Failed to get BLE events: {}", e),
                })?;

            // Clone necessary data for the discovery task
            let peers = Arc::clone(&self.peers);
            let cached_peers = Arc::clone(&self.cached_peers);
            let active = Arc::clone(&self.active);
            let discovery = BleDiscovery::new(self.config.clone());

            // Discovery event handler
            let discovery_handle = tokio::spawn(async move {
                while let Some(event) = events.next().await {
                    if !*active.read().await {
                        break;
                    }

                    if let Err(e) = discovery
                        .process_discovery_event(event, &peers, &cached_peers)
                        .await
                    {
                        error!("Failed to process discovery event: {}", e);
                    }
                }
                debug!("Discovery event handler ended");
            });

            self.task_handles.push(discovery_handle);
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for BleTransport {
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> BitchatResult<()> {
        // Check if peer exists and is connected
        {
            let peers = self.peers.read().await;
            let peer = peers.get(&peer_id).ok_or_else(|| BitchatError::Transport {
                message: "Peer not discovered".to_string(),
            })?;

            if !peer.is_connected() {
                return Err(BitchatError::Transport {
                    message: "Peer not connected".to_string(),
                });
            }
        }

        // Serialize packet
        let data = bincode::serialize(&packet).map_err(BitchatError::Serialization)?;

        if data.len() > self.config.max_packet_size {
            return Err(BitchatError::Transport {
                message: "Packet too large for BLE".to_string(),
            });
        }

        self.connection
            .send_to_peer(&peer_id, &data, &self.peers)
            .await
    }

    async fn broadcast(&mut self, packet: BitchatPacket) -> BitchatResult<()> {
        // Get connected peers only
        let connected_peers = self.connection.get_connected_peers(&self.peers).await;

        if connected_peers.is_empty() {
            warn!("No connected peers for broadcast");
            return Ok(());
        }

        // Serialize packet once
        let data = bincode::serialize(&packet).map_err(BitchatError::Serialization)?;

        if data.len() > self.config.max_packet_size {
            return Err(BitchatError::Transport {
                message: "Packet too large for BLE broadcast".to_string(),
            });
        }

        // Send to all connected peers sequentially
        // Note: Due to BLE limitations, concurrent sends might not work reliably
        for peer_id in connected_peers {
            if let Err(e) = self
                .connection
                .send_to_peer(&peer_id, &data, &self.peers)
                .await
            {
                error!("Failed to broadcast to peer {}: {}", peer_id, e);
            }
        }

        Ok(())
    }

    async fn receive(&mut self) -> BitchatResult<(PeerId, BitchatPacket)> {
        let mut rx = self.packet_rx.lock().await;
        rx.recv().await.ok_or_else(|| BitchatError::Transport {
            message: "Receive channel closed".to_string(),
        })
    }

    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        // Use try_read for non-blocking access to cached peers
        match self.cached_peers.try_read() {
            Ok(cached_peers) => SmallVec::from_vec(cached_peers.clone()),
            Err(_) => {
                // If we can't acquire the lock non-blockingly, return empty list
                // This prevents blocking the caller and is safe since cached_peers
                // is updated asynchronously in the background
                SmallVec::new()
            }
        }
    }

    async fn start(&mut self) -> BitchatResult<()> {
        *self.active.write().await = true;

        // Initialize BLE adapter
        self.discovery.initialize_adapter().await?;

        // Start advertising (now with platform-specific support)
        self.discovery.start_advertising(self.local_peer_id).await?;

        // Start scanning for peers
        self.discovery.start_scanning().await?;

        // Start event-driven peer discovery
        self.start_discovery_events().await?;

        // Start background connection manager
        self.start_connection_manager().await?;

        info!("BLE transport started with event-driven architecture");
        Ok(())
    }

    async fn stop(&mut self) -> BitchatResult<()> {
        *self.active.write().await = false;

        // Stop scanning and advertising
        self.discovery.stop_scanning().await?;
        self.discovery.stop_advertising().await?;

        // Cancel all background tasks
        for handle in self.task_handles.drain(..) {
            handle.abort();
        }

        // Disconnect from all peers
        self.connection.disconnect_all_peers(&self.peers).await?;

        // Clear cached peers
        self.cached_peers.write().await.clear();

        info!("BLE transport stopped");
        Ok(())
    }

    fn is_active(&self) -> bool {
        // Use try_read for non-blocking access to active state
        match self.active.try_read() {
            Ok(active) => *active,
            Err(_) => {
                // If we can't acquire the lock non-blockingly, assume inactive
                // This is a safe conservative default
                false
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
