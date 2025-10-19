//! Core Logic Task Implementation
//!
//! Contains the main CoreLogicTask struct and its coordination logic.

use bitchat_core::{
    PeerId, BitchatError, BitchatResult,
    Command, Event, Effect, AppEvent,
    internal::{
        CommandReceiver, EventReceiver, EffectSender, AppEventSender,
        TaskId, LogLevel, TransportError, TimeSource
    }
};
use crate::rate_limiter::RateLimiter;
use super::state::{LoggerWrapper, SystemTimeSource, CoreStats, CoreState};
use super::handlers::CommandHandlers;

#[cfg(feature = "std")]
use tracing::{info, error};
#[cfg(not(feature = "std"))]
use log::{info, warn, error, debug};

// ----------------------------------------------------------------------------
// Core Logic Task
// ----------------------------------------------------------------------------

/// The Core Logic task that processes all commands and events
pub struct CoreLogicTask {
    /// Core application state (consolidated state management)
    state: CoreState,
    /// Rate limiter for DoS protection
    rate_limiter: RateLimiter,
    /// Channel for receiving commands from UI and external systems
    command_receiver: CommandReceiver,
    /// Channel for receiving events from transport tasks
    event_receiver: EventReceiver,
    /// Channel for sending effects to transport tasks
    effect_sender: EffectSender,
    /// Channel for sending app events to UI task
    app_event_sender: AppEventSender,
    /// Logger for task communication (using enum for object safety)
    logger: LoggerWrapper,
    /// Task start time for uptime calculation
    start_time: bitchat_core::internal::Timestamp,
    /// Whether the task should continue running
    running: bool,
}

impl CoreLogicTask {
    /// Create a new Core Logic task
    pub fn new(
        peer_id: PeerId,
        command_receiver: CommandReceiver,
        event_receiver: EventReceiver,
        effect_sender: EffectSender,
        app_event_sender: AppEventSender,
        logger: LoggerWrapper,
        session_config: bitchat_core::internal::SessionConfig,
        delivery_config: bitchat_core::internal::DeliveryConfig,
        rate_limit_config: bitchat_core::internal::RateLimitConfig,
    ) -> BitchatResult<Self> {
        let time_source = SystemTimeSource;
        
        // Use consolidated CoreState instead of individual managers
        let state = CoreState::new(peer_id, session_config, delivery_config)?;
        let rate_limiter = RateLimiter::new(rate_limit_config);

        Ok(Self {
            state,
            rate_limiter,
            command_receiver,
            event_receiver,
            effect_sender,
            app_event_sender,
            logger,
            start_time: time_source.now(),
            running: true,
        })
    }

