//! BLE Transport Task Implementation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use smallvec::SmallVec;

use bitchat_core::internal::{IdentityKeyPair, TransportError};
use bitchat_core::protocol::{BitchatPacket, DiscoveredPeer, MessageType, WireFormat};
use bitchat_core::{BitchatError, BitchatResult, PeerId, Timestamp, TransportTask};
use bitchat_core::{EffectReceiver, EventSender};
use bitchat_harness::{
    messages::{ChannelTransportType, Effect, Event},
    TransportHandle,
};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::config::BleTransportConfig;
use crate::connection::BleConnection;
use crate::discovery::BleDiscovery;
use crate::peer::BlePeer;

// ----------------------------------------------------------------------------
// BLE Transport Task Implementation
// ----------------------------------------------------------------------------

/// BLE transport task that implements the new transport architecture
pub struct BleTransportTask {
    /// Transport type
    transport_type: ChannelTransportType,
    /// Channels provided by the runtime harness
    transport_channels: Option<TransportHandle>,
    /// Task running state
    running: bool,
    /// Transport configuration
    config: BleTransportConfig,
    /// Discovery manager
    discovery: BleDiscovery,
    /// Connection manager
    connection: BleConnection,
    /// Discovered peers
    peers: Arc<RwLock<HashMap<PeerId, BlePeer>>>,
    /// Our own peer ID for identification
    local_peer_id: PeerId,
    /// Identity keypair for advertising
    identity: IdentityKeyPair,
    /// Background task handles
    #[allow(dead_code)]
    task_handles: Vec<JoinHandle<()>>,
    /// Cached discovered peers (non-blocking access)
    cached_peers: Arc<RwLock<Vec<PeerId>>>,
    /// Packet receiver for incoming data from BLE connections
    packet_rx: Option<mpsc::UnboundedReceiver<(PeerId, Vec<u8>)>>,
}

impl Default for BleTransportTask {
    fn default() -> Self {
        Self::new()
    }
}

