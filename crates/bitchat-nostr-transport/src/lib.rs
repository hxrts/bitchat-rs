//! Nostr transport implementation for BitChat
//!
//! This crate provides a Nostr transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Nostr relays.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bitchat_core::transport::{Transport, TransportCapabilities, TransportType, LatencyClass, ReliabilityClass};
use bitchat_core::{BitchatPacket, PeerId, Result as BitchatResult, BitchatError};
use nostr_sdk::prelude::*;
use nostr_sdk::{Keys, Client, EventBuilder, Filter, Kind, Tag, Timestamp, SubscriptionId, RelayPoolNotification, SecretKey, PublicKey};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use ::url::Url;

// ----------------------------------------------------------------------------
// Nostr Configuration
// ----------------------------------------------------------------------------

/// Configuration for Nostr transport
#[derive(Debug, Clone)]
pub struct NostrTransportConfig {
    /// List of Nostr relay URLs
    pub relay_urls: Vec<String>,
    /// Connection timeout for relays
    pub connection_timeout: Duration,
    /// Maximum time to wait for message delivery
    pub message_timeout: Duration,
    /// Maximum packet size
    pub max_packet_size: usize,
    /// Whether to automatically reconnect to relays
    pub auto_reconnect: bool,
    /// Private key for Nostr identity (None = generate random)
    pub private_key: Option<Keys>,
}

impl Default for NostrTransportConfig {
    fn default() -> Self {
        Self {
            relay_urls: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            connection_timeout: Duration::from_secs(10),
            message_timeout: Duration::from_secs(30),
            max_packet_size: 64000, // Nostr event content limit
            auto_reconnect: true,
            private_key: None,
        }
    }
}

// ----------------------------------------------------------------------------
// BitChat Nostr Event Format
// ----------------------------------------------------------------------------

/// BitChat message wrapper for Nostr events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatNostrMessage {
    /// BitChat peer ID of sender
    pub sender_peer_id: PeerId,
    /// BitChat peer ID of recipient (None for broadcast)
    pub recipient_peer_id: Option<PeerId>,
    /// Serialized BitChat packet
    pub packet_data: Vec<u8>,
    /// Message timestamp
    pub timestamp: u64,
}