    /// Run the main Core Logic task loop
    #[cfg(feature = "std")]
    pub async fn run(&mut self) -> BitchatResult<()> {
        self.logger.log_task_event(
            TaskId::CoreLogic,
            LogLevel::Info,
            "Core Logic task starting"
        );

        while self.running {
            tokio::select! {
                // Process command from UI or external systems
                command = self.command_receiver.recv() => {
                    match command {
                        Some(cmd) => {
                            self.logger.log_receive_command(
                                TaskId::UI,
                                TaskId::CoreLogic,
                                &cmd,
                                None
                            );
                            
                            if let Err(e) = self.process_command(cmd).await {
                                match e {
                                    // Unrecoverable errors: shut down the task
                                    BitchatError::Channel { .. } | 
                                    BitchatError::Configuration { .. } => {
                                        error!("Unrecoverable error processing command, shutting down CoreLogicTask: {}", e);
                                        self.running = false;
                                        break;
                                    },
                                    // Peer-specific errors: log and continue
                                    BitchatError::Session(bitchat_core::internal::SessionError::SessionNotFound { peer_id }) => {
                                        #[cfg(not(feature = "std"))]
                                        log::warn!("Session not found for peer {}. Dropping command.", peer_id);
                                        #[cfg(feature = "std")]
                                        tracing::warn!("Session not found for peer {}. Dropping command.", peer_id);
                                    },
                                    BitchatError::Transport(bitchat_core::internal::TransportError::PeerNotFound { peer_id }) => {
                                        #[cfg(not(feature = "std"))]
                                        log::warn!("Peer not found: {}. Dropping command.", peer_id);
                                        #[cfg(feature = "std")]
                                        tracing::warn!("Peer not found: {}. Dropping command.", peer_id);
                                    },
                                    // Log other errors and continue
                                    _ => {
                                        error!("Error processing command: {}", e);
                                    }
                                }
                            }
                        }
                        None => {
                            info!("Command channel closed, shutting down");
                            break;
                        }
                    }
                }

                // Process event from transport tasks
                event = self.event_receiver.recv() => {
                    match event {
                        Some(evt) => {
                            let transport = match &evt {
                                Event::PeerDiscovered { transport, .. } => *transport,
                                Event::MessageReceived { transport, .. } => *transport,
                                Event::ConnectionEstablished { transport, .. } => *transport,
                                Event::ConnectionLost { transport, .. } => *transport,
                                Event::TransportError { transport, .. } => *transport,
                            };

                            self.logger.log_receive_event(
                                TaskId::Transport(transport),
                                TaskId::CoreLogic,
                                &evt,
                                None
                            );
                            
                            if let Err(e) = self.process_event(evt).await {
                                match e {
                                    // Unrecoverable errors: shut down the task
                                    BitchatError::Channel { .. } | 
                                    BitchatError::Configuration { .. } => {
                                        error!("Unrecoverable error processing event, shutting down CoreLogicTask: {}", e);
                                        self.running = false;
                                        break;
                                    },
                                    // Peer-specific errors: log and continue
                                    BitchatError::Session(bitchat_core::internal::SessionError::SessionNotFound { peer_id }) => {
                                        #[cfg(not(feature = "std"))]
                                        log::warn!("Session not found for peer {}. Dropping event.", peer_id);
                                        #[cfg(feature = "std")]
                                        tracing::warn!("Session not found for peer {}. Dropping event.", peer_id);
                                    },
                                    BitchatError::Transport(bitchat_core::internal::TransportError::PeerNotFound { peer_id }) => {
                                        #[cfg(not(feature = "std"))]
                                        log::warn!("Peer not found: {}. Dropping event.", peer_id);
                                        #[cfg(feature = "std")]
                                        tracing::warn!("Peer not found: {}. Dropping event.", peer_id);
                                    },
                                    // Cryptographic errors on events are usually peer-specific
                                    BitchatError::Crypto(_) | BitchatError::Noise(_) => {
                                        #[cfg(not(feature = "std"))]
                                        log::warn!("Cryptographic error processing event (possibly malicious peer): {}", e);
                                        #[cfg(feature = "std")]
                                        tracing::warn!("Cryptographic error processing event (possibly malicious peer): {}", e);
                                    },
                                    // Log other errors and continue
                                    _ => {
                                        error!("Error processing event: {}", e);
                                    }
                                }
                            }
                        }
                        None => {
                            info!("Event channel closed");
                            // Continue running even if event channel closes
                        }
                    }
                }
            }
        }

        self.logger.log_task_event(
            TaskId::CoreLogic,
            LogLevel::Info,
            "Core Logic task stopped"
        );

        Ok(())
    }

    /// Stop the Core Logic task
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Process a command and return effects and app events
    async fn process_command(&mut self, command: Command) -> BitchatResult<()> {
        self.state.stats.commands_processed += 1;
        
        let (effects, app_events) = match command {
            Command::SendMessage { recipient, content } => {
                CommandHandlers::handle_send_message(&mut self.state, recipient, content).await?
            }
            Command::ConnectToPeer { peer_id } => {
                CommandHandlers::handle_connect_to_peer(&mut self.state, peer_id).await?
            }
            Command::StartDiscovery => {
                CommandHandlers::handle_start_discovery().await?
            }
            Command::StopDiscovery => {
                CommandHandlers::handle_stop_discovery().await?
            }
            Command::DisconnectFromPeer { peer_id } => {
                CommandHandlers::handle_disconnect_from_peer(&mut self.state, peer_id).await?
            }
            Command::PauseTransport { transport } => {
                // These commands don't have state and can be handled directly
                (vec![bitchat_core::Effect::PauseTransport { transport }], Vec::new())
            }
            Command::ResumeTransport { transport } => {
                (vec![bitchat_core::Effect::ResumeTransport { transport }], Vec::new())
            }
            Command::GetSystemStatus => {
                self.handle_get_system_status().await?
            }
            Command::Shutdown => {
                self.running = false;
                CommandHandlers::handle_shutdown().await?
            }
        };

        // Send effects to transport tasks
        for effect in effects {
            self.send_effect(effect).await?;
        }

        // Send app events to UI task
        for app_event in app_events {
            self.send_app_event(app_event).await?;
        }

        Ok(())
    }

