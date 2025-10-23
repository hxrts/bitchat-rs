//! Nostr transport task implementation for BitChat hybrid architecture

use async_trait::async_trait;
use tracing::{debug, error, info, warn};

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        // Native imports
        use nostr_sdk::prelude::*;
        use nostr_sdk::{Client, Event as NostrEvent, EventBuilder, Filter, Keys, RelayPoolNotification, Timestamp};
        use nostr_sdk::base64::{engine::general_purpose, Engine as _};
    } else {
        // WASM stub types
        pub struct Client;
        pub struct Keys;
        pub struct NostrEvent;
        pub struct EventBuilder;
        pub struct Filter;
        pub struct RelayPoolNotification;
        pub struct Timestamp;

        // WASM base64 stub - in a real implementation this would use web-sys
        mod general_purpose {
            pub struct STANDARD;
            impl STANDARD {
                pub fn decode(_data: &str) -> Result<Vec<u8>, String> {
                    Err("Base64 decode not implemented for WASM".to_string())
                }
                pub fn encode(_data: &[u8]) -> String {
                    "base64_stub".to_string()
                }
            }
        }
        use general_purpose::STANDARD;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use tokio::select;
        use tokio::time::{interval, sleep, Duration};
    } else if #[cfg(feature = "wasm")] {
        use wasm_bindgen_futures::spawn_local;
    }
}

use bitchat_core::internal::TransportError;
use bitchat_core::protocol::{BitchatPacket, WireFormat};
use bitchat_core::{
    BitchatError, EffectReceiver, EventSender, PeerId, Result as BitchatResult, TransportTask,
};
use bitchat_harness::{
    messages::{ChannelTransportType, Effect, Event},
    TransportHandle,
};

use super::config::NostrConfig;
use super::error::NostrTransportError;
use super::message::{BitchatNostrMessage, BITCHAT_KIND};
use super::nip17::{Nip17GiftUnwrapper, Nip17GiftWrapper};
use super::embedding::{EmbeddingStrategy, EmbeddingConfig, NostrEmbeddedBitChat};
// use super::relay_manager::{NostrRelayManager, RelaySelectionStrategy};

async fn forward_event(sender: &EventSender, event: Event) -> Result<(), String> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            sender.try_send(event).map_err(|e| e.to_string())
        } else if #[cfg(feature = "wasm")] {
            let mut sender = sender.clone();
            sender
                .try_send(event)
                .map_err(|e| format!("{:?}", e))
        } else {
            Err("No event channel backend configured".to_string())
        }
    }
}

// ----------------------------------------------------------------------------
// Nostr Transport Task
// ----------------------------------------------------------------------------

/// Nostr transport task implementation using CSP channels with canonical tunneling
pub struct NostrTransportTask {
    /// Transport identification
    transport_type: ChannelTransportType,
    /// Channels provided by the runtime harness
    channels: Option<TransportHandle>,
    /// Task configuration
    config: NostrConfig,
    /// Our Nostr identity keys
    keys: Keys,
    /// Our BitChat peer ID (derived from keys)
    local_peer_id: Option<PeerId>,
    /// Nostr client instance
    client: Option<Client>,
    /// NIP-17 gift wrapper for creating encrypted messages
    gift_wrapper: Option<Nip17GiftWrapper>,
    /// NIP-17 gift unwrapper for decrypting received messages
    gift_unwrapper: Option<Nip17GiftUnwrapper>,
    /// Canonical relay manager for health monitoring and selection (TODO: Add back with proper threading)
    // relay_manager: Option<NostrRelayManager<bitchat_core::types::SystemTimeSource>>,
    /// Embedding strategy for BitChat messages
    embedding_strategy: EmbeddingStrategy,
    /// Embedding configuration for privacy features
    embedding_config: EmbeddingConfig,
}