impl BitchatNostrMessage {
    /// Create a new BitChat Nostr message
    pub fn new(sender_peer_id: PeerId, recipient_peer_id: Option<PeerId>, packet: &BitchatPacket) -> BitchatResult<Self> {
        let packet_data = bincode::serialize(packet).map_err(BitchatError::Serialization)?;
        
        Ok(Self {
            sender_peer_id,
            recipient_peer_id,
            packet_data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
    
    /// Extract BitChat packet from Nostr message
    pub fn to_packet(&self) -> BitchatResult<BitchatPacket> {
        bincode::deserialize(&self.packet_data).map_err(BitchatError::Serialization)
    }
}

// ----------------------------------------------------------------------------
// Nostr Transport Implementation
// ----------------------------------------------------------------------------

/// Nostr transport for BitChat communication
pub struct NostrTransport {
    /// Transport configuration
    config: NostrTransportConfig,
    /// Nostr client
    client: Option<Client>,
    /// Our Nostr keys
    keys: Keys,
    /// Our BitChat peer ID
    local_peer_id: PeerId,
    /// Discovered peers (Nostr pubkey -> BitChat peer ID)
    peers: Arc<RwLock<HashMap<PublicKey, PeerId>>>,
    /// Reverse mapping (BitChat peer ID -> Nostr pubkey)
    peer_keys: Arc<RwLock<HashMap<PeerId, PublicKey>>>,
    /// Receiver for incoming packets
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>>,
    /// Sender for incoming packets (used internally)
    packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    /// Whether the transport is active
    active: Arc<RwLock<bool>>,
}

impl NostrTransport {
    /// Create a new Nostr transport
    pub fn new(local_peer_id: PeerId) -> BitchatResult<Self> {
        Self::with_config(local_peer_id, NostrTransportConfig::default())
    }
    
    /// Create a new Nostr transport with custom configuration
    pub fn with_config(local_peer_id: PeerId, config: NostrTransportConfig) -> BitchatResult<Self> {
        let keys = config.private_key.clone()
            .unwrap_or_else(|| Keys::generate());
        
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();
        
        Ok(Self {
            config,
            client: None,
            keys,
            local_peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            peer_keys: Arc::new(RwLock::new(HashMap::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            packet_tx,
            active: Arc::new(RwLock::new(false)),
        })
    }
    
    /// Initialize Nostr client and connect to relays
    async fn initialize_client(&mut self) -> BitchatResult<()> {
        let client = Client::new(&self.keys);
        
        // Add relays
        for relay_url in &self.config.relay_urls {
            if let Ok(url) = Url::parse(relay_url) {
                client.add_relay(url).await.map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to add relay {}: {}", relay_url, e))
                })?;
            } else {
                warn!("Invalid relay URL: {}", relay_url);
            }
        }
        
        // Connect to relays
        client.connect().await;
        
        // Wait a bit for connections to establish
        sleep(Duration::from_secs(2)).await;
        
        self.client = Some(client);
        info!("Nostr client initialized with {} relays", self.config.relay_urls.len());
        Ok(())
    }
    
    /// Start listening for BitChat messages
    async fn start_listening(&self) -> BitchatResult<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;
        
        // Subscribe to BitChat messages
        let subscription_id = SubscriptionId::new("bitchat");
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .since(Timestamp::now());
        
        client.subscribe(vec![filter], None).await;
        let _subscription_id = subscription_id;
        
        // Handle incoming events
        let mut notifications = client.notifications();
        let packet_tx = self.packet_tx.clone();
        let peers: Arc<RwLock<HashMap<XOnlyPublicKey, PeerId>>> = Arc::clone(&self.peers);
        let peer_keys: Arc<RwLock<HashMap<PeerId, XOnlyPublicKey>>> = Arc::clone(&self.peer_keys);
        let local_pubkey = self.keys.public_key();
        
        tokio::spawn(async move {
            while let Ok(notification) = notifications.recv().await {
                match notification {
                    RelayPoolNotification::Event { event, .. } => {
                        // Skip our own events
                        if event.pubkey == local_pubkey {
                            continue;
                        }
                        
                        // Try to parse as BitChat message
                        if let Ok(bitchat_msg) = serde_json::from_str::<BitchatNostrMessage>(&event.content) {
                            // Check if this message is for us or is a broadcast
                            if bitchat_msg.recipient_peer_id.is_none() || 
                               bitchat_msg.recipient_peer_id == Some(bitchat_msg.sender_peer_id) {
                                
                                // Update peer mapping
                                {
                                    let mut peers = peers.write().await;
                                    let mut peer_keys = peer_keys.write().await;
                                    peers.insert(event.pubkey, bitchat_msg.sender_peer_id);
                                    peer_keys.insert(bitchat_msg.sender_peer_id, event.pubkey);
                                }
                                
                                // Forward packet
                                if let Ok(packet) = bitchat_msg.to_packet() {
                                    if let Err(_) = packet_tx.send((bitchat_msg.sender_peer_id, packet)) {
                                        error!("Failed to forward packet - receiver dropped");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    RelayPoolNotification::Message { message, .. } => {
                        debug!("Relay message: {:?}", message);
                    }
                    RelayPoolNotification::Shutdown => {
                        info!("Nostr relay pool shutdown");
                        break;
                    }
                    _ => {}
                }
            }
        });
        
        info!("Started listening for BitChat messages on Nostr");
        Ok(())
    }
    
    /// Send a BitChat packet to a specific peer via Nostr
    async fn send_to_peer(&self, peer_id: &PeerId, packet: &BitchatPacket) -> BitchatResult<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;
        
        // Check if we know the Nostr public key for this peer
        let target_pubkey = {
            let peer_keys = self.peer_keys.read().await;
            peer_keys.get(peer_id).copied()
        };
        
        let bitchat_msg = BitchatNostrMessage::new(self.local_peer_id, Some(*peer_id), packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e))
        })?;
        
        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket("Message too large for Nostr".into()));
        }
        
        let event = if let Some(pubkey) = target_pubkey {
            // Send encrypted direct message
            EventBuilder::encrypted_direct_msg(&self.keys, pubkey, content, None)
                .map_err(|e| BitchatError::InvalidPacket(format!("Failed to create encrypted message: {}", e)))?
                .to_event(&self.keys)
                .map_err(|e| BitchatError::InvalidPacket(format!("Failed to sign event: {}", e)))?
        } else {
            // Send as public message (peer discovery)
            EventBuilder::text_note(content, vec![])
                .to_event(&self.keys)
                .map_err(|e| BitchatError::InvalidPacket(format!("Failed to create event: {}", e)))?
        };
        
        client.send_event(event).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to send event: {}", e))
        })?;
        
