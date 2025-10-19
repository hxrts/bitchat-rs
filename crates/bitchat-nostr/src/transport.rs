//! Nostr transport task implementation for BitChat hybrid architecture


use async_trait::async_trait;

// Native imports
#[cfg(not(target_arch = "wasm32"))]
use nostr_sdk::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use nostr_sdk::{Client, Event as NostrEvent, EventBuilder, Filter, Keys, RelayPoolNotification, Timestamp};

// WASM stub types
#[cfg(target_arch = "wasm32")]
pub struct Client;
#[cfg(target_arch = "wasm32")]
pub struct Keys;
#[cfg(target_arch = "wasm32")]
pub struct NostrEvent;
#[cfg(target_arch = "wasm32")]
pub struct EventBuilder;
#[cfg(target_arch = "wasm32")]
pub struct Filter;
#[cfg(target_arch = "wasm32")]
pub struct RelayPoolNotification;
#[cfg(target_arch = "wasm32")]
pub struct Timestamp;
#[cfg(feature = "std")]
use tokio::select;
#[cfg(feature = "std")]
use tokio::time::{interval, sleep, Duration};
#[cfg(feature = "wasm")]
use wasm_bindgen_futures::spawn_local;
use tracing::{debug, error, info, warn};

use bitchat_core::{
    PeerId,
    Effect, ChannelTransportType,
    EventSender, EffectReceiver,
    TransportTask,
    Result as BitchatResult, BitchatError, Event,
};
use bitchat_core::internal::TransportError;

use super::config::NostrConfig;
use super::error::NostrTransportError;
use super::message::{BitchatNostrMessage, BITCHAT_KIND};

#[cfg(feature = "std")]
async fn forward_event(sender: &EventSender, event: Event) -> Result<(), String> {
    sender.try_send(event).map_err(|e| e.to_string())
}

#[cfg(all(feature = "wasm", not(feature = "std")))]
async fn forward_event(sender: &EventSender, event: Event) -> Result<(), String> {
    let mut sender = sender.clone();
    sender
        .try_send(event)
        .map_err(|e| format!("{:?}", e))
}

#[cfg(not(any(feature = "std", feature = "wasm")))]
async fn forward_event(_sender: &EventSender, _event: Event) -> Result<(), String> {
    Err("No event channel backend configured".to_string())
}

// ----------------------------------------------------------------------------
// Nostr Transport Task
// ----------------------------------------------------------------------------

/// Nostr transport task implementation using CSP channels
pub struct NostrTransportTask {
    /// Transport identification
    transport_type: ChannelTransportType,
    /// Channel for sending events to Core Logic
    event_sender: Option<EventSender>,
    /// Channel for receiving effects from Core Logic
    effect_receiver: Option<EffectReceiver>,
    /// Task configuration
    config: NostrConfig,
    /// Our Nostr identity keys
    keys: Keys,
    /// Our BitChat peer ID (derived from keys)
    local_peer_id: Option<PeerId>,
    /// Nostr client instance
    client: Option<Client>,
}

impl NostrTransportTask {
    /// Create new Nostr transport task
    pub fn new(config: NostrConfig) -> BitchatResult<Self> {
        let keys = config.private_key.clone().unwrap_or_else(Keys::generate);
        
        Ok(Self {
            transport_type: ChannelTransportType::Nostr,
            event_sender: None,
            effect_receiver: None,
            config,
            keys,
            local_peer_id: None,
            client: None,
        })
    }

    /// Set the local peer ID (called by Core Logic during initialization)
    pub fn set_local_peer_id(&mut self, peer_id: PeerId) {
        self.local_peer_id = Some(peer_id);
    }

    /// Main task loop processing effects from Core Logic
    pub async fn run_internal(&mut self) -> BitchatResult<()> {
        info!("Starting Nostr transport task");

        // Initialize Nostr client
        self.initialize_client().await?;

        // Start listening for Nostr events
        self.start_listening().await?;

        let mut effect_receiver = self.effect_receiver.take().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Nostr transport started without attached effect receiver".to_string(),
            })
        })?;
        
