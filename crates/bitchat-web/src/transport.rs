//! WASM-compatible Nostr transport implementation

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bitchat_core::{
    transport::{LatencyClass, ReliabilityClass, Transport, TransportCapabilities, TransportType},
    BitchatError, BitchatPacket, PeerId, Result as BitchatResult,
};
use nostr_sdk::base64::{engine::general_purpose, Engine as _};
use nostr_sdk::prelude::*;
use smallvec::SmallVec;
use tokio::sync::{mpsc, Mutex, RwLock};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::utils::console_log;

/// WASM-compatible Nostr transport configuration
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmNostrConfig {
    relay_urls: Vec<String>,
    connection_timeout_ms: u32,
    #[allow(dead_code)]
    message_timeout_ms: u32,
    max_packet_size: usize,
}

#[wasm_bindgen]
impl WasmNostrConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            relay_urls: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            connection_timeout_ms: 10000,
            message_timeout_ms: 30000,
            max_packet_size: 64000,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_relay_urls(&mut self, urls: Vec<String>) {
        self.relay_urls = urls;
    }

    #[wasm_bindgen(getter)]
    pub fn relay_urls(&self) -> Vec<String> {
        self.relay_urls.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_connection_timeout_ms(&mut self, timeout: u32) {
        self.connection_timeout_ms = timeout;
    }

    #[wasm_bindgen(getter)]
    pub fn connection_timeout_ms(&self) -> u32 {
        self.connection_timeout_ms
    }
}

impl Default for WasmNostrConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// BitChat message wrapper for Nostr events (WASM version)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WasmBitchatNostrMessage {
    pub sender_peer_id: PeerId,
    pub recipient_peer_id: Option<PeerId>,
    pub packet_data: String, // base64 encoded
    pub timestamp: u64,
}

impl WasmBitchatNostrMessage {
    pub fn new(
        sender_peer_id: PeerId,
        recipient_peer_id: Option<PeerId>,
        packet: &BitchatPacket,
    ) -> Result<Self, BitchatError> {
        let packet_bytes = bincode::serialize(packet).map_err(BitchatError::Serialization)?;
        let packet_data = general_purpose::STANDARD.encode(packet_bytes);

        Ok(Self {
            sender_peer_id,
            recipient_peer_id,
            packet_data,
            timestamp: js_sys::Date::now() as u64,
        })
    }

    pub fn to_packet(&self) -> Result<BitchatPacket, BitchatError> {
        let packet_bytes = general_purpose::STANDARD
            .decode(&self.packet_data)
            .map_err(|e| BitchatError::Transport {
                message: format!("Base64 decode error: {}", e),
            })?;
        bincode::deserialize(&packet_bytes).map_err(BitchatError::Serialization)
    }
}

/// WASM-compatible Nostr transport
pub struct WasmNostrTransport {
    config: WasmNostrConfig,
    client: Option<Client>,
    keys: Keys,
    local_peer_id: PeerId,
    peers: Arc<RwLock<HashMap<PeerId, PublicKey>>>,
    peer_cache: Arc<RwLock<Vec<PeerId>>>,
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<(PeerId, BitchatPacket)>>>,
    packet_tx: mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    active: Arc<RwLock<bool>>,
}

impl WasmNostrTransport {
    pub fn new(local_peer_id: PeerId) -> BitchatResult<Self> {
        Self::with_config(local_peer_id, WasmNostrConfig::default())
    }

