//! Nostr transport implementation

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use ::url::Url;
use async_trait::async_trait;
use smallvec::SmallVec;

use bitchat_core::transport::{
    LatencyClass, ReliabilityClass, Transport, TransportCapabilities, TransportType,
};
use bitchat_core::{BitchatError, BitchatPacket, PeerId, Result as BitchatResult};
use nostr_sdk::prelude::*;
use nostr_sdk::{
    Client, EventBuilder, Filter, Keys, Kind, PublicKey, RelayPoolNotification, SecretKey,
    SubscriptionId, Tag, Timestamp,
};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info};

use super::config::NostrTransportConfig;
use super::error::NostrTransportError;
use super::message::{BitchatNostrMessage, BITCHAT_KIND};

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
        let keys = config.private_key.clone().unwrap_or_else(Keys::generate);

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
        let bitchat_filter = Filter::new().kind(BITCHAT_KIND).since(Timestamp::now());

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

        client
            .subscribe(vec![bitchat_filter, discovery_filter, dm_filter], None)
            .await;
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
                                if let Ok(bitchat_msg) =
                                    serde_json::from_str::<BitchatNostrMessage>(&event.content)
                                {
                                    Self::process_bitchat_message(
                                        bitchat_msg,
                                        event.pubkey,
                                        &peers,
                                        &peer_cache,
                                        &packet_tx,
                                    )
                                    .await;
                                }
                            }
                            Kind::TextNote => {
                                // Text note - try to parse for discovery
                                if event.content.contains("#bitchat") {
                                    if let Ok(bitchat_msg) =
                                        serde_json::from_str::<BitchatNostrMessage>(&event.content)
                                    {
                                        Self::process_bitchat_message(
                                            bitchat_msg,
                                            event.pubkey,
                                            &peers,
                                            &peer_cache,
                                            &packet_tx,
                                        )
                                        .await;
                                    }
                                }
                            }
                            Kind::EncryptedDirectMessage => {
                                // Encrypted DM - decrypt and process
                                // Note: nostr-sdk handles decryption automatically for events directed to us
                                if let Ok(bitchat_msg) =
                                    serde_json::from_str::<BitchatNostrMessage>(&event.content)
                                {
                                    Self::process_bitchat_message(
                                        bitchat_msg,
                                        event.pubkey,
                                        &peers,
                                        &peer_cache,
                                        &packet_tx,
                                    )
                                    .await;
                                }
                            }
                            _ => {
                                // Unknown event type - ignore
                                debug!("Received unknown event kind: {:?}", event.kind);
                            }
                        }
                    }
                    RelayPoolNotification::Message { .. } => {
                        debug!("Received relay message");
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
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e).into())
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
                    BitchatError::InvalidPacket(
                        format!("Failed to create encrypted message: {}", e).into(),
                    )
                })?
                .to_event(&self.keys)
                .map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to sign event: {}", e).into())
                })?
        } else {
            // Send as custom BitChat event for discovery
            EventBuilder::new(BITCHAT_KIND, content, vec![Tag::hashtag("bitchat")])
                .to_event(&self.keys)
                .map_err(|e| {
                    BitchatError::InvalidPacket(format!("Failed to create event: {}", e).into())
                })?
        };

        client.send_event(event).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to send event: {}", e).into())
        })?;

        debug!(
            "Sent BitChat packet to peer {}... via Nostr",
            &peer_id.to_string()[..8]
        );
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
            BitchatError::InvalidPacket(format!("Failed to serialize message: {}", e).into())
        })?;

        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket(
                "Message too large for Nostr".into(),
            ));
        }

        let event = EventBuilder::new(BITCHAT_KIND, content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to create event: {}", e).into())
            })?;

        client.send_event(event).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to send event: {}", e).into())
        })?;

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
                if let std::collections::hash_map::Entry::Vacant(e) =
                    peers_lock.entry(bitchat_msg.sender_peer_id)
                {
                    e.insert(sender_pubkey);

                    // Update cached peer list
                    let mut cached = peer_cache.write().await;
                    if !cached.contains(&bitchat_msg.sender_peer_id) {
                        cached.push(bitchat_msg.sender_peer_id);
                    }
                }
            }

            // Forward packet
            if let Ok(packet) = bitchat_msg.to_packet() {
                if packet_tx
                    .send((bitchat_msg.sender_peer_id, packet))
                    .is_err()
                {
                    error!("Failed to forward packet - receiver dropped");
                }
            }
        }
    }
}

#[async_trait]
impl Transport for NostrTransport {
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> BitchatResult<()> {
        self.send_to_peer(&peer_id, &packet).await
    }

    async fn broadcast(&mut self, packet: BitchatPacket) -> BitchatResult<()> {
        self.broadcast_packet(&packet).await
    }

    async fn receive(&mut self) -> BitchatResult<(PeerId, BitchatPacket)> {
        let mut rx = self.packet_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| BitchatError::InvalidPacket("Receive channel closed".into()))
    }

    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        // Use the cached peer list for non-blocking access
        if let Ok(cached_peers) = self.peer_cache.try_read() {
            SmallVec::from_vec(cached_peers.clone())
        } else {
            // Fallback to empty list if lock is contended
            SmallVec::new()
        }
    }

    async fn start(&mut self) -> BitchatResult<()> {
        self.active
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Initialize Nostr client
        self.initialize_client().await?;

        // Start listening for messages
        self.start_listening().await?;

        info!("Nostr transport started");
        Ok(())
    }

    async fn stop(&mut self) -> BitchatResult<()> {
        self.active
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // Disconnect from relays
        if let Some(client) = &self.client {
            client.disconnect().await.map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to disconnect: {}", e).into())
            })?;
        }

        info!("Nostr transport stopped");
        Ok(())
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
