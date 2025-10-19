//! Decomposed async tasks replacing the monolithic CoreLogicTask
//!
//! This module implements the runtime decomposition from the refactor plan,
//! breaking the single CoreLogicTask into specialized async tasks:
//!
//! - `MessageIngressTask`: Handles commands/events with schema validation
//! - `SessionManagerTask`: Noise handshakes and key rotation  
//! - `StorageDeliveryTask`: Message store, deduplication, and retry state
//! - `SupervisorTask`: Channel monitoring and diagnostics

use crate::managers::{DeliveryTracker, NoiseSessionManager, SessionTimeouts};
use crate::rate_limiter::RateLimiter;
use alloc::{string::String, vec::Vec};
use bitchat_core::{
    channel::communication::ConnectionStatus,
    internal::{
        AppEventSender, CommandReceiver, ConnectionState, DeliveryConfig, DeliveryStatus,
        EffectSender, EventReceiver, MessageId, MessageStore, PacketError, SessionConfig, TaskId,
        TimeSource, Timestamp,
    },
    protocol::crypto::NoiseKeyPair,
    types::SystemTimeSource,
    AppEvent, BitchatError, BitchatResult, ChannelTransportType, Command, Effect, Event, PeerId,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[cfg(not(feature = "std"))]
use log::{debug, error, info, warn};
#[cfg(feature = "std")]
use tracing::{debug, error, info, warn};

// ----------------------------------------------------------------------------
// Inter-Task Communication
// ----------------------------------------------------------------------------

/// Commands sent between internal tasks
#[derive(Debug, Clone)]
pub enum InternalCommand {
    /// Process an encrypted message from a peer
    ProcessEncryptedMessage {
        peer_id: PeerId,
        ciphertext: Vec<u8>,
        transport: ChannelTransportType,
    },
    /// Store a message and track delivery
    StoreMessage {
        message_id: MessageId,
        content: String,
        sender: PeerId,
        recipient: Option<PeerId>,
    },
    /// Request session establishment with peer
    EstablishSession {
        peer_id: PeerId,
        transport: ChannelTransportType,
    },
    /// Rotate session keys for peer
    RotateSessionKeys { peer_id: PeerId },
    /// Update delivery status for a message
    UpdateDeliveryStatus {
        message_id: MessageId,
        status: DeliveryStatus,
    },
    /// Request system shutdown
    Shutdown,
}

/// Events sent between internal tasks
#[derive(Debug, Clone)]
pub enum InternalEvent {
    /// Session established with peer
    SessionEstablished {
        peer_id: PeerId,
        transport: ChannelTransportType,
    },
    /// Session failed for peer
    SessionFailed { peer_id: PeerId, reason: String },
    /// Message decrypted successfully
    MessageDecrypted {
        message_id: MessageId,
        content: String,
        sender: PeerId,
        recipient: Option<PeerId>,
    },
    /// Message stored successfully
    MessageStored {
        message_id: MessageId,
        timestamp: Timestamp,
    },
    /// Delivery confirmation received
    DeliveryConfirmed {
        message_id: MessageId,
        peer_id: PeerId,
    },
    /// Task health update
    TaskHealth {
        task_id: TaskId,
        status: TaskHealthStatus,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum TaskHealthStatus {
    Healthy,
    Degraded,
    Failed,
}

// ----------------------------------------------------------------------------
// Message Ingress Task
// ----------------------------------------------------------------------------

/// Handles commands/events with schema validation
pub struct MessageIngressTask {
    // Input channels
    command_receiver: CommandReceiver,
    event_receiver: EventReceiver,

    // Output channels to other tasks
    session_command_sender: mpsc::UnboundedSender<InternalCommand>,
    storage_command_sender: mpsc::UnboundedSender<InternalCommand>,

    // Output channels to transports/UI
    #[allow(dead_code)]
    effect_sender: EffectSender,
    app_event_sender: AppEventSender,

    // State
    peer_id: PeerId,
    rate_limiter: RateLimiter,
    message_sequence: u64,
    running: bool,
}

impl MessageIngressTask {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        peer_id: PeerId,
        command_receiver: CommandReceiver,
        event_receiver: EventReceiver,
        effect_sender: EffectSender,
        app_event_sender: AppEventSender,
        session_command_sender: mpsc::UnboundedSender<InternalCommand>,
        storage_command_sender: mpsc::UnboundedSender<InternalCommand>,
        rate_limit_config: bitchat_core::internal::RateLimitConfig,
    ) -> Self {
        Self {
            command_receiver,
            event_receiver,
            session_command_sender,
            storage_command_sender,
            effect_sender,
            app_event_sender,
            peer_id,
            rate_limiter: RateLimiter::new(rate_limit_config),
            message_sequence: 0,
            running: true,
        }
    }

    pub async fn run(&mut self) -> BitchatResult<()> {
        info!("Message ingress task starting");

        while self.running {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    if let Err(e) = self.handle_command(command).await {
                        error!("Error handling command: {:?}", e);
                    }
                }
                Some(event) = self.event_receiver.recv() => {
                    if let Err(e) = self.handle_event(event).await {
                        error!("Error handling event: {:?}", e);
                    }
                }
                else => {
                    debug!("All channels closed, stopping ingress task");
                    break;
                }
            }
        }

        info!("Message ingress task stopped");
        Ok(())
    }

    async fn handle_command(&mut self, command: Command) -> BitchatResult<()> {
        // Apply rate limiting - for now just check against a dummy peer
        let dummy_peer = PeerId::new([0; 8]);
        if self.rate_limiter.check_message_allowed(dummy_peer).is_err() {
            warn!("Rate limit exceeded for command: {:?}", command);
            return Err(BitchatError::RateLimited {
                reason: "Command rate limit exceeded".to_string(),
            });
        }

        match command {
            Command::SendMessage { recipient, content } => {
                self.message_sequence += 1;

                // Request session establishment if needed
                self.session_command_sender
                    .send(InternalCommand::EstablishSession {
                        peer_id: recipient,
                        transport: ChannelTransportType::Ble, // Default, will be determined by session manager
                    })
                    .map_err(|_| BitchatError::Channel {
                        message: "Failed to send session command".to_string(),
                    })?;

                // Generate message ID and store
                let message_id = MessageId::from_bytes(
                    Sha256::digest(
                        format!("{:?}:{}", self.peer_id, self.message_sequence).as_bytes(),
                    )
                    .into(),
                );

                self.storage_command_sender
                    .send(InternalCommand::StoreMessage {
                        message_id,
                        content: content.clone(),
                        sender: self.peer_id,
                        recipient: Some(recipient),
                    })
                    .map_err(|_| BitchatError::Channel {
                        message: "Failed to send storage command".to_string(),
                    })?;

                // Send app event
                let app_event = AppEvent::MessageSent {
                    to: recipient,
                    content,
                    timestamp: SystemTimeSource.now().as_millis(),
                };

                let _ = self.app_event_sender.try_send(app_event);
            }

            Command::ConnectToPeer { peer_id } => {
                self.session_command_sender
                    .send(InternalCommand::EstablishSession {
                        peer_id,
                        transport: ChannelTransportType::Ble,
                    })
                    .map_err(|_| BitchatError::Channel {
                        message: "Failed to send session command".to_string(),
                    })?;
            }

            Command::Shutdown => {
                info!("Shutdown command received");
                self.running = false;

                // Notify other tasks
                let _ = self.session_command_sender.send(InternalCommand::Shutdown);
                let _ = self.storage_command_sender.send(InternalCommand::Shutdown);
            }

            _ => {
                debug!("Unhandled command: {:?}", command);
            }
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: Event) -> BitchatResult<()> {
        match event {
            Event::MessageReceived {
                from,
                content,
                message_id,
                ..
            } => {
                // Validate and process received message
                if content.is_empty() {
                    return Err(BitchatError::InvalidPacket(PacketError::Generic {
                        message: "Empty message content".to_string(),
                    }));
                }

                // For now, treat as plaintext (session manager will handle encryption)
                let generated_id = message_id.unwrap_or_else(|| {
                    MessageId::from_bytes(
                        Sha256::digest(format!("{}:{}", from, content).as_bytes()).into(),
                    )
                });

                // Store the message
                self.storage_command_sender
                    .send(InternalCommand::StoreMessage {
                        message_id: generated_id,
                        content: content.clone(),
                        sender: from,
                        recipient: Some(self.peer_id),
                    })
                    .map_err(|_| BitchatError::Channel {
                        message: "Failed to send storage command".to_string(),
                    })?;

                // Forward to app
                let app_event = AppEvent::MessageReceived {
                    from,
                    content,
                    timestamp: SystemTimeSource.now().as_millis(),
                };

                let _ = self.app_event_sender.try_send(app_event);
            }

            Event::PeerDiscovered {
                peer_id, transport, ..
            } => {
                let app_event = AppEvent::PeerStatusChanged {
                    peer_id,
                    status: ConnectionStatus::Discovering,
                    transport: Some(transport),
                };
                let _ = self.app_event_sender.try_send(app_event);
            }

            Event::ConnectionEstablished { peer_id, transport } => {
                let app_event = AppEvent::PeerStatusChanged {
                    peer_id,
                    status: ConnectionStatus::Connected,
                    transport: Some(transport),
                };
                let _ = self.app_event_sender.try_send(app_event);
            }

            Event::ConnectionLost {
                peer_id,
                transport,
                reason: _,
            } => {
                let app_event = AppEvent::PeerStatusChanged {
                    peer_id,
                    status: ConnectionStatus::Disconnected,
                    transport: Some(transport),
                };
                let _ = self.app_event_sender.try_send(app_event);
            }

            _ => {
                debug!("Unhandled event: {:?}", event);
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Session Manager Task
// ----------------------------------------------------------------------------

/// Handles Noise handshakes and key rotation
pub struct SessionManagerTask {
    // Input/Output channels
    command_receiver: mpsc::UnboundedReceiver<InternalCommand>,
    event_sender: mpsc::UnboundedSender<InternalEvent>,
    effect_sender: EffectSender,

    // State
    peer_id: PeerId,
    #[allow(dead_code)]
    session_manager: NoiseSessionManager<SystemTimeSource>,
    connections: HashMap<PeerId, ConnectionState>,
    running: bool,
}

impl SessionManagerTask {
    pub fn new(
        peer_id: PeerId,
        command_receiver: mpsc::UnboundedReceiver<InternalCommand>,
        event_sender: mpsc::UnboundedSender<InternalEvent>,
        effect_sender: EffectSender,
        _session_config: SessionConfig,
    ) -> BitchatResult<Self> {
        Ok(Self {
            command_receiver,
            event_sender,
            effect_sender,
            peer_id,
            session_manager: {
                // For now, use dummy values for the session manager
                let local_key = NoiseKeyPair::generate();
                let timeouts = SessionTimeouts::default();
                NoiseSessionManager::new(local_key, SystemTimeSource::new(), timeouts)
            },
            connections: HashMap::new(),
            running: true,
        })
    }

    pub async fn run(&mut self) -> BitchatResult<()> {
        info!("Session manager task starting");

        while self.running {
            if let Some(command) = self.command_receiver.recv().await {
                if let Err(e) = self.handle_command(command).await {
                    error!("Error in session manager: {:?}", e);
                }
            } else {
                debug!("Session manager command channel closed");
                break;
            }
        }

        info!("Session manager task stopped");
        Ok(())
    }

    async fn handle_command(&mut self, command: InternalCommand) -> BitchatResult<()> {
        match command {
            InternalCommand::EstablishSession { peer_id, transport } => {
                debug!("Establishing session with peer: {:?}", peer_id);

                // Create connection state if not exists
                self.connections
                    .entry(peer_id)
                    .or_insert_with(|| ConnectionState::new_disconnected(peer_id));

                // For now, just mark as connected (real Noise handshake would go here)
                let _ = self
                    .event_sender
                    .send(InternalEvent::SessionEstablished { peer_id, transport });

                // Send connection effect to transport
                let effect = Effect::InitiateConnection { peer_id, transport };
                let _ = self.effect_sender.send(effect);
            }

            InternalCommand::RotateSessionKeys { peer_id } => {
                debug!("Rotating session keys for peer: {:?}", peer_id);
                // Key rotation logic would go here
            }

            InternalCommand::ProcessEncryptedMessage {
                peer_id,
                ciphertext,
                transport: _,
            } => {
                debug!("Processing encrypted message from peer: {:?}", peer_id);

                // Decrypt message (simplified for now)
                let decrypted =
                    String::from_utf8(ciphertext).unwrap_or_else(|_| "Invalid UTF-8".to_string());

                let message_id = MessageId::from_bytes(
                    Sha256::digest(format!("{}:{}", peer_id, decrypted).as_bytes()).into(),
                );

                let _ = self.event_sender.send(InternalEvent::MessageDecrypted {
                    message_id,
                    content: decrypted,
                    sender: peer_id,
                    recipient: Some(self.peer_id),
                });
            }

            InternalCommand::Shutdown => {
                info!("Session manager shutdown requested");
                self.running = false;
            }

            _ => {
                debug!("Unhandled session command: {:?}", command);
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Storage and Delivery Task
// ----------------------------------------------------------------------------

/// Handles message store, deduplication, and retry state
pub struct StorageDeliveryTask {
    // Input/Output channels
    command_receiver: mpsc::UnboundedReceiver<InternalCommand>,
    event_sender: mpsc::UnboundedSender<InternalEvent>,

    // State
    #[allow(dead_code)]
    message_store: MessageStore,
    delivery_tracker: DeliveryTracker<SystemTimeSource>,
    running: bool,
}

impl StorageDeliveryTask {
    pub fn new(
        command_receiver: mpsc::UnboundedReceiver<InternalCommand>,
        event_sender: mpsc::UnboundedSender<InternalEvent>,
        _delivery_config: DeliveryConfig,
    ) -> BitchatResult<Self> {
        Ok(Self {
            command_receiver,
            event_sender,
            message_store: MessageStore::new(),
            delivery_tracker: DeliveryTracker::new(SystemTimeSource::new()),
            running: true,
        })
    }

    pub async fn run(&mut self) -> BitchatResult<()> {
        info!("Storage/delivery task starting");

        while self.running {
            if let Some(command) = self.command_receiver.recv().await {
                if let Err(e) = self.handle_command(command).await {
                    error!("Error in storage/delivery task: {:?}", e);
                }
            } else {
                debug!("Storage/delivery command channel closed");
                break;
            }
        }

        info!("Storage/delivery task stopped");
        Ok(())
    }

    async fn handle_command(&mut self, command: InternalCommand) -> BitchatResult<()> {
        match command {
            InternalCommand::StoreMessage {
                message_id,
                content,
                sender,
                recipient,
            } => {
                debug!("Storing message: {:?}", message_id);

                // Store in message store
                // (Simplified - real implementation would use MessageStore methods)

                // Track delivery if outbound message
                if sender == bitchat_core::PeerId::new([0; 8]) {
                    // Use proper peer comparison
                    if let Some(recipient_id) = recipient {
                        use uuid::Uuid;
                        let uuid = Uuid::new_v4();
                        self.delivery_tracker.track_message(
                            uuid,
                            recipient_id,
                            content.as_bytes().to_vec(),
                        );
                    }
                }

                let _ = self.event_sender.send(InternalEvent::MessageStored {
                    message_id,
                    timestamp: SystemTimeSource.now(),
                });
            }

            InternalCommand::UpdateDeliveryStatus { message_id, status } => {
                debug!("Updating delivery status for message: {:?}", message_id);

                match status {
                    DeliveryStatus::Confirmed => {
                        // Mark as delivered in tracker
                        // self.delivery_tracker.mark_delivered(message_id)?;
                    }
                    DeliveryStatus::Failed => {
                        // Handle failed delivery
                    }
                    _ => {}
                }
            }

            InternalCommand::Shutdown => {
                info!("Storage/delivery task shutdown requested");
                self.running = false;
            }

            _ => {
                debug!("Unhandled storage command: {:?}", command);
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Helper Structs
// ----------------------------------------------------------------------------