    pub fn with_config(local_peer_id: PeerId, config: WasmNostrConfig) -> BitchatResult<Self> {
        let keys = Keys::generate();
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
            active: Arc::new(RwLock::new(false)),
        })
    }

    async fn initialize_client(&mut self) -> BitchatResult<()> {
        console_log!("Initializing Nostr client for WASM...");

        let client = Client::new(&self.keys);

        // Add relays
        for relay_url in &self.config.relay_urls {
            console_log!("Adding relay: {}", relay_url);
            if let Ok(url) = url::Url::parse(relay_url) {
                client
                    .add_relay(url)
                    .await
                    .map_err(|e| BitchatError::Transport {
                        message: format!("Failed to add relay {}: {}", relay_url, e),
                    })?;
            } else {
                return Err(BitchatError::Transport {
                    message: format!("Invalid relay URL: {}", relay_url),
                });
            }
        }

        // Connect to relays
        console_log!("Connecting to Nostr relays...");
        client.connect().await;

        // Wait for connections
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        self.client = Some(client);
        console_log!(
            "Nostr client initialized with {} relays",
            self.config.relay_urls.len()
        );
        Ok(())
    }

    async fn start_listening(&self) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        console_log!("Starting to listen for BitChat messages...");

        // Subscribe to BitChat events
        let _subscription_id = SubscriptionId::new("bitchat-wasm");

        let bitchat_filter = Filter::new()
            .kind(Kind::Custom(30420)) // BITCHAT_KIND
            .since(Timestamp::now());

        client.subscribe(vec![bitchat_filter], None).await;

        // Handle incoming events
        let mut notifications = client.notifications();
        let packet_tx = self.packet_tx.clone();
        let peers = Arc::clone(&self.peers);
        let peer_cache = Arc::clone(&self.peer_cache);
        let local_pubkey = self.keys.public_key();

        spawn_local(async move {
            console_log!("Event listener spawned");
            while let Ok(notification) = notifications.recv().await {
                match notification {
                    RelayPoolNotification::Event { event, .. } => {
                        if event.pubkey == local_pubkey {
                            continue;
                        }

                        if event.kind == Kind::Custom(30420) {
                            if let Ok(bitchat_msg) =
                                serde_json::from_str::<WasmBitchatNostrMessage>(&event.content)
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
                    RelayPoolNotification::Shutdown => {
                        console_log!("Nostr relay pool shutdown");
                        break;
                    }
                    _ => {}
                }
            }
        });

        console_log!("Started listening for BitChat messages on Nostr");
        Ok(())
    }

    async fn process_bitchat_message(
        bitchat_msg: WasmBitchatNostrMessage,
        sender_pubkey: PublicKey,
        peers: &Arc<RwLock<HashMap<PeerId, PublicKey>>>,
        peer_cache: &Arc<RwLock<Vec<PeerId>>>,
        packet_tx: &mpsc::UnboundedSender<(PeerId, BitchatPacket)>,
    ) {
        // Update peer mapping and cache
        {
            let mut peers_lock = peers.write().await;
            if let std::collections::hash_map::Entry::Vacant(e) =
                peers_lock.entry(bitchat_msg.sender_peer_id)
            {
                e.insert(sender_pubkey);

                let mut cached = peer_cache.write().await;
                if !cached.contains(&bitchat_msg.sender_peer_id) {
                    cached.push(bitchat_msg.sender_peer_id);
                }

                console_log!("New peer discovered: {}", bitchat_msg.sender_peer_id);
            }
        }

        // Forward packet
        if let Ok(packet) = bitchat_msg.to_packet() {
            if packet_tx
                .send((bitchat_msg.sender_peer_id, packet))
                .is_err()
            {
                console_log!("Failed to forward packet - receiver dropped");
            }
        }
    }

    async fn send_to_peer(&self, peer_id: &PeerId, packet: &BitchatPacket) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        let bitchat_msg = WasmBitchatNostrMessage::new(self.local_peer_id, Some(*peer_id), packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| BitchatError::Transport {
            message: format!("Failed to serialize message: {}", e),
        })?;

        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket(
                "Message too large for Nostr".into(),
            ));
        }

        // Send as custom BitChat event
        let event = EventBuilder::new(Kind::Custom(30420), content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| BitchatError::Transport {
                message: format!("Failed to create event: {}", e),
            })?;

        client
            .send_event(event)
            .await
            .map_err(|e| BitchatError::Transport {
                message: format!("Failed to send event: {}", e),
            })?;

        console_log!("Sent BitChat packet to peer {} via Nostr", peer_id);
        Ok(())
    }

    async fn broadcast_packet(&self, packet: &BitchatPacket) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| BitchatError::InvalidPacket("Nostr client not initialized".into()))?;

        let bitchat_msg = WasmBitchatNostrMessage::new(self.local_peer_id, None, packet)?;
        let content = serde_json::to_string(&bitchat_msg).map_err(|e| BitchatError::Transport {
            message: format!("Failed to serialize message: {}", e),
        })?;

        if content.len() > self.config.max_packet_size {
            return Err(BitchatError::InvalidPacket(
                "Message too large for Nostr".into(),
            ));
        }

        let event = EventBuilder::new(Kind::Custom(30420), content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| BitchatError::Transport {
                message: format!("Failed to create event: {}", e),
            })?;

        client
            .send_event(event)
            .await
            .map_err(|e| BitchatError::Transport {
                message: format!("Failed to send event: {}", e),
            })?;

        console_log!("Broadcast BitChat packet via Nostr");
        Ok(())
    }
}

#[async_trait]
impl Transport for WasmNostrTransport {
    async fn send_to(&mut self, peer_id: PeerId, packet: BitchatPacket) -> BitchatResult<()> {
        self.send_to_peer(&peer_id, &packet).await
    }

    async fn broadcast(&mut self, packet: BitchatPacket) -> BitchatResult<()> {
        self.broadcast_packet(&packet).await
    }

    async fn receive(&mut self) -> BitchatResult<(PeerId, BitchatPacket)> {
        let mut rx = self.packet_rx.lock().await;
        rx.recv().await.ok_or_else(|| BitchatError::Transport {
            message: "Receive channel closed".to_string(),
        })
    }

    fn discovered_peers(&self) -> SmallVec<[PeerId; 8]> {
        // Use cached peers for non-blocking access
        if let Ok(cached_peers) = self.peer_cache.try_read() {
            SmallVec::from_vec(cached_peers.clone())
        } else {
            SmallVec::new()
        }
    }

    async fn start(&mut self) -> BitchatResult<()> {
        *self.active.write().await = true;

        self.initialize_client().await?;
        self.start_listening().await?;

        console_log!("WASM Nostr transport started");
        Ok(())
    }

    async fn stop(&mut self) -> BitchatResult<()> {
        *self.active.write().await = false;

        if let Some(client) = &self.client {
            client
                .disconnect()
                .await
                .map_err(|e| BitchatError::Transport {
                    message: format!("Failed to disconnect: {}", e),
                })?;
        }

        console_log!("WASM Nostr transport stopped");
        Ok(())
    }

    fn is_active(&self) -> bool {
        // Simple non-blocking check for WASM
        self.active
            .try_read()
            .map(|active| *active)
            .unwrap_or(false)
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
