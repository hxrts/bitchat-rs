//! BLE connection management and data transmission

use std::collections::HashMap;
use std::sync::Arc;

use bitchat_core::{BitchatError, BitchatPacket, PeerId, Result as BitchatResult};
use btleplug::api::{Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::stream::StreamExt;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info};

use crate::config::BleTransportConfig;
use crate::peer::{BlePeer, ConnectionState};
use crate::protocol::{BITCHAT_RX_CHARACTERISTIC_UUID, BITCHAT_TX_CHARACTERISTIC_UUID};

// ----------------------------------------------------------------------------
// Connection Management
// ----------------------------------------------------------------------------

/// Handles BLE connections and data transmission
pub struct BleConnection {
    config: BleTransportConfig,
    packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
}

impl BleConnection {
    /// Create a new connection manager
    pub fn new(
        config: BleTransportConfig,
        packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    ) -> Self {
        Self { config, packet_tx }
    }

    /// Connect to a specific peer with proper state management
    pub async fn connect_to_peer(
        &self,
        peer_id: &PeerId,
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    ) -> BitchatResult<()> {
        let mut peers_lock = peers.write().await;
        let peer = peers_lock
            .get_mut(peer_id)
            .ok_or_else(|| BitchatError::InvalidPacket("Peer not found".into()))?;

        if peer.is_connected() {
            return Ok(());
        }
        
        if peer.is_connecting() {
            return Err(BitchatError::InvalidPacket("Connection in progress".into()));
        }
        
        if !peer.can_retry() {
            return Err(BitchatError::InvalidPacket("Too many failed connection attempts".into()));
        }

        peer.start_connection_attempt();

        let connect_result =
            timeout(self.config.connection_timeout, peer.peripheral.connect()).await;

        match connect_result {
            Ok(Ok(_)) => {
                peer.mark_connected();
                info!("Connected to peer: {}", peer_id);

                // Discover services and characteristics
                if let Err(e) = peer.peripheral.discover_services().await {
                    error!("Failed to discover services for peer {}: {}", peer_id, e);
                    peer.mark_failed();
                    return Err(BitchatError::InvalidPacket(format!("Failed to discover services: {}", e)));
                }

                // Start receiving data from this peer
                self.start_receiving_from_peer(peer_id, &peer.peripheral).await?;

                Ok(())
            }
            Ok(Err(e)) => {
                peer.mark_failed();
                error!("Failed to connect to peer {}: {}", peer_id, e);
                Err(BitchatError::InvalidPacket(format!("Connection failed: {}", e)))
            }
            Err(_) => {
                peer.mark_failed();
                error!("Connection to peer {} timed out", peer_id);
                Err(BitchatError::InvalidPacket("Connection timeout".into()))
            }
        }
    }
    
    /// Start receiving data from a connected peer
    pub async fn start_receiving_from_peer(&self, peer_id: &PeerId, peripheral: &Peripheral) -> BitchatResult<()> {
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
    pub async fn send_to_peer(
        &self,
        peer_id: &PeerId,
        data: &[u8],
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    ) -> BitchatResult<()> {
        let peers_lock = peers.read().await;
        let peer = peers_lock
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

    /// Disconnect from a peer
    pub async fn disconnect_peer(
        &self,
        peer_id: &PeerId,
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    ) -> BitchatResult<()> {
        let mut peers_lock = peers.write().await;
        if let Some(peer) = peers_lock.get_mut(peer_id) {
            if peer.is_connected() {
                if let Err(e) = peer.peripheral.disconnect().await {
                    error!("Failed to disconnect from peer {}: {}", peer_id, e);
                }
                peer.mark_disconnected();
                info!("Disconnected from peer: {}", peer_id);
            }
        }
        Ok(())
    }

    /// Disconnect from all peers
    pub async fn disconnect_all_peers(
        &self,
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    ) -> BitchatResult<()> {
        let mut peers_lock = peers.write().await;
        for (peer_id, peer) in peers_lock.iter_mut() {
            if peer.is_connected() {
                if let Err(e) = peer.peripheral.disconnect().await {
                    error!("Failed to disconnect from peer {}: {}", peer_id, e);
                }
                peer.mark_disconnected();
            }
        }
        info!("Disconnected from all peers");
        Ok(())
    }

    /// Get list of connected peer IDs
    pub async fn get_connected_peers(
        &self,
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    ) -> Vec<PeerId> {
        let peers_lock = peers.read().await;
        peers_lock
            .iter()
            .filter(|(_, peer)| peer.is_connected())
            .map(|(id, _)| *id)
            .collect()
    }
}