impl NostrTransportTask {
    /// Create new Nostr transport task
    pub fn new(config: NostrConfig) -> BitchatResult<Self> {
        let keys = config.private_key.clone().unwrap_or_else(Keys::generate);
        
        // TODO: Add relay manager back with proper threading support
        // #[cfg(not(target_arch = "wasm32"))]
        // let time_source = bitchat_core::types::SystemTimeSource;
        // #[cfg(target_arch = "wasm32")]
        // let time_source = bitchat_core::types::SystemTimeSource; // WASM also uses SystemTimeSource
        // 
        // let mut relay_manager = NostrRelayManager::new(time_source);
        // relay_manager.load_default_relays();

        Ok(Self {
            transport_type: ChannelTransportType::Nostr,
            channels: None,
            config,
            keys,
            local_peer_id: None,
            client: None,
            gift_wrapper: None,
            gift_unwrapper: None,
            // relay_manager: Some(relay_manager),
            embedding_strategy: EmbeddingStrategy::default(),
            embedding_config: EmbeddingConfig::default(),
        })
    }

    /// Set the local peer ID (called by Core Logic during initialization)
    pub fn set_local_peer_id(&mut self, peer_id: PeerId) {
        self.local_peer_id = Some(peer_id);
    }

    /// Update embedding configuration for privacy features
    pub fn set_embedding_config(&mut self, config: EmbeddingConfig) {
        self.embedding_config = config;
    }

    /// Get current embedding configuration
    pub fn embedding_config(&self) -> &EmbeddingConfig {
        &self.embedding_config
    }