impl BleTransportTask {
    /// Create a new BLE transport task
    pub fn new() -> Self {
        let local_peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]); // Default for now
        let config = BleTransportConfig::default();
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();

        let discovery = BleDiscovery::new(config.clone());
        let connection = BleConnection::new(config.clone(), packet_tx);
        let identity = IdentityKeyPair::generate().unwrap_or_else(|_| {
            // Fallback to a dummy identity if generation fails
            IdentityKeyPair::generate().unwrap() // This would normally not fail twice
        });

        Self {
            transport_type: ChannelTransportType::Ble,
            transport_channels: None,
            running: false,
            config,
            discovery,
            connection,
            peers: Arc::new(RwLock::new(HashMap::new())),
            local_peer_id,
            identity,
            task_handles: Vec::new(),
            cached_peers: Arc::new(RwLock::new(Vec::new())),
            packet_rx: Some(packet_rx),
        }
    }

    /// Main task loop processing effects from Core Logic
    pub async fn run_internal(&mut self) -> BitchatResult<()> {
        tracing::info!("BLE transport task starting");

        let channels = self.transport_channels.as_mut().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE transport started without transport channels".to_string(),
            })
        })?;
        let mut effect_receiver = channels.take_effect_receiver().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE transport started twice".to_string(),
            })
        })?;

        let mut packet_receiver = self.packet_rx.take().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE packet receiver already taken".to_string(),
            })
        })?;

        self.running = true;

        while self.running {
            tokio::select! {
                // Process effects from Core Logic
                effect = effect_receiver.recv() => {
                    match effect {
                        Ok(eff) => {
                            if let Err(e) = self.process_effect(eff).await {
                                tracing::error!("Effect processing error: {}", e);
                            }
                        }
                        Err(_) => {
                            tracing::info!("Effect channel closed, shutting down");
                            break;
                        }
                    }
                }

                // Process incoming packets from BLE connections
                packet = packet_receiver.recv() => {
                    match packet {
                        Some((peer_id, data)) => {
                            if let Err(e) = self.handle_incoming_packet(peer_id, data).await {
                                tracing::error!("Failed to handle incoming packet from {}: {}", peer_id, e);
                            }
                        }
                        None => {
                            tracing::info!("Packet channel closed");
                        }
                    }
                }

                // Periodic discovery scanning
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                    self.perform_discovery_scan().await;
                }

                // Periodic maintenance
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    self.perform_maintenance().await;
                }
            }
        }

        tracing::info!("BLE transport task stopped");

        Ok(())
    }

    /// Process effect from Core Logic
    async fn process_effect(&mut self, effect: Effect) -> BitchatResult<()> {
        match effect {
            Effect::SendPacket {
                peer_id,
                data,
                transport,
            } if transport == self.transport_type => {
                self.send_packet_to_peer(peer_id, data.to_vec()).await?;
            }
            Effect::SendBitchatPacket {
                peer_id,
                packet,
                transport,
            } if transport == self.transport_type => {
                self.send_bitchat_packet_to_peer(peer_id, packet).await?;
            }
            Effect::BroadcastBitchatPacket { packet, transport }
                if transport == self.transport_type =>
            {
                self.broadcast_bitchat_packet(packet).await?;
            }
            Effect::InitiateConnection { peer_id, transport }
                if transport == self.transport_type =>
            {
                self.initiate_connection(peer_id).await?;
            }
            Effect::StartListening { transport } if transport == self.transport_type => {
                self.start_advertising().await?;
            }
            Effect::StopListening { transport } if transport == self.transport_type => {
                self.stop_advertising().await?;
            }
            Effect::StartTransportDiscovery { transport } if transport == self.transport_type => {
                self.start_discovery().await?;
            }
            Effect::StopTransportDiscovery { transport } if transport == self.transport_type => {
                self.stop_discovery().await?;
            }
            _ => {
                // Effect not for this transport - ignore
            }
        }
        Ok(())
    }

    /// Send packet to specific peer via BLE
    async fn send_packet_to_peer(&mut self, peer_id: PeerId, data: Vec<u8>) -> BitchatResult<()> {
        // Check if peer exists and is connected
        {
            let peers = self.peers.read().await;
            let peer = peers.get(&peer_id).ok_or_else(|| {
                BitchatError::Transport(TransportError::PeerNotFound {
                    peer_id: peer_id.to_string(),
                })
            })?;

            if !peer.is_connected() {
                return Err(BitchatError::Transport(TransportError::ConnectionFailed {
                    peer_id: peer_id.to_string(),
                    reason: "Peer not connected".to_string(),
                }));
            }
        }

        if data.len() > self.config.max_packet_size {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Packet too large for BLE".to_string(),
                },
            ));
        }

        self.connection
            .send_to_peer(&peer_id, &data, &self.peers)
            .await
    }

    /// Send BitChat packet to specific peer via BLE
    async fn send_bitchat_packet_to_peer(
        &mut self,
        peer_id: PeerId,
        packet: BitchatPacket,
    ) -> BitchatResult<()> {
        // Serialize the packet to binary wire format
        let data = WireFormat::encode(&packet).map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to encode BitChat packet: {}", e),
            })
        })?;

        // Use existing packet sending logic
        self.send_packet_to_peer(peer_id, data).await
    }

    /// Broadcast BitChat packet to all connected peers
    async fn broadcast_bitchat_packet(&mut self, packet: BitchatPacket) -> BitchatResult<()> {
        // Serialize the packet to binary wire format
        let data = WireFormat::encode(&packet).map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to encode BitChat packet: {}", e),
            })
        })?;

        // Check packet size
        if data.len() > self.config.max_packet_size {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Packet too large for BLE".to_string(),
                },
            ));
        }

        // Broadcast to all connected peers
        let peers = self.peers.read().await;
        let connected_peers: Vec<PeerId> = peers
            .iter()
            .filter(|(_, peer)| peer.is_connected())
            .map(|(peer_id, _)| *peer_id)
            .collect();
        drop(peers);

        for peer_id in connected_peers {
            if let Err(e) = self
                .connection
                .send_to_peer(&peer_id, &data, &self.peers)
                .await
            {
                tracing::warn!("Failed to broadcast to peer {}: {}", peer_id, e);
                // Continue broadcasting to other peers instead of failing completely
            }
        }

        Ok(())
    }

    /// Initiate BLE connection to peer
    async fn initiate_connection(&mut self, peer_id: PeerId) -> BitchatResult<()> {
        let peers = self.peers.read().await;
        if peers.contains_key(&peer_id) {
            drop(peers);

            // Simulate BLE connection establishment
            tracing::info!("Established BLE connection to peer {}", peer_id);

            // Send connection established event to Core Logic
            let event = Event::ConnectionEstablished {
                peer_id,
                transport: self.transport_type,
            };
            self.send_event(event).await?;

            Ok(())
        } else {
            Err(BitchatError::Transport(TransportError::PeerNotFound {
                peer_id: peer_id.to_string(),
            }))
        }
    }

    /// Start BLE advertising
    async fn start_advertising(&mut self) -> BitchatResult<()> {
        self.discovery
            .start_advertising(self.local_peer_id, &self.identity)
            .await?;
        tracing::info!("Started BLE advertising");
        Ok(())
    }

    /// Stop BLE advertising
    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        self.discovery.stop_advertising().await?;
        tracing::info!("Stopped BLE advertising");
        Ok(())
    }

    /// Start BLE discovery
    async fn start_discovery(&mut self) -> BitchatResult<()> {
        self.discovery.start_scanning().await?;
        tracing::info!("Started BLE discovery");
        Ok(())
    }

    /// Stop BLE discovery
    async fn stop_discovery(&mut self) -> BitchatResult<()> {
        self.discovery.stop_scanning().await?;
        tracing::info!("Stopped BLE discovery");
        Ok(())
    }

    /// Perform discovery scan for BLE peers
    async fn perform_discovery_scan(&mut self) {
        // Mock discovery of a peer for demonstration
        if self.peers.read().await.is_empty() {
            let _mock_peer = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
            // We would need a real peripheral for a proper implementation
            // For now, this is just demonstrating the structure
        }
    }

    /// Send event to Core Logic
    async fn send_event(&self, event: Event) -> BitchatResult<()> {
        let sender = self
            .transport_channels
            .as_ref()
            .ok_or_else(|| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: "BLE transport missing event sender".to_string(),
                })
            })?
            .event_sender();

        sender.send(event).await.map_err(|_| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Failed to send event - channel closed".to_string(),
            })
        })
    }

    /// Handle incoming packet from BLE connection with TTL-based mesh routing
    async fn handle_incoming_packet(
        &mut self,
        from_peer: PeerId,
        data: Vec<u8>,
    ) -> BitchatResult<()> {
        // Try to decode as wire protocol packet
        let packet = match WireFormat::decode(&data) {
            Ok(packet) => packet,
            Err(e) => {
                tracing::debug!("Failed to decode packet from {}: {}", from_peer, e);
                return Ok(()); // Ignore invalid packets
            }
        };

        tracing::debug!(
            "Received packet from {} type:{:?} ttl:{}",
            from_peer,
            packet.header.message_type,
            packet.header.ttl.value()
        );

        // Handle announce packets specially
        if packet.header.message_type == MessageType::Announce {
            if let Err(e) = self.handle_announce_packet(from_peer, packet.clone()).await {
                tracing::warn!("Failed to handle announce packet from {}: {}", from_peer, e);
            }
            // Don't return here - still forward announce packets to core logic
        }

        // Check if packet is for us
        let is_for_us = packet.is_broadcast() || packet.recipient_id == Some(self.local_peer_id);

        if is_for_us {
            // Send packet to Core Logic
            let event = Event::BitchatPacketReceived {
                from: packet.sender_id,
                packet: packet.clone(),
                transport: self.transport_type,
            };
            self.send_event(event).await?;
        }

        // If not for us and TTL > 0, forward the packet (mesh routing)
        if !is_for_us && packet.header.ttl.value() > 0 {
            self.forward_packet(packet, from_peer).await?;
        }

        Ok(())
    }

    /// Forward packet to other peers (mesh routing)
    async fn forward_packet(
        &mut self,
        mut packet: BitchatPacket,
        from_peer: PeerId,
    ) -> BitchatResult<()> {
        // Decrement TTL
        let new_ttl = match packet.header.ttl.decrement() {
            Some(ttl) => ttl,
            None => {
                tracing::debug!("Dropping packet with TTL=0");
                return Ok(());
            }
        };

        // Update packet TTL
        packet.header.ttl = new_ttl;
        packet.header.payload_length = packet.payload.len() as u32;

        // TODO: Add duplicate detection using bloom filter to prevent loops
        // For now, we implement basic duplicate prevention by not forwarding back to sender

        // Get connected peers (excluding the sender)
        let peers = self.peers.read().await;
        let connected_peers: Vec<PeerId> = peers
            .iter()
            .filter(|(peer_id, peer)| **peer_id != from_peer && peer.is_connected())
            .map(|(peer_id, _)| *peer_id)
            .collect();
        drop(peers);

        if connected_peers.is_empty() {
            tracing::debug!("No peers to forward packet to");
            return Ok(());
        }

        let peer_count = connected_peers.len();

        // Forward to all connected peers except sender
        for peer_id in connected_peers {
            if let Err(e) = self
                .send_bitchat_packet_to_peer(peer_id, packet.clone())
                .await
            {
                tracing::warn!("Failed to forward packet to peer {}: {}", peer_id, e);
                // Continue forwarding to other peers
            }
        }

        tracing::debug!(
            "Forwarded packet with TTL={} to {} peers",
            new_ttl.value(),
            peer_count
        );

        Ok(())
    }

    /// Perform periodic maintenance
    async fn perform_maintenance(&mut self) {
        let _current_time = std::time::SystemTime::now();
        let timeout_threshold = Duration::from_secs(300); // 5 minutes

        // Remove stale discovered peers based on last connection attempt
        let mut stale_peers = Vec::new();
        {
            let peers = self.peers.read().await;
            for (peer_id, peer) in peers.iter() {
                if let Some(last_attempt) = peer.last_connection_attempt {
                    if last_attempt.elapsed() > timeout_threshold {
                        stale_peers.push(*peer_id);
                    }
                }
            }
        }

        if !stale_peers.is_empty() {
            let mut peers = self.peers.write().await;
            let mut cached_peers = self.cached_peers.write().await;

            for peer_id in stale_peers {
                peers.remove(&peer_id);
                cached_peers.retain(|&p| p != peer_id);

                tracing::debug!("Removed stale BLE peer {}", peer_id);
            }
        }
    }

    /// Get discovered peers (non-blocking)
    pub fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        match self.cached_peers.try_read() {
            Ok(cached_peers) => SmallVec::from_vec(cached_peers.clone()),
            Err(_) => SmallVec::new(),
        }
    }

    /// Check if task is running (non-blocking)
    pub fn is_active(&self) -> bool {
        self.running
    }

    /// Send an announce packet to a specific peer
    pub async fn send_announce_to_peer(
        &mut self,
        peer_id: PeerId,
        nickname: String,
    ) -> BitchatResult<()> {
        let noise_public_key = [0u8; 32]; // TODO: Get from noise session when available

        let announce_packet = BitchatPacket::create_announce(
            self.local_peer_id,
            nickname,
            noise_public_key,
            &self.identity,
            None, // No direct neighbors for now
            Timestamp::now(),
        )?;

        self.send_bitchat_packet_to_peer(peer_id, announce_packet)
            .await
    }

    /// Broadcast an announce packet to all connected peers
    pub async fn broadcast_announce(&mut self, nickname: String) -> BitchatResult<()> {
        let noise_public_key = [0u8; 32]; // TODO: Get from noise session when available

        let announce_packet = BitchatPacket::create_announce(
            self.local_peer_id,
            nickname,
            noise_public_key,
            &self.identity,
            None, // No direct neighbors for now
            Timestamp::now(),
        )?;

        self.broadcast_bitchat_packet(announce_packet).await
    }

    /// Handle an incoming announce packet from a peer
    pub async fn handle_announce_packet(
        &mut self,
        peer_id: PeerId,
        packet: BitchatPacket,
    ) -> BitchatResult<()> {
        if packet.message_type() != MessageType::Announce {
            return Err(BitchatError::invalid_packet("Expected announce packet"));
        }

        // Parse and verify the announce packet
        let discovered_peer = DiscoveredPeer::from_announce_packet(&packet, Timestamp::now())?;

        // Verify the sender ID matches the packet sender
        if discovered_peer.peer_id != peer_id {
            return Err(BitchatError::invalid_packet(
                "Announce packet sender ID mismatch",
            ));
        }

        // Update or add the peer to our discovered peers list
        {
            let mut peers = self.peers.write().await;
            if let Some(ble_peer) = peers.get_mut(&peer_id) {
                // Update existing peer with announce information
                ble_peer.update_from_announce(&discovered_peer)?;
            } else {
                // Add new peer discovered via announce
                let ble_peer = BlePeer::from_discovered_peer(discovered_peer)?;
                peers.insert(peer_id, ble_peer);
            }
        }

        // Notify the runtime about the peer discovery
        if let Some(ref channels) = self.transport_channels {
            let event = Event::PeerDiscovered {
                peer_id,
                transport: self.transport_type,
                signal_strength: None, // BLE signal strength not available here
            };

            if let Err(e) = channels.event_sender().send(event).await {
                tracing::warn!("Failed to send peer discovery event: {}", e);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl TransportTask for BleTransportTask {
    fn attach_channels(
        &mut self,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<()> {
        if self.transport_channels.is_some() {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "BLE transport channels already attached".to_string(),
                },
            ));
        }
        self.transport_channels = Some(TransportHandle::new(event_sender, effect_receiver));
        Ok(())
    }

    async fn run(&mut self) -> BitchatResult<()> {
        // Delegate to the existing run implementation
        self.run_internal().await
    }

    fn transport_type(&self) -> ChannelTransportType {
        self.transport_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_creation() {
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);
        let (_effect_tx, effect_rx) = tokio::sync::broadcast::channel(100);

        let mut transport = BleTransportTask::new();
        transport.attach_channels(event_tx, effect_rx).unwrap();
        assert_eq!(transport.transport_type(), ChannelTransportType::Ble);
        assert!(!transport.is_active());
    }
}