#[cfg(feature = "std")]
        {
            // Reconnection interval (tokio-based)
            let mut reconnect_timer = interval(self.config.reconnect_interval);

            loop {
                select! {
                    // Process effects from Core Logic
                    effect_result = effect_receiver.recv() => {
                        match effect_result {
                            Ok(effect) => {
                                if let Err(e) = self.handle_effect(effect).await {
                                    error!("Failed to handle effect: {}", e);
                                    tracing::error!("Effect handling failed: {}", e);
                                }
                            }
                            Err(_) => {
                                info!("Effect channel closed, shutting down Nostr transport task");
                                break;
                            }
                        }
                    }
                    
                    // Periodic reconnection check
                    _ = reconnect_timer.tick() => {
                        if self.config.auto_reconnect {
                            self.check_and_reconnect().await;
                        }
                    }
                }
            }
        }

        #[cfg(feature = "wasm")]
        {
            // WASM-compatible event loop (no tokio::select!)
            loop {
                // Process effects from Core Logic
                if let Ok(effect) = effect_receiver.recv().await {
                    if let Err(e) = self.handle_effect(effect).await {
                        error!("Failed to handle effect: {}", e);
                        tracing::error!("Effect handling failed: {}", e);
                    }
                } else {
                    info!("Effect channel closed, shutting down Nostr transport task");
                    break;
                }

                // TODO: Add periodic connection checks for WASM
            }
        }
        
        info!("Nostr transport task stopped");
        Ok(())
    }

    /// Initialize Nostr client and connect to relays
    async fn initialize_client(&mut self) -> BitchatResult<()> {
        let client = Client::new(&self.keys);

        // Add all configured relays
        for relay_config in &self.config.relays {
            match Url::parse(&relay_config.url) {
                Ok(url) => {
                    if let Err(e) = client.add_relay(url).await {
                        warn!("Failed to add relay {}: {}", relay_config.url, e);
                    } else {
                        debug!("Added relay: {}", relay_config.url);
                    }
                }
                Err(_) => {
                    return Err(NostrTransportError::InvalidRelayUrl {
                        url: relay_config.url.clone(),
                    }.into());
                }
            }
        }

        // Connect to relays
        client.connect().await;
        
        // Wait for initial connections
        #[cfg(feature = "std")]
        sleep(Duration::from_secs(2)).await;
        #[cfg(feature = "wasm")]
        {
            // WASM: Skip the delay for now - relays should connect quickly in browser
            // In a production WASM implementation, we'd use web_sys::window().set_timeout()
            // but for simplicity, we'll proceed without the delay
        }

        self.client = Some(client);
        info!("Nostr client initialized with {} relays", self.config.relays.len());
        
        Ok(())
    }

    /// Start listening for BitChat messages on Nostr
    async fn start_listening(&self) -> BitchatResult<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        let event_sender = self.event_sender.as_ref().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Nostr transport missing event sender".to_string(),
            })
        })?.clone();

        // Subscribe to BitChat events
        let subscription_filters = vec![
            // Custom BitChat events (public)
            Filter::new().kind(BITCHAT_KIND).since(Timestamp::now()),
            // Encrypted direct messages to us (private)
            Filter::new()
                .kind(Kind::EncryptedDirectMessage)
                .pubkey(self.keys.public_key())
                .since(Timestamp::now()),
        ];

        client.subscribe(subscription_filters, None).await;

        // Handle incoming events
        let mut notifications = client.notifications();
        let local_peer_id = self.local_peer_id;
        let local_pubkey = self.keys.public_key();

        #[cfg(feature = "std")]
        {
            tokio::spawn(async move {
                while let Ok(notification) = notifications.recv().await {
                    match notification {
                        RelayPoolNotification::Event { event, .. } => {
                            // Skip our own events
                            if event.pubkey == local_pubkey {
                                continue;
                            }

                            // Process BitChat events
                            if let Err(e) = Self::process_nostr_event(
                                &event,
                                &event_sender,
                                local_peer_id,
                            ).await {
                                debug!("Failed to process Nostr event: {}", e);
                            }
                        }
                        RelayPoolNotification::Shutdown => {
                            info!("Nostr relay pool shutdown");
                            break;
                        }
                        _ => {}
                    }
                }
            });
        }

        #[cfg(feature = "wasm")]
        {
            // WASM-compatible version using wasm-bindgen-futures::spawn_local
            spawn_local(async move {
                while let Ok(notification) = notifications.recv().await {
                    match notification {
                        RelayPoolNotification::Event { event, .. } => {
                            // Skip our own events
                            if event.pubkey == local_pubkey {
                                continue;
                            }

                            // Process BitChat events
                            if let Err(e) = Self::process_nostr_event(
                                &event,
                                &event_sender,
                                local_peer_id,
                            ).await {
                                debug!("Failed to process Nostr event: {}", e);
                            }
                        }
                        RelayPoolNotification::Shutdown => {
                            info!("Nostr relay pool shutdown");
                            break;
                        }
                        _ => {}
                    }
                }
            });
        }

        info!("Started listening for BitChat messages on Nostr");
        Ok(())
    }

    /// Process a Nostr event and potentially send events to Core Logic
    async fn process_nostr_event(
        event: &NostrEvent,
        event_sender: &EventSender,
        local_peer_id: Option<PeerId>,
    ) -> Result<(), NostrTransportError> {
        // Only process relevant event kinds
        if event.kind != BITCHAT_KIND && event.kind != Kind::EncryptedDirectMessage {
            return Ok(());
        }

        // Try to parse as BitChat message
        let bitchat_msg = BitchatNostrMessage::from_nostr_content(&event.content)?;
        
        // Check if message is for us
        let is_for_us = match local_peer_id {
            Some(our_peer_id) => bitchat_msg.is_for_peer(&our_peer_id) || bitchat_msg.is_broadcast(),
            None => bitchat_msg.is_broadcast(), // Accept broadcasts if we don't know our peer ID yet
        };

        if !is_for_us {
            return Ok(());
        }

        // Send peer discovery event to Core Logic (let Core Logic manage discovered state)
        #[allow(unused_variables)] // Used in feature-gated code below
        let discovery_event = bitchat_core::Event::PeerDiscovered {
            peer_id: bitchat_msg.sender_peer_id,
            transport: ChannelTransportType::Nostr,
            signal_strength: None, // Nostr doesn't have signal strength
        };

        if let Err(e) = forward_event(event_sender, discovery_event).await {
            warn!("Failed to send peer discovery event: {}", e);
            tracing::warn!("Failed to send peer discovery event: {}", e);
        }

        // Extract message data and send to Core Logic
        let data = bitchat_msg.to_data()?;
        #[allow(unused_variables)] // Used in feature-gated code below
        let message_event = bitchat_core::Event::MessageReceived {
            from: bitchat_msg.sender_peer_id,
            content: data.clone().into(),
            transport: ChannelTransportType::Nostr,
            message_id: None,
            recipient: bitchat_msg.recipient_peer_id,
            timestamp: Some(bitchat_msg.timestamp),
            sequence: None,
        };

        if let Err(e) = forward_event(event_sender, message_event).await {
            warn!("Failed to send message event: {}", e);
            tracing::warn!("Failed to send message event: {}", e);
        }

        Ok(())
    }

    /// Handle effects from Core Logic
    async fn handle_effect(&self, effect: Effect) -> BitchatResult<()> {
        match effect {
            Effect::SendPacket { peer_id, data, transport } => {
                if transport == self.transport_type {
                    self.send_data_to_peer(peer_id, data.to_vec()).await?;
                }
            }
            Effect::StartTransportDiscovery { transport } => {
                if transport == self.transport_type {
                    self.start_discovery().await?;
                }
            }
            Effect::StopTransportDiscovery { transport } => {
                if transport == self.transport_type {
                    self.stop_discovery().await?;
                }
            }
            Effect::InitiateConnection { peer_id, transport } => {
                if transport == self.transport_type {
                    self.initiate_connection(peer_id).await?;
                }
            }
            _ => {
                // Ignore effects not relevant to Nostr transport
            }
        }
        Ok(())
    }

    /// Send data to a specific peer via Nostr
    async fn send_data_to_peer(&self, peer_id: PeerId, data: Vec<u8>) -> BitchatResult<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        let local_peer_id = self.local_peer_id.ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        // Check data size
        if data.len() > self.config.max_data_size {
            return Err(NostrTransportError::MessageTooLarge {
                size: data.len(),
                max_size: self.config.max_data_size,
            }.into());
        }

        // Create BitChat message
        let bitchat_msg = BitchatNostrMessage::new(local_peer_id, Some(peer_id), data);
        let content = bitchat_msg.to_nostr_content()?;

        // For now, send all messages as public BitChat events
        // In a future enhancement, Core Logic could maintain peer->pubkey mappings
        // and provide them through effects for encrypted messaging
        let event = EventBuilder::new(BITCHAT_KIND, content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;

        client.send_event(event).await
            .map_err(NostrTransportError::EventSendFailed)?;

        debug!("Sent data to peer {} via Nostr", peer_id);
        Ok(())
    }

    /// Start discovery (already handled by subscriptions)
    async fn start_discovery(&self) -> BitchatResult<()> {
        info!("Nostr discovery is always active via subscriptions");
        Ok(())
    }

    /// Stop discovery
    async fn stop_discovery(&self) -> BitchatResult<()> {
        info!("Nostr discovery stop requested (but stays active via subscriptions)");
        Ok(())
    }

    /// Initiate connection to a peer (for Nostr, this is a no-op)
    async fn initiate_connection(&self, peer_id: PeerId) -> BitchatResult<()> {
        debug!("Connection initiation requested for peer {} (Nostr is connectionless)", peer_id);
        Ok(())
    }

    /// Check relay connections and reconnect if needed
    async fn check_and_reconnect(&self) {
        if let Some(_client) = &self.client {
            // Check relay status and reconnect if needed
            // This is a simplified implementation - nostr-sdk handles most reconnection logic
            debug!("Checking relay connections");
            
            // The nostr-sdk client automatically handles reconnections
            // We could add custom logic here to monitor specific relays
        }
    }
}

#[async_trait]
impl TransportTask for NostrTransportTask {
    fn attach_channels(
        &mut self,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<()> {
        if self.event_sender.is_some() || self.effect_receiver.is_some() {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Nostr transport channels already attached".to_string(),
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