    /// Main task loop processing effects from Core Logic
    pub async fn run_internal(&mut self) -> BitchatResult<()> {
        info!("Starting Nostr transport task");

        // Initialize Nostr client
        self.initialize_client().await?;

        // Start listening for Nostr events
        self.start_listening().await?;

        let channels = self.channels.as_mut().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Nostr transport started without harness channels".to_string(),
            })
        })?;
        let mut effect_receiver = channels.take_effect_receiver().ok_or_else(|| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Nostr transport already running".to_string(),
            })
        })?;

        cfg_if::cfg_if! {
            if #[cfg(feature = "std")] {
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
            } else if #[cfg(feature = "wasm")] {
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

                    // Periodic connection monitoring for WASM environments
                    cfg_if::cfg_if! {
                        if #[cfg(target_arch = "wasm32")] {
                            if let Some(client) = &self.client {
                                // Check connection status every 30 seconds in WASM
                                tokio::select! {
                                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                                        self.check_connection_health().await;
                                    }
                                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                                        // Short wait to prevent busy loop
                                    }
                                }
                            }
                        }
                    }
                }
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
                    }
                    .into());
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

        // Initialize NIP-17 wrappers
        self.gift_wrapper = Some(Nip17GiftWrapper::new(self.keys.clone()));
        self.gift_unwrapper = Some(Nip17GiftUnwrapper::new(self.keys.clone()));

        self.client = Some(client);
        info!(
            "Nostr client initialized with {} relays and NIP-17 support",
            self.config.relays.len()
        );

        Ok(())
    }

    /// Start listening for BitChat messages on Nostr
    async fn start_listening(&self) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        let event_sender = self
            .channels
            .as_ref()
            .ok_or_else(|| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: "Nostr transport missing event sender".to_string(),
                })
            })?
            .event_sender();

        // Subscribe to BitChat events
        let subscription_filters = vec![
            // Custom BitChat events (public)
            Filter::new().kind(BITCHAT_KIND).since(Timestamp::now()),
            // Encrypted direct messages to us (private, NIP-04)
            Filter::new()
                .kind(Kind::EncryptedDirectMessage)
                .pubkey(self.keys.public_key())
                .since(Timestamp::now()),
            // Gift-wrapped events (NIP-17) - we subscribe to all and try to decrypt
            Filter::new().kind(Kind::GiftWrap).since(Timestamp::now()),
        ];

        client.subscribe(subscription_filters, None).await;

        // Handle incoming events
        let mut notifications = client.notifications();
        let local_peer_id = self.local_peer_id;
        let local_pubkey = self.keys.public_key();

        #[cfg(feature = "std")]
        {
            let gift_unwrapper = self.gift_unwrapper.clone();
            tokio::spawn(async move {
                while let Ok(notification) = notifications.recv().await {
                    match notification {
                        RelayPoolNotification::Event { event, .. } => {
                            // Skip our own events
                            if event.pubkey == local_pubkey {
                                continue;
                            }

                            // Process BitChat events
                            if let Err(e) = Self::process_nostr_event_static(
                                &event,
                                &event_sender,
                                local_peer_id,
                                gift_unwrapper.as_ref(),
                            )
                            .await
                            {
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
            let gift_unwrapper = self.gift_unwrapper.clone();
            spawn_local(async move {
                while let Ok(notification) = notifications.recv().await {
                    match notification {
                        RelayPoolNotification::Event { event, .. } => {
                            // Skip our own events
                            if event.pubkey == local_pubkey {
                                continue;
                            }

                            // Process BitChat events
                            if let Err(e) = Self::process_nostr_event_static(
                                &event,
                                &event_sender,
                                local_peer_id,
                                gift_unwrapper.as_ref(),
                            )
                            .await
                            {
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

    /// Process a Nostr event and potentially send events to Core Logic (static version for spawned tasks)
    async fn process_nostr_event_static(
        event: &NostrEvent,
        event_sender: &EventSender,
        local_peer_id: Option<PeerId>,
        gift_unwrapper: Option<&Nip17GiftUnwrapper>,
    ) -> Result<(), NostrTransportError> {
        // Only process relevant event kinds
        if event.kind != BITCHAT_KIND
            && event.kind != Kind::EncryptedDirectMessage
            && event.kind != Kind::GiftWrap
        {
            return Ok(());
        }

        // Handle NIP-17 gift-wrapped events
        if event.kind == Kind::GiftWrap {
            if let Some(unwrapper) = gift_unwrapper {
                match unwrapper.unwrap_gift_wrapped_message(event) {
                    Ok(Some(content)) => {
                        // Process the unwrapped content
                        if let Ok(Some(packet)) = content.to_bitchat_packet() {
                            // Check if message is for us
                            let is_for_us = match local_peer_id {
                                Some(our_peer_id) => {
                                    packet.is_broadcast()
                                        || packet.recipient_id == Some(our_peer_id)
                                }
                                None => packet.is_broadcast(),
                            };

                            if !is_for_us {
                                return Ok(());
                            }

                            // Send peer discovery event
                            let discovery_event = bitchat_core::Event::PeerDiscovered {
                                peer_id: packet.sender_id,
                                transport: ChannelTransportType::Nostr,
                                signal_strength: None,
                            };

                            if let Err(e) = forward_event(event_sender, discovery_event).await {
                                warn!("Failed to send peer discovery event: {}", e);
                            }

                            // Send packet event
                            let packet_event = bitchat_core::Event::BitchatPacketReceived {
                                from: packet.sender_id,
                                packet,
                                transport: ChannelTransportType::Nostr,
                            };

                            if let Err(e) = forward_event(event_sender, packet_event).await {
                                warn!("Failed to send packet event: {}", e);
                            }

                            return Ok(());
                        }
                    }
                    Ok(None) => {
                        // Not a message for us, ignore
                        return Ok(());
                    }
                    Err(e) => {
                        debug!("Failed to unwrap NIP-17 message: {}", e);
                        return Ok(()); // Don't treat as error, might not be for us
                    }
                }
            }
            return Ok(()); // No unwrapper available
        }

        // Check if this is a canonical BitChat embedded message
        if NostrEmbeddedBitChat::is_bitchat_content(&event.content) {
            // Decode using canonical embedding
            match NostrEmbeddedBitChat::decode_from_nostr(&event.content) {
                Ok(Some(packet)) => {

                    // Check if message is for us
                    let is_for_us = match local_peer_id {
                        Some(our_peer_id) => {
                            packet.is_broadcast() || packet.recipient_id == Some(our_peer_id)
                        }
                        None => packet.is_broadcast(), // Accept broadcasts if we don't know our peer ID yet
                    };

                    if !is_for_us {
                        return Ok(());
                    }

                    // Send peer discovery event to Core Logic
                    let discovery_event = bitchat_core::Event::PeerDiscovered {
                        peer_id: packet.sender_id,
                        transport: ChannelTransportType::Nostr,
                        signal_strength: None, // Nostr doesn't have signal strength
                    };

                    if let Err(e) = forward_event(event_sender, discovery_event).await {
                        warn!("Failed to send peer discovery event: {}", e);
                    }

                    // Send BitchatPacket event to Core Logic
                    let packet_event = bitchat_core::Event::BitchatPacketReceived {
                        from: packet.sender_id,
                        packet,
                        transport: ChannelTransportType::Nostr,
                    };

                    if let Err(e) = forward_event(event_sender, packet_event).await {
                        warn!("Failed to send packet event: {}", e);
                    }
                }
                Ok(None) => {
                    // Not a BitChat message, ignore
                    return Ok(());
                }
                Err(e) => {
                    debug!("Failed to decode canonical BitChat message: {}", e);
                    return Ok(()); // Don't treat as error, might be non-BitChat content
                }
            }
        } else {
            // Legacy format - try to parse as BitChat message
            let bitchat_msg = BitchatNostrMessage::from_nostr_content(&event.content)?;

            // Check if message is for us
            let is_for_us = match local_peer_id {
                Some(our_peer_id) => {
                    bitchat_msg.is_for_peer(&our_peer_id) || bitchat_msg.is_broadcast()
                }
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
            let content = String::from_utf8_lossy(&data).to_string();
            #[allow(unused_variables)] // Used in feature-gated code below
            let message_event = bitchat_core::Event::MessageReceived {
                from: bitchat_msg.sender_peer_id,
                content,
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
        }

        Ok(())
    }

    /// Handle effects from Core Logic
    async fn handle_effect(&self, effect: Effect) -> BitchatResult<()> {
        match effect {
            Effect::SendPacket {
                peer_id,
                data,
                transport,
            } => {
                if transport == self.transport_type {
                    self.send_data_to_peer(peer_id, data.to_vec()).await?;
                }
            }
            Effect::SendBitchatPacket {
                peer_id,
                packet,
                transport,
            } => {
                if transport == self.transport_type {
                    self.send_bitchat_packet_to_peer(peer_id, packet).await?;
                }
            }
            Effect::BroadcastBitchatPacket { packet, transport } => {
                if transport == self.transport_type {
                    self.broadcast_bitchat_packet(packet).await?;
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
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        let local_peer_id = self
            .local_peer_id
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        // Check data size
        if data.len() > self.config.max_data_size {
            return Err(NostrTransportError::MessageTooLarge {
                size: data.len(),
                max_size: self.config.max_data_size,
            }
            .into());
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

        client
            .send_event(event)
            .await
            .map_err(NostrTransportError::EventSendFailed)?;

        debug!("Sent data to peer {} via Nostr", peer_id);
        Ok(())
    }

    /// Send BitChat packet to specific peer via Nostr using canonical embedding
    async fn send_bitchat_packet_to_peer(
        &self,
        peer_id: PeerId,
        packet: BitchatPacket,
    ) -> BitchatResult<()> {
        // Apply timing jitter if configured
        let jitter_delay = self.embedding_config.get_timing_jitter();
        if jitter_delay.as_millis() > 0 {
            #[cfg(feature = "std")]
            {
                use tokio::time::sleep;
                sleep(jitter_delay).await;
            }
            #[cfg(feature = "wasm")]
            {
                // WASM timing jitter - use web APIs if available
                // For now, we'll skip jitter in WASM environments
            }
        }
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        let local_peer_id = self
            .local_peer_id
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        // Use canonical embedding to create Nostr content
        let embedded_content = {
            // For now, encode the packet directly as binary data
            // In a full implementation, we'd determine the payload type and use appropriate encoding
            let data = WireFormat::encode(&packet).map_err(|e| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: format!("Failed to encode BitChat packet: {}", e),
                })
            })?;
            
            // Use canonical base64url encoding with bitchat1: prefix
            cfg_if::cfg_if! {
                if #[cfg(not(target_arch = "wasm32"))] {
                    use ::base64::{engine::general_purpose, Engine as _};
                    format!("bitchat1:{}", general_purpose::URL_SAFE_NO_PAD.encode(&data))
                } else {
                    // WASM stub
                    format!("bitchat1:base64url_stub")
                }
            }
        };

        // Determine embedding strategy and send accordingly
        match self.embedding_strategy {
            EmbeddingStrategy::PrivateMessage => {
                // Use NIP-17 gift wrapping for private messages
                if let Some(gift_wrapper) = &self.gift_wrapper {
                    let nip17_content = super::nip17::Nip17Content {
                        content: embedded_content,
                        expiration: None,
                    };

                    // Convert peer_id to Nostr public key
                    let recipient_pubkey = super::nip17::peer_id_to_pubkey(&peer_id)
                        .map_err(|e| NostrTransportError::KeyOperationFailed(e.to_string()))?;

                    let mut wrapper = gift_wrapper.clone();
                    let gift_wrapped_event = wrapper.create_gift_wrapped_message(&nip17_content, &recipient_pubkey)
                        .map_err(|e| NostrTransportError::EncryptionFailed(e.to_string()))?;

                    client
                        .send_event(gift_wrapped_event)
                        .await
                        .map_err(NostrTransportError::EventSendFailed)?;
                } else {
                    return Err(NostrTransportError::EncryptionFailed(
                        "NIP-17 gift wrapper not available".to_string(),
                    ).into());
                }
            }
            EmbeddingStrategy::PublicGeohash => {
                // Send as public event for geohash/location-based messaging
                let event = EventBuilder::new(BITCHAT_KIND, embedded_content, vec![Tag::hashtag("bitchat")])
                    .to_event(&self.keys)
                    .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;

                client
                    .send_event(event)
                    .await
                    .map_err(NostrTransportError::EventSendFailed)?;
            }
            EmbeddingStrategy::CustomKind(kind) => {
                // Use custom event kind
                let custom_kind = Kind::Custom(kind);
                let event = EventBuilder::new(custom_kind, embedded_content, vec![])
                    .to_event(&self.keys)
                    .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;

                client
                    .send_event(event)
                    .await
                    .map_err(NostrTransportError::EventSendFailed)?;
            }
        }

        debug!("Sent BitChat packet to peer {} via Nostr", peer_id);
        Ok(())
    }

    /// Broadcast BitChat packet via Nostr using canonical embedding
    async fn broadcast_bitchat_packet(&self, packet: BitchatPacket) -> BitchatResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| NostrTransportError::ClientNotInitialized)?;

        // Serialize the packet to binary wire format
        let data = WireFormat::encode(&packet).map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to encode BitChat packet: {}", e),
            })
        })?;

        // Check data size
        if data.len() > self.config.max_data_size {
            return Err(NostrTransportError::MessageTooLarge {
                size: data.len(),
                max_size: self.config.max_data_size,
            }
            .into());
        }

        // Use canonical embedding for broadcast
        let embedded_content = {
            cfg_if::cfg_if! {
                if #[cfg(not(target_arch = "wasm32"))] {
                    use ::base64::{engine::general_purpose, Engine as _};
                    format!("bitchat1:{}", general_purpose::URL_SAFE_NO_PAD.encode(&data))
                } else {
                    // WASM stub
                    format!("bitchat1:base64url_stub")
                }
            }
        };

        // Send as public BitChat event (broadcasts are always public)
        let event = EventBuilder::new(BITCHAT_KIND, embedded_content, vec![Tag::hashtag("bitchat")])
            .to_event(&self.keys)
            .map_err(|e| NostrTransportError::DeserializationFailed(e.to_string()))?;

        // Send to all connected relays 
        client
            .send_event(event)
            .await
            .map_err(NostrTransportError::EventSendFailed)?;

        debug!("Broadcast BitChat packet via Nostr");
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
        debug!(
            "Connection initiation requested for peer {} (Nostr is connectionless)",
            peer_id
        );
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

    /// Check connection health for WASM environments
    #[cfg(target_arch = "wasm32")]
    #[allow(dead_code)]
    async fn check_connection_health(&self) {
        if let Some(client) = &self.client {
            // In WASM, network connectivity can be intermittent
            // Check if we can reach any relays
            let relay_urls = client.relays().await;
            if relay_urls.is_empty() {
                warn!("No Nostr relays connected in WASM environment");

                // Attempt to reconnect to configured relays
                for relay_url in &self.config.relay_urls {
                    if let Err(e) = client.add_relay(relay_url).await {
                        debug!("Failed to reconnect to relay {}: {}", relay_url, e);
                    } else {
                        info!("Reconnected to relay: {}", relay_url);
                    }
                }
            } else {
                debug!(
                    "WASM connection health check: {} relays connected",
                    relay_urls.len()
                );
            }
        }
    }

    /// No-op for non-WASM environments  
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    async fn check_connection_health(&self) {
        // Connection monitoring not needed for native environments
    }
}

#[async_trait]
impl TransportTask for NostrTransportTask {
    fn attach_channels(
        &mut self,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<()> {
        if self.channels.is_some() {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Nostr transport channels already attached".to_string(),
                },
            ));
        }
        self.channels = Some(TransportHandle::new(event_sender, effect_receiver));
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
