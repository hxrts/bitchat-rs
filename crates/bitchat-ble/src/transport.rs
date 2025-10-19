//! BLE Transport Task Implementation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use smallvec::SmallVec;

use bitchat_core::{ChannelTransportType, Event, Effect};
use bitchat_core::TransportTask;
use bitchat_core::{EventSender, EffectReceiver};
use bitchat_core::internal::{IdentityKeyPair, TransportError};
use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
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
    /// Channel for sending events to Core Logic
    event_sender: Option<EventSender>,
    /// Channel for receiving effects from Core Logic
    effect_receiver: Option<EffectReceiver>,
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
    task_handles: Vec<JoinHandle<()>>,
    /// Cached discovered peers (non-blocking access)
    cached_peers: Arc<RwLock<Vec<PeerId>>>,
}

impl BleTransportTask {
    /// Create a new BLE transport task
    pub fn new() -> Self {
        let local_peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]); // Default for now
        let config = BleTransportConfig::default();
        let (packet_tx, _packet_rx) = mpsc::unbounded_channel();
        
        let discovery = BleDiscovery::new(config.clone());
        let connection = BleConnection::new(config.clone(), packet_tx);
        let identity = IdentityKeyPair::generate().unwrap_or_else(|_| {
            // Fallback to a dummy identity if generation fails
            IdentityKeyPair::generate().unwrap() // This would normally not fail twice
        });

        Self {
            transport_type: ChannelTransportType::Ble,
            event_sender: None,
            effect_receiver: None,
            running: false,
            config,
            discovery,
            connection,
            peers: Arc::new(RwLock::new(HashMap::new())),
            local_peer_id,
            identity,
            task_handles: Vec::new(),
            cached_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Main task loop processing effects from Core Logic
    pub async fn run_internal(&mut self) -> BitchatResult<()> {
        tracing::info!("BLE transport task starting");

        let mut effect_receiver = self.effect_receiver.take().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE transport started without attached effect receiver".to_string(),
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
            Effect::SendPacket { peer_id, data, transport } if transport == self.transport_type => {
                self.send_packet_to_peer(peer_id, data).await?;
            }
            Effect::InitiateConnection { peer_id, transport } if transport == self.transport_type => {
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
            let peer = peers.get(&peer_id).ok_or_else(|| BitchatError::Transport(
                TransportError::PeerNotFound {
                    peer_id: peer_id.to_string(),
                }
            ))?;

            if !peer.is_connected() {
                return Err(BitchatError::Transport(
                    TransportError::ConnectionFailed {
                        peer_id: peer_id.to_string(),
                        reason: "Peer not connected".to_string(),
                    }
                ));
            }
        }

        if data.len() > self.config.max_packet_size {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Packet too large for BLE".to_string(),
                }
            ));
        }

        self.connection
            .send_to_peer(&peer_id, &data, &self.peers)
            .await
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
            Err(BitchatError::Transport(
                TransportError::PeerNotFound {
                    peer_id: peer_id.to_string(),
                }
            ))
        }
    }

    /// Start BLE advertising
    async fn start_advertising(&mut self) -> BitchatResult<()> {
        self.discovery.start_advertising(self.local_peer_id, &self.identity).await?;
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
            return;
        }
    }

    /// Send event to Core Logic
    async fn send_event(&self, event: Event) -> BitchatResult<()> {
        #[allow(unused_variables)] // Used in feature-gated code below
        let sender = self.event_sender.as_ref().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE transport missing event sender".to_string(),
            })
        })?;

        #[cfg(feature = "std")]
        {
            sender.send(event).await.map_err(|_| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: "Failed to send event - channel closed".to_string(),
                })
            })?;
            return Ok(());
        }

        #[cfg(feature = "wasm")]
        {
            let mut sender_clone = sender.clone();
            sender_clone.send(event).await.map_err(|_| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: "Failed to send event - channel closed".to_string(),
                })
            })?;
            return Ok(());
        }

        #[cfg(not(any(feature = "std", feature = "wasm")))]
        {
            let _ = event;
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "No channel implementation available for BLE transport".to_string(),
            }));
        }
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
}

#[async_trait]
impl TransportTask for BleTransportTask {
    fn attach_channels(
        &mut self,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<()> {
        if self.event_sender.is_some() || self.effect_receiver.is_some() {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "BLE transport channels already attached".to_string(),
            }));
        }
        self.event_sender = Some(event_sender);
        self.effect_receiver = Some(effect_receiver);
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
