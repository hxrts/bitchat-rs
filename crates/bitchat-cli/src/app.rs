//! Core BitChat application with improved architecture

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use bitchat_ble::BleTransport;
use bitchat_core::{
    BitchatMessage, MessageBuilder, MessageFragmenter, MessageReassembler,
    PeerId, StdDeliveryTracker, StdNoiseSessionManager, StdTimeSource,
    transport::{TransportManager, TransportSelectionPolicy, TransportType},
    handlers::{BitchatEvent, EventEmittingHandler, EventHandler},
    delivery::DeliveryStats,
};
use bitchat_nostr::NostrTransport;

use crate::config::AppConfig;
use crate::state::{StateManager, AppState};
use crate::error::{CliError, Result};

/// Message event for the UI
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// New message received
    MessageReceived {
        from: PeerId,
        message: BitchatMessage,
    },
    /// Message delivery confirmed
    DeliveryConfirmed {
        message_id: Uuid,
        confirmed_by: PeerId,
    },
    /// New peer discovered
    PeerDiscovered {
        peer_id: PeerId,
        transport_type: TransportType,
    },
    /// Peer status changed
    PeerStatusChanged {
        peer_id: PeerId,
        status: PeerStatus,
    },
    /// Transport status changed
    TransportStatusChanged {
        transport_type: TransportType,
        status: TransportStatus,
    },
    /// Error occurred
    Error {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum PeerStatus {
    Online,
    Offline,
    Connecting,
}

#[derive(Debug, Clone)]
pub enum TransportStatus {
    Starting,
    Active,
    Failed(String),
    Stopped,
}

/// Internal event handler that forwards events to the app
struct AppEventHandler {
    event_sender: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler for AppEventHandler {
    fn handle_event(&mut self, event: BitchatEvent) {
        let app_event = match event {
            BitchatEvent::MessageReceived { from, message } => {
                AppEvent::MessageReceived { from, message }
            }
            BitchatEvent::DeliveryConfirmed { message_id, confirmed_by } => {
                AppEvent::DeliveryConfirmed { message_id, confirmed_by }
            }
            BitchatEvent::PeerAnnounced { peer_id, .. } => {
                AppEvent::PeerDiscovered {
                    peer_id,
                    transport_type: TransportType::Nostr, // Default assumption
                }
            }
            _ => return, // Ignore other events for now
        };

        if let Err(_) = self.event_sender.send(app_event) {
            error!("Failed to send app event - receiver dropped");
        }
    }
}

/// Core BitChat application with improved architecture
pub struct BitchatApp {
    /// Our peer ID
    peer_id: PeerId,
    /// Session manager for handling peer connections
    session_manager: StdNoiseSessionManager,
    /// Delivery tracker for message reliability
    delivery_tracker: StdDeliveryTracker,
    /// Transport manager with intelligent routing
    transport_manager: TransportManager,
    /// Message fragmenter for large messages
    fragmenter: MessageFragmenter,
    /// Message reassembler
    reassembler: Arc<Mutex<MessageReassembler>>,
    /// Application configuration
    config: AppConfig,
    /// State manager for persistence
    state_manager: StateManager,
    /// Event receiver for UI
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AppEvent>>>,
    /// Event sender (kept for cloning)
    event_sender: mpsc::UnboundedSender<AppEvent>,
    /// Message handler for processing incoming packets
    message_handler: Arc<Mutex<EventEmittingHandler<AppEventHandler>>>,
    /// Application start time
    start_time: Instant,
    /// Whether the application is running
    running: Arc<RwLock<bool>>,
}

impl BitchatApp {
    /// Create a new BitChat application
    pub async fn new(mut config: AppConfig) -> Result<Self> {
        // Set up state directory
        let state_dir = config.get_state_dir()?;
        let mut state_manager = StateManager::new(state_dir, config.state.auto_save_interval)
            .map_err(|e| CliError::StatePersistence(format!("Failed to initialize state manager: {}", e)))?;

        // Record startup
        state_manager.state_mut().record_startup();

        // Generate or load crypto keys
        let noise_key = bitchat_core::crypto::NoiseKeyPair::generate();
        let peer_id = PeerId::from_bytes(&noise_key.public_key_bytes());

        info!("Starting BitChat with peer ID: {}...", &peer_id.to_string()[..8]);

        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Create message handler
        let app_event_handler = AppEventHandler {
            event_sender: event_sender.clone(),
        };
        let message_handler = EventEmittingHandler::new(app_event_handler);

        // Initialize core components
        let session_manager = StdNoiseSessionManager::new(noise_key, StdTimeSource);
        let delivery_tracker = StdDeliveryTracker::new(StdTimeSource);
        let transport_manager = TransportManager::new();
        let fragmenter = MessageFragmenter;
        let reassembler = MessageReassembler::new();

        Ok(Self {
            peer_id,
            session_manager,
            delivery_tracker,
            transport_manager,
            fragmenter,
            reassembler: Arc::new(Mutex::new(reassembler)),
            config,
            state_manager,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            event_sender,
            message_handler: Arc::new(Mutex::new(message_handler)),
            start_time: Instant::now(),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Initialize and start transports
    pub async fn start_transports(
        &mut self,
        use_ble: bool,
        use_nostr: bool,
        local_relay: bool,
    ) -> Result<()> {
        // Send status update
        let _ = self.event_sender.send(AppEvent::TransportStatusChanged {
            transport_type: TransportType::Ble,
            status: TransportStatus::Starting,
        });

        // Add BLE transport if enabled
        if use_ble {
            info!("Initializing BLE transport...");
            match self.try_add_ble_transport().await {
                Ok(_) => {
                    let _ = self.event_sender.send(AppEvent::TransportStatusChanged {
                        transport_type: TransportType::Ble,
                        status: TransportStatus::Active,
                    });
                }
                Err(e) => {
                    warn!("Failed to initialize BLE transport: {}", e);
                    let _ = self.event_sender.send(AppEvent::TransportStatusChanged {
                        transport_type: TransportType::Ble,
                        status: TransportStatus::Failed(e.to_string()),
                    });
                }
            }
        }

        // Add Nostr transport if enabled
        if use_nostr {
            info!("Initializing Nostr transport...");
            match self.try_add_nostr_transport(local_relay).await {
                Ok(_) => {
                    let _ = self.event_sender.send(AppEvent::TransportStatusChanged {
                        transport_type: TransportType::Nostr,
                        status: TransportStatus::Active,
                    });
                }
                Err(e) => {
                    warn!("Failed to initialize Nostr transport: {}", e);
                    let _ = self.event_sender.send(AppEvent::TransportStatusChanged {
                        transport_type: TransportType::Nostr,
                        status: TransportStatus::Failed(e.to_string()),
                    });
                }
            }
        }

        // Configure transport selection policy
        self.configure_transport_policy();

        // Start all transports
        self.transport_manager
            .start_all()
            .await
            .map_err(|e| CliError::TransportInit(format!("Failed to start transports: {}", e)))?;

        info!("All transports started successfully");
        Ok(())
    }

    /// Try to add BLE transport with error handling
    async fn try_add_ble_transport(&mut self) -> Result<()> {
        info!("Initializing BLE transport...");
        
        let ble_config = self.config.ble.clone();
        let mut ble_transport = BleTransport::with_config(self.peer_id, ble_config);
        
        self.transport_manager.add_transport(Box::new(ble_transport));
        info!("BLE transport added successfully");
        Ok(())
    }

    /// Try to add Nostr transport with error handling
    async fn try_add_nostr_transport(&mut self, local_relay: bool) -> Result<()> {
        info!("Initializing Nostr transport...");
        
        let nostr_config = self.config.nostr.clone();
        let mut nostr_transport = NostrTransport::with_config(self.peer_id, nostr_config, local_relay);
        
        self.transport_manager.add_transport(Box::new(nostr_transport));
        info!("Nostr transport added successfully");
        Ok(())
    }

    /// Configure transport selection policy based on preferences
    fn configure_transport_policy(&mut self) {
        if self.config.transport_preferences.prefer_ble {
            self.transport_manager.set_selection_policy(
                TransportSelectionPolicy::PreferenceOrder(vec![
                    TransportType::Ble,
                    TransportType::Nostr,
                ]),
            );
        } else {
            self.transport_manager.set_selection_policy(
                TransportSelectionPolicy::PreferenceOrder(vec![
                    TransportType::Nostr,
                    TransportType::Ble,
                ]),
            );
        }
    }

    /// Send a message to a specific peer or broadcast
    pub async fn send_message(
        &mut self,
        recipient_id: Option<PeerId>,
        content: String,
    ) -> Result<Uuid> {
        let message = BitchatMessage::new(self.config.display_name.clone(), content);
        let message_id = message.id;

        // Create packet
        let packet = if let Some(recipient) = recipient_id {
            MessageBuilder::create_message(
                self.peer_id,
                self.config.display_name.clone(),
                message.content.clone(),
                Some(recipient),
            )
        } else {
            MessageBuilder::create_message(
                self.peer_id,
                self.config.display_name.clone(),
                message.content.clone(),
                None,
            )
        }.map_err(|e| CliError::MessageProcessing(format!("Failed to create message: {}", e)))?;

        // Send via transport manager
        if let Some(recipient) = recipient_id {
            self.delivery_tracker
                .track_message(message_id, recipient, packet.payload.clone());
            self.transport_manager.send_to(recipient, packet).await
                .map_err(|e| CliError::MessageProcessing(format!("Failed to send message: {}", e)))?;
            self.delivery_tracker.mark_sent(&message_id);
        } else {
            self.transport_manager.broadcast_all(packet).await
                .map_err(|e| CliError::MessageProcessing(format!("Failed to broadcast message: {}", e)))?;
        }

        // Update state
        self.state_manager.state_mut().add_message(self.peer_id, &message, false);
        
        info!("Message sent: {}", message_id);
        Ok(message_id)
    }

    /// Get list of discovered peers
    pub fn get_discovered_peers(&self) -> Vec<(PeerId, TransportType)> {
        self.transport_manager.all_discovered_peers()
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Get application configuration
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get application state
    pub fn state(&self) -> &AppState {
        self.state_manager.state()
    }

    /// Get event receiver for UI
    pub fn event_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<AppEvent>>> {
        self.event_receiver.clone()
    }

    /// Run the main message processing loop
    pub async fn run_message_loop(&mut self) -> Result<()> {
        info!("Starting message processing loop");
        *self.running.write().await = true;

        // Clone necessary components for the background task
        let running = self.running.clone();

        // Spawn background message processing task (placeholder implementation)
        let _message_task = tokio::spawn(async move {
            while *running.read().await {
                tokio::select! {
                    // Process incoming messages from transports (simplified for now)
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Placeholder for message processing - would need proper transport receive implementation
                        debug!("Message processing loop tick");
                    }

                    // Periodic cleanup and maintenance
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        debug!("Running periodic maintenance");
                        // Cleanup will be handled by the main app
                    }
                }
            }
            info!("Message processing loop ended");
        });

        // Main loop for cleanup and state management
        loop {
            tokio::select! {
                // Periodic cleanup
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    self.session_manager.cleanup_expired();
                    let (completed, expired) = self.delivery_tracker.cleanup();
                    if !completed.is_empty() || !expired.is_empty() {
                        debug!("Cleaned up {} completed and {} expired deliveries",
                               completed.len(), expired.len());
                    }
                    
                    {
                        let mut reassembler = self.reassembler.lock().await;
                        reassembler.cleanup_expired();
                    }

                    // Auto-save state
                    if let Err(e) = self.state_manager.maybe_auto_save() {
                        error!("Failed to auto-save state: {}", e);
                    }

                    // Update peer discovery
                    self.update_peer_discovery().await;
                }

                // Handle shutdown signal
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                    break;
                }
            }

            // Check if app should stop
            if !*self.running.read().await {
                break;
            }
        }

        Ok(())
    }

    /// Update peer discovery and send events
    async fn update_peer_discovery(&mut self) {
        let current_peers = self.get_discovered_peers();
        let state_peers: std::collections::HashSet<_> = self.state_manager.state()
            .discovered_peers
            .keys()
            .cloned()
            .collect();

        for (peer_id, transport_type) in current_peers {
            let peer_id_str = peer_id.to_string();
            
            if !state_peers.contains(&peer_id_str) {
                // New peer discovered
                self.state_manager.state_mut().add_peer(peer_id, transport_type, None);
                
                let _ = self.event_sender.send(AppEvent::PeerDiscovered {
                    peer_id,
                    transport_type,
                });
            }
        }
    }

    /// Stop the application
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping BitChat application");
        
        *self.running.write().await = false;

        // Update runtime statistics
        let runtime = self.start_time.elapsed().as_secs();
        self.state_manager.state_mut().update_runtime(runtime);

        // Save final state
        if let Err(e) = self.state_manager.save() {
            error!("Failed to save final state: {}", e);
        }

        // Stop transports
        self.transport_manager
            .stop_all()
            .await
            .map_err(|e| CliError::TransportInit(format!("Failed to stop transports: {}", e)))?;

        info!("BitChat application stopped");
        Ok(())
    }

    /// Check if application is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get transport status
    pub fn get_transport_status(&self) -> Vec<(TransportType, bool)> {
        // This would need to be implemented in the transport manager
        vec![
            (TransportType::Ble, true),
            (TransportType::Nostr, true),
        ]
    }

    /// Get application statistics
    pub fn get_stats(&self) -> (crate::state::AppStats, DeliveryStats) {
        let app_stats = self.state_manager.state().stats.clone();
        let delivery_stats = self.delivery_tracker.get_stats();
        (app_stats, delivery_stats)
    }
}