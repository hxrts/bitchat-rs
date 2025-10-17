//! Nostr transport implementation for BitChat
//!
//! This crate provides a Nostr transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Nostr relays.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ::url::Url;
use base64;
use bitchat_core::transport::{
    LatencyClass, ReliabilityClass, Transport, TransportCapabilities, TransportType,
};
use bitchat_core::{BitchatError, BitchatPacket, PeerId, Result as BitchatResult};
use thiserror::Error;
use nostr_sdk::prelude::*;
use nostr_sdk::{
    Client, EventBuilder, Filter, Keys, Kind, PublicKey, RelayPoolNotification, SecretKey,
    SubscriptionId, Tag, Timestamp,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors specific to the Nostr transport
#[derive(Error, Debug)]
pub enum NostrTransportError {
    #[error("Failed to connect to relay: {relay} - {source}")]
    RelayConnectionFailed {
        relay: String,
        #[source]
        source: nostr_sdk::client::Error,
    },
    
    #[error("Failed to send event: {0}")]
    EventSendFailed(#[from] nostr_sdk::client::Error),
    
    #[error("Failed to serialize message: {0}")]
    SerializationFailed(#[from] bincode::Error),
    
    #[error("Failed to deserialize message: {0}")]
    DeserializationFailed(String),
    
    #[error("Invalid relay URL: {url}")]
    InvalidRelayUrl { url: String },
    
    #[error("Client not initialized")]
    ClientNotInitialized,
    
    #[error("Message too large: {size} bytes (max: {max_size})")]
    MessageTooLarge { size: usize, max_size: usize },
    
    #[error("Failed to create encrypted message: {0}")]
    EncryptionFailed(String),
    
    #[error("Receive channel closed")]
    ReceiveChannelClosed,
    
    #[error("Unknown peer: {peer_id}")]
    UnknownPeer { peer_id: PeerId },
}

impl From<NostrTransportError> for BitchatError {
    fn from(err: NostrTransportError) -> Self {
        BitchatError::InvalidPacket(err.to_string())
    }
}

// ----------------------------------------------------------------------------
// Nostr Configuration
// ----------------------------------------------------------------------------

/// Configuration for Nostr transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    #[serde(skip)]
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

/// Custom Nostr event kind for BitChat protocol
pub const BITCHAT_KIND: Kind = Kind::Custom(30420);

/// BitChat message wrapper for Nostr events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatNostrMessage {
    /// BitChat peer ID of sender
    pub sender_peer_id: PeerId,
    /// BitChat peer ID of recipient (None for broadcast)
    pub recipient_peer_id: Option<PeerId>,
    /// Serialized BitChat packet (base64 encoded)
    pub packet_data: String,
    /// Message timestamp
    pub timestamp: u64,
}

impl BitchatNostrMessage {
    /// Create a new BitChat Nostr message
    pub fn new(
        sender_peer_id: PeerId,
        recipient_peer_id: Option<PeerId>,
        packet: &BitchatPacket,
    ) -> Result<Self, NostrTransportError> {
        let packet_bytes = bincode::serialize(packet)?;
        let packet_data = base64::encode(packet_bytes);

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
    pub fn to_packet(&self) -> Result<BitchatPacket, NostrTransportError> {
        let packet_bytes = base64::decode(&self.packet_data)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;
        Ok(bincode::deserialize(&packet_bytes)?)
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
    /// Discovered peers (BitChat peer ID -> Nostr public key)
    peers: Arc<RwLock<HashMap<PeerId, PublicKey>>>,
    /// Cached list of discovered peer IDs for quick access
    peer_cache: Arc<RwLock<Vec<PeerId>>>,
    /// Receiver for incoming packets
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>>,
    /// Sender for incoming packets (used internally)
    packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    /// Whether the transport is active (atomic for non-blocking access)
    active: Arc<AtomicBool>,
}

impl NostrTransport {
    /// Create a new Nostr transport
    pub fn new(local_peer_id: PeerId) -> BitchatResult<Self> {
        Self::with_config(local_peer_id, NostrTransportConfig::default())
    }

    /// Create a new Nostr transport with custom configuration
    pub fn with_config(local_peer_id: PeerId, config: NostrTransportConfig) -> BitchatResult<Self> {
        let keys = config
            .private_key
            .clone()
            .unwrap_or_else(|| Keys::generate());

        let (packet_tx, packet_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            client: None,
            keys,
            local_peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            peer_cache: Arc::new(RwLock::new(Vec::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            packet_tx,
            active: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Initialize Nostr client and connect to relays
    async fn initialize_client(&mut self) -> Result<(), NostrTransportError> {
        let client = Client::new(&self.keys);

        // Add relays
        for relay_url in &self.config.relay_urls {
            if let Ok(url) = Url::parse(relay_url) {
                client.add_relay(url).await.map_err(|source| {
                    NostrTransportError::RelayConnectionFailed {
                        relay: relay_url.clone(),
                        source,
                    }
                })?;
            } else {
                return Err(NostrTransportError::InvalidRelayUrl {
                    url: relay_url.clone(),
                });
            }
        }

        // Connect to relays
        client.connect().await;

        // Wait a bit for connections to establish
        sleep(Duration::from_secs(2)).await;

        self.client = Some(client);
        info!(
            "Nostr client initialized with {} relays",
            self.config.relay_urls.len()
        );
        Ok(())
    }

    /// Start listening for BitChat messages
    async fn start_listening(&self) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        // Subscribe to multiple event types for comprehensive BitChat coverage
        let subscription_id = SubscriptionId::new("bitchat");
        
        // Filter 1: Custom BitChat events (public)
        let bitchat_filter = Filter::new()
            .kind(BITCHAT_KIND)
            .since(Timestamp::now());
            
        // Filter 2: Text notes with #bitchat hashtag (for discovery)
        let discovery_filter = Filter::new()
            .kind(Kind::TextNote)
            .hashtag("bitchat")
            .since(Timestamp::now());
            
        // Filter 3: Encrypted direct messages to us (private)
        let dm_filter = Filter::new()
            .kind(Kind::EncryptedDirectMessage)
            .pubkey(self.keys.public_key())
            .since(Timestamp::now());

        client.subscribe(vec![bitchat_filter, discovery_filter, dm_filter], None).await;
        let _subscription_id = subscription_id;

        // Handle incoming events
        let mut notifications = client.notifications();
        let packet_tx = self.packet_tx.clone();
        let peers: Arc<RwLock<HashMap<PeerId, PublicKey>>> = Arc::clone(&self.peers);
        let peer_cache: Arc<RwLock<Vec<PeerId>>> = Arc::clone(&self.peer_cache);
        let local_pubkey = self.keys.public_key();

        tokio::spawn(async move {
            while let Ok(notification) = notifications.recv().await {
                match notification {
                    RelayPoolNotification::Event { event, .. } => {
                        // Skip our own events
                        if event.pubkey == local_pubkey {
                            continue;
                        }

                        // Handle different event kinds
                        match event.kind {
                            kind if kind == BITCHAT_KIND => {
                                // Custom BitChat event - parse as structured message
                                if let Ok(bitchat_msg) = serde_json::from_str::<BitchatNostrMessage>(&event.content) {
                                    Self::process_bitchat_message(bitchat_msg, event.pubkey, &peers, &peer_cache, &packet_tx).await;
                                }
                            }
                            Kind::TextNote => {
                                // Text note - try to parse for discovery
                                if event.content.contains("#bitchat") {
                                    if let Ok(bitchat_msg) = serde_json::from_str::<BitchatNostrMessage>(&event.content) {
                                        Self::process_bitchat_message(bitchat_msg, event.pubkey, &peers, &peer_cache, &packet_tx).await;
                                    }
                                }
                            }
                            Kind::EncryptedDirectMessage => {
                                // Encrypted DM - decrypt and process
                                // Note: nostr-sdk handles decryption automatically for events directed to us
                                if let Ok(bitchat_msg) = serde_json::from_str::<BitchatNostrMessage>(&event.content) {
                                    Self::process_bitchat_message(bitchat_msg, event.pubkey, &peers, &peer_cache, &packet_tx).await;
                                }
                            }
                            _ => {
                                // Unknown event type - ignore
                                debug!("Received unknown event kind: {:?}", event.kind);
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
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        // Check if we know the Nostr public key for this peer
        let target_pubkey = {
            let peers = self.peers.read().await;
            peers.get(peer_id).copied()
        };

        let bitchat_msg = BitchatNostrMessage::new(self.local_peer_id, Some(*peer_id), packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e))
        })?;

        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket(
                "Message too large for Nostr".into(),
            ));
        }

        let event = if let Some(pubkey) = target_pubkey {
            // Send encrypted direct message
            EventBuilder::encrypted_direct_msg(&self.keys, pubkey, content, None)
                .map_err(|e| {
                    BitchatError::InvalidPacket(format!(
                        "Failed to create encrypted message: {}",
                        e
                    ))
                })?
                .to_event(&self.keys)
                .map_err(|e| BitchatError::InvalidPacket(format!("Failed to sign event: {}", e)))?
        } else {
            // Send as custom BitChat event for discovery
            EventBuilder::new(BITCHAT_KIND, content, vec![Tag::hashtag("bitchat")])
                .to_event(&self.keys)
                .map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to create event: {}", e))
                })?
        };

        client
            .send_event(event)
            .await
            .map_err(|e| BitchatError::InvalidPacket(format!("Failed to send event: {}", e)))?;

        debug!("Sent BitChat packet to peer {} via Nostr", peer_id);
        Ok(())
    }

    /// Broadcast a BitChat packet to all peers via Nostr
    async fn broadcast_packet(&self, packet: &BitchatPacket) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        let bitchat_msg = BitchatNostrMessage::new(self.local_peer_id, None, packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e))
        })?;

        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket(
                "Message too large for Nostr".into(),
            ));
        }

        let event = EventBuilder::new(BITCHAT_KIND, content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| BitchatError::InvalidPacket(format!("Failed to create event: {}", e)))?;

        client
            .send_event(event)
            .await
            .map_err(|e| BitchatError::InvalidPacket(format!("Failed to send event: {}", e)))?;

        debug!("Broadcast BitChat packet via Nostr");
        Ok(())
    }

    /// Get our Nostr public key
    pub fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    /// Get our Nostr private key
    pub fn secret_key(&self) -> Result<&SecretKey, nostr_sdk::key::Error> {
        self.keys.secret_key()
    }

    /// Process a BitChat message from any event type
    async fn process_bitchat_message(
        bitchat_msg: BitchatNostrMessage,
        sender_pubkey: PublicKey,
        peers: &Arc<RwLock<HashMap<PeerId, PublicKey>>>,
        peer_cache: &Arc<RwLock<Vec<PeerId>>>,
        packet_tx: &mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    ) {
        // Check if this message is for us or is a broadcast
        if bitchat_msg.recipient_peer_id.is_none() {
            // Update peer mapping and cache
            {
                let mut peers_lock = peers.write().await;
                if !peers_lock.contains_key(&bitchat_msg.sender_peer_id) {
                    peers_lock.insert(bitchat_msg.sender_peer_id, sender_pubkey);
                    
                    // Update cached peer list
                    let mut cached = peer_cache.write().await;
                    if !cached.contains(&bitchat_msg.sender_peer_id) {
                        cached.push(bitchat_msg.sender_peer_id);
                    }
                }
            }

            // Forward packet
            if let Ok(packet) = bitchat_msg.to_packet() {
                if let Err(_) = packet_tx.send((bitchat_msg.sender_peer_id, packet)) {
                    error!("Failed to forward packet - receiver dropped");
                }
            }
        }
    }
}

impl Transport for NostrTransport {
    fn send_to(
        &mut self,
        peer_id: PeerId,
        packet: BitchatPacket,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move { self.send_to_peer(&peer_id, &packet).await })
    }

    fn broadcast(
        &mut self,
        packet: BitchatPacket,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move { self.broadcast_packet(&packet).await })
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
        // Use the cached peer list for non-blocking access
        if let Ok(cached_peers) = self.peer_cache.try_read() {
            cached_peers.clone()
        } else {
            // Fallback to empty list if lock is contended
            Vec::new()
        }
    }

    fn start(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.active.store(true, std::sync::atomic::Ordering::Relaxed);

            // Initialize Nostr client
            self.initialize_client().await?;

            // Start listening for messages
            self.start_listening().await?;

            info!("Nostr transport started");
            Ok(())
        })
    }

    fn stop(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitchatResult<()>> + Send + '_>> {
        Box::pin(async move {
            self.active.store(false, std::sync::atomic::Ordering::Relaxed);

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
        self.active.load(std::sync::atomic::Ordering::Relaxed)
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

        let packet = BitchatPacket::new(MessageType::Message, sender_id, b"test message".to_vec());

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