        debug!("Sent BitChat packet to peer {} via Nostr", peer_id);
        Ok(())
    }
    
    /// Broadcast a BitChat packet to all peers via Nostr
    async fn broadcast_packet(&self, packet: &BitchatPacket) -> BitchatResult<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;
        
        let bitchat_msg = BitchatNostrMessage::new(self.local_peer_id, None, packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e))
        })?;
        
        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket("Message too large for Nostr".into()));
        }
        
        let event = EventBuilder::text_note(content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| BitchatError::InvalidPacket(format!("Failed to create event: {}", e)))?;
        
        client.send_event(event).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to send event: {}", e))
        })?;
        
        debug!("Broadcast BitChat packet via Nostr");
        Ok(())
    }
    
    /// Get our Nostr public key
    pub fn public_key(&self) -> XOnlyPublicKey {
        self.keys.public_key()
    }
    
    /// Get our Nostr private key
    pub fn secret_key(&self) -> Result<&SecretKey, nostr_sdk::key::Error> {
        self.keys.secret_key()
    }
}

impl Transport for NostrTransport {
    fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.send_to_peer(&peer_id, &packet).await
        })
    }
    
    fn broadcast(&mut self, packet: BitchatPacket) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.broadcast_packet(&packet).await
        })
    }
    
    fn receive(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<(PeerId, BitchatPacket)>> + Send + '_>> {
        let packet_rx = Arc::clone(&self.packet_rx);
        
        Box::pin(async move {
            let mut rx = packet_rx.lock().await;
            rx.recv().await
                .ok_or_else(|| BitchatError::InvalidPacket("Receive channel closed".into()))
        })
    }
    
    fn discovered_peers(&self) -> Vec<PeerId> {
        // This is a blocking call, but we need async access
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.peers.read().await.values().copied().collect()
            })
        })
    }
    
    fn start(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            *self.active.write().await = true;
            
            // Initialize Nostr client
            self.initialize_client().await?;
            
            // Start listening for messages
            self.start_listening().await?;
            
            info!("Nostr transport started");
            Ok(())
        })
    }
    
    fn stop(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            *self.active.write().await = false;
            
            // Disconnect from relays
            if let Some(client) = &self.client {
                client.disconnect().await.map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to disconnect: {}", e))
                })?;
            }
            
            info!("Nostr transport stopped");
            Ok(())
        })
    }
    
    fn is_active(&self) -> bool {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *self.active.read().await
            })
        })
    }
    
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            transport_type: TransportType::Nostr,
            max_packet_size: self.config.max_packet_size,
            supports_discovery: true,
            supports_broadcast: true,
            requires_internet: true,
            latency_class: LatencyClass::Medium,
            reliability_class: ReliabilityClass::High,
        }
    }
}

// ----------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------

/// Create a local Nostr relay configuration for testing
pub fn create_local_relay_config() -> NostrTransportConfig {
    NostrTransportConfig {
        relay_urls: vec!["ws://localhost:8080".to_string()],
        connection_timeout: Duration::from_secs(5),
        message_timeout: Duration::from_secs(10),
        max_packet_size: 64000,
        auto_reconnect: true,
        private_key: None,
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::packet::MessageType;

    #[test]
    fn test_bitchat_nostr_message() {
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let recipient_id = Some(PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]));
        
        let packet = BitchatPacket::new(
            MessageType::Message,
            sender_id,
            b"test message".to_vec(),
        );
        
        let nostr_msg = BitchatNostrMessage::new(sender_id, recipient_id, &packet).unwrap();
        
        assert_eq!(nostr_msg.sender_peer_id, sender_id);
        assert_eq!(nostr_msg.recipient_peer_id, recipient_id);
        
        let reconstructed_packet = nostr_msg.to_packet().unwrap();
        assert_eq!(reconstructed_packet.sender_id, sender_id);
        assert_eq!(reconstructed_packet.payload, b"test message");
    }
    
    #[test]
    fn test_transport_capabilities() {
        let transport = NostrTransport::new(PeerId::new([1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
        let caps = transport.capabilities();
        
        assert_eq!(caps.transport_type, TransportType::Nostr);
        assert!(caps.supports_discovery);
        assert!(caps.supports_broadcast);
        assert!(caps.requires_internet);
        assert_eq!(caps.latency_class, LatencyClass::Medium);
        assert_eq!(caps.reliability_class, ReliabilityClass::High);
    }
    
    #[test]
    fn test_local_relay_config() {
        let config = create_local_relay_config();
        assert_eq!(config.relay_urls, vec!["ws://localhost:8080"]);
        assert_eq!(config.connection_timeout, Duration::from_secs(5));
    }
}