    /// Process an event from transport tasks
    async fn process_event(&mut self, event: Event) -> BitchatResult<()> {
        self.state.stats.events_processed += 1;
        
        let (effects, app_events) = match event {
            Event::PeerDiscovered { peer_id, transport, signal_strength } => {
                CommandHandlers::handle_peer_discovered(&mut self.state, peer_id, transport, signal_strength).await?
            }
            Event::MessageReceived { from, content, transport, message_id, recipient, timestamp, sequence } => {
                // Rate limit incoming messages to prevent DoS attacks
                self.rate_limiter.check_message_allowed(from)?;
                CommandHandlers::handle_message_received(
                    &mut self.state,
                    from,
                    content,
                    transport,
                    message_id,
                    recipient,
                    timestamp,
                    sequence,
                ).await?
            }
            Event::ConnectionEstablished { peer_id, transport } => {
                // Rate limit new connections to prevent DoS attacks
                self.rate_limiter.check_connection_allowed(peer_id)?;
                CommandHandlers::handle_connection_established(&mut self.state, peer_id, transport).await?
            }
            Event::ConnectionLost { peer_id, transport, reason } => {
                CommandHandlers::handle_connection_lost(&mut self.state, peer_id, transport, reason).await?
            }
            Event::TransportError { transport, error } => {
                CommandHandlers::handle_transport_error(transport, error).await?
            }
        };

        // Send effects to transport tasks
        for effect in effects {
            self.send_effect(effect).await?;
        }

        // Send app events to UI task
        for app_event in app_events {
            self.send_app_event(app_event).await?;
        }

        Ok(())
    }

    /// Send effect to transport tasks
    async fn send_effect(&mut self, effect: Effect) -> BitchatResult<()> {
        let transport = match &effect {
            Effect::SendPacket { transport, .. } => *transport,
            Effect::InitiateConnection { transport, .. } => *transport,
            Effect::StartListening { transport } => *transport,
            Effect::StopListening { transport } => *transport,
            Effect::StartTransportDiscovery { transport } => *transport,
            Effect::StopTransportDiscovery { transport } => *transport,
            Effect::PauseTransport { transport } => *transport,
            Effect::ResumeTransport { transport } => *transport,
            Effect::WriteToStorage { .. } => return Ok(()), // Handled locally for now
            Effect::ScheduleRetry { .. } => return Ok(()), // Handled locally for now
        };

        self.logger.log_send_effect(
            TaskId::CoreLogic,
            TaskId::Transport(transport),
            &effect,
            None
        );

        #[cfg(feature = "std")]
        {
            self.effect_sender.send(effect).map_err(|_| {
                BitchatError::Transport(bitchat_core::internal::TransportError::Shutdown {
                    reason: "Effect channel closed".to_string(),
                })
            })?;
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = effect;
            return Err(BitchatError::Transport(bitchat_core::internal::TransportError::InvalidConfiguration {
                reason: "No effect channel implementation configured".to_string(),
            }));
        }

        self.state.stats.effects_generated += 1;
        Ok(())
    }

    /// Send app event to UI task
    async fn send_app_event(&mut self, app_event: AppEvent) -> BitchatResult<()> {
        self.logger.log_send_app_event(
            TaskId::CoreLogic,
            TaskId::UI,
            &app_event,
            None
        );

        self.app_event_sender.send(app_event).await
            .map_err(|_| TransportError::SendBufferFull { 
                capacity: 0 // Channel closed
            })?;

        self.state.stats.app_events_generated += 1;
        Ok(())
    }

    /// Get current statistics
    pub fn stats(&self) -> &CoreStats {
        &self.state.stats
    }

    /// Get message store reference
    pub fn message_store(&self) -> &bitchat_core::internal::MessageStore {
        &self.state.message_store
    }

    /// Get connection states
    pub fn connections(&self) -> &std::collections::HashMap<PeerId, bitchat_core::internal::ConnectionState> {
        &self.state.connections
    }

    // ----------------------------------------------------------------------------
    // Command Handlers (System Status only - others use CommandHandlers)
    // ----------------------------------------------------------------------------

    /// Handle system status request
    async fn handle_get_system_status(&mut self) -> BitchatResult<(Vec<Effect>, Vec<AppEvent>)> {
        let current_time = SystemTimeSource.now();
        let uptime_seconds = (current_time.as_millis().saturating_sub(self.start_time.as_millis())) / 1000;
        
        // Count connections by status
        let mut peer_count = 0;
        let mut active_connections = 0;
        for connection in self.state.connections.values() {
            peer_count += 1;
            if connection.can_send_messages() {
                active_connections += 1;
            }
        }
        
        // For now, report both transports as active (real implementation would track actual status)
        let transport_status = vec![
            (bitchat_core::ChannelTransportType::Ble, bitchat_core::TransportStatus::Active),
            (bitchat_core::ChannelTransportType::Nostr, bitchat_core::TransportStatus::Active),
        ];
        
        let app_events = vec![AppEvent::SystemStatusReport {
            peer_count,
            active_connections,
            message_count: self.state.stats.messages_sent + self.state.stats.messages_received,
            uptime_seconds,
            transport_status,
            memory_usage_bytes: None, // Could implement memory tracking later
        }];
        
        Ok((Vec::new(), app_events))
    }

    // Event handlers now use CommandHandlers from bitchat-core for proper layering
}

// All individual command and event handlers have been removed.
// They now properly use the CommandHandlers abstraction from bitchat-core,
// which maintains proper protocol layering and avoids state fragmentation.
