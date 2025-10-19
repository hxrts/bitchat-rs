//! Terminal Interface Implementation
//!
//! Implements terminal UI task with traditional concurrent patterns and cross-platform support
//! Moved from bitchat-core to bitchat-cli crate for better architectural separation.

use bitchat_core::{
    PeerId, Command, AppEvent, ChannelTransportType, ConnectionStatus,
    BitchatError, BitchatResult,
    internal::{
        CommandSender, AppEventReceiver, TaskId, LogLevel, TransportError
    }
};
use bitchat_runtime::logic::LoggerWrapper;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::mpsc;

// ----------------------------------------------------------------------------
// UI State Management
// ----------------------------------------------------------------------------

/// UI state using traditional concurrent patterns (Arc<Mutex<T>>)
#[derive(Debug, Clone)]
pub struct UIState {
    /// Current discovery state
    pub discovery_active: bool,
    /// Connected peers and their status
    pub peers: HashMap<PeerId, PeerUIState>,
    /// Recent messages for display
    pub recent_messages: Vec<UIMessage>,
    /// System status
    pub system_status: SystemStatus,
    /// UI busy indicator
    pub busy_operations: Vec<String>,
}

/// Per-peer UI state
#[derive(Debug, Clone)]
pub struct PeerUIState {
    pub peer_id: PeerId,
    pub status: ConnectionStatus,
    pub transport: Option<ChannelTransportType>,
    pub last_seen: Option<u64>,
    pub message_count: u32,
}

/// UI-formatted message
#[derive(Debug, Clone)]
pub struct UIMessage {
    pub from: PeerId,
    pub to: Option<PeerId>,
    pub content: String,
    pub timestamp: u64,
    pub direction: MessageDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDirection {
    Incoming,
    Outgoing,
}

/// System status for UI display
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemStatus {
    Starting,
    Ready,
    Busy(String),
    Error(String),
    ShuttingDown,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            discovery_active: false,
            peers: HashMap::new(),
            recent_messages: Vec::new(),
            system_status: SystemStatus::Starting,
            busy_operations: Vec::new(),
        }
    }
}

// ----------------------------------------------------------------------------
// Terminal Interface Task
// ----------------------------------------------------------------------------

/// Terminal interface task managing user interactions and display updates
pub struct TerminalInterfaceTask {
    /// Shared UI state using Arc<Mutex<T>> for traditional concurrent patterns
    state: Arc<Mutex<UIState>>,
    /// Channel for sending commands to Core Logic
    command_sender: CommandSender,
    /// Channel for receiving app events from Core Logic
    app_event_receiver: AppEventReceiver,
    /// Logger for task communication
    logger: LoggerWrapper,
    /// Task running state
    running: bool,
    /// Our peer ID for filtering messages
    our_peer_id: PeerId,
}

impl TerminalInterfaceTask {
    /// Create new terminal interface task
    pub fn new(
        our_peer_id: PeerId,
        command_sender: CommandSender,
        app_event_receiver: AppEventReceiver,
        logger: LoggerWrapper,
    ) -> Self {
        let initial_state = UIState {
            system_status: SystemStatus::Starting,
            ..Default::default()
        };

        Self {
            state: Arc::new(Mutex::new(initial_state)),
            command_sender,
            app_event_receiver,
            logger,
            running: false,
            our_peer_id,
        }
    }

    /// Get shared state handle for UI rendering
    pub fn state(&self) -> Arc<Mutex<UIState>> {
        self.state.clone()
    }

    /// Run the terminal interface task main loop
    #[cfg(feature = "std")]
    pub async fn run(&mut self) -> BitchatResult<()> {
        self.logger.log_task_event(
            TaskId::UI,
            LogLevel::Info,
            "Terminal interface task starting"
        );

        self.running = true;
        
        // Update system status to ready
        {
            let mut state = self.state.lock().unwrap();
            state.system_status = SystemStatus::Ready;
        }

        while self.running {
            tokio::select! {
                // Process app events from Core Logic
                app_event = self.app_event_receiver.recv() => {
                    match app_event {
                        Some(event) => {
                            if let Err(e) = self.process_app_event(event).await {
                                self.logger.log_task_event(
                                    TaskId::UI,
                                    LogLevel::Error,
                                    &format!("Error processing app event: {}", e)
                                );
                            }
                        }
                        None => {
                            self.logger.log_task_event(
                                TaskId::UI,
                                LogLevel::Info,
                                "App event channel closed"
                            );
                            break;
                        }
                    }
                }

                // Periodic UI maintenance
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                    self.perform_ui_maintenance().await;
                }
            }
        }

        // Update system status to shutting down
        {
            let mut state = self.state.lock().unwrap();
            state.system_status = SystemStatus::ShuttingDown;
        }

        self.logger.log_task_event(
            TaskId::UI,
            LogLevel::Info,
            "Terminal interface task stopped"
        );

        Ok(())
    }

    /// Stop the terminal interface task
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Process app event from Core Logic with graceful degradation
    async fn process_app_event(&mut self, app_event: AppEvent) -> BitchatResult<()> {
        let mut state = self.state.lock().map_err(|_| BitchatError::Transport(TransportError::InvalidConfiguration { 
            reason: "UI state lock poisoned".to_string() 
        }))?;

        match app_event {
            AppEvent::MessageReceived { from, content, timestamp } => {
                // Add to recent messages
                let ui_message = UIMessage {
                    from,
                    to: Some(self.our_peer_id),
                    content,
                    timestamp,
                    direction: MessageDirection::Incoming,
                };
                state.recent_messages.push(ui_message);
                
                // Update peer message count
                if let Some(peer_state) = state.peers.get_mut(&from) {
                    peer_state.message_count += 1;
                    peer_state.last_seen = Some(timestamp);
                }

                // Trim to last 100 messages
                if state.recent_messages.len() > 100 {
                    state.recent_messages.remove(0);
                }
            }

            AppEvent::MessageSent { to, content, timestamp } => {
                // Add to recent messages
                let ui_message = UIMessage {
                    from: self.our_peer_id,
                    to: Some(to),
                    content,
                    timestamp,
                    direction: MessageDirection::Outgoing,
                };
                state.recent_messages.push(ui_message);

                // Update peer message count
                if let Some(peer_state) = state.peers.get_mut(&to) {
                    peer_state.message_count += 1;
                }

                // Trim to last 100 messages
                if state.recent_messages.len() > 100 {
                    state.recent_messages.remove(0);
                }
            }

            AppEvent::PeerStatusChanged { peer_id, status, transport } => {
                // Update or create peer state
                let peer_state = state.peers.entry(peer_id).or_insert_with(|| PeerUIState {
                    peer_id,
                    status: ConnectionStatus::Disconnected,
                    transport: None,
                    last_seen: None,
                    message_count: 0,
                });
                
                peer_state.status = status;
                peer_state.transport = transport;
                peer_state.last_seen = Some(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64);
            }

            AppEvent::DiscoveryStateChanged { active, transport: _ } => {
                state.discovery_active = active;
                
                if active {
                    state.busy_operations.push("Discovery active".to_string());
                } else {
                    state.busy_operations.retain(|op| !op.contains("Discovery"));
                }
            }

            AppEvent::ConversationUpdated { peer_id, message_count, last_message_time: _ } => {
                if let Some(peer_state) = state.peers.get_mut(&peer_id) {
                    peer_state.message_count = message_count as u32;
                }
            }

            AppEvent::SystemBusy { reason } => {
                state.system_status = SystemStatus::Busy(reason.clone());
                if !state.busy_operations.contains(&reason) {
                    state.busy_operations.push(reason);
                }
            }

            AppEvent::SystemError { error } => {
                state.system_status = SystemStatus::Error(error);
            }
            AppEvent::SystemStatusReport { peer_count, active_connections, message_count, uptime_seconds, transport_status, memory_usage_bytes } => {
                // Log detailed system status (could be displayed in UI later)
                tracing::info!("System status: {} peers, {} active connections, {} messages, {}s uptime, {} transports, {:?} MB memory", 
                    peer_count, active_connections, message_count, uptime_seconds, transport_status.len(), memory_usage_bytes.map(|b| b / (1024 * 1024)));
            }
        }

        Ok(())
    }

    /// Send command to Core Logic with non-blocking try_send
    pub async fn send_command(&self, command: Command) -> BitchatResult<()> {
        // Use try_send for non-blocking communication with graceful degradation
        match self.command_sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Channel is full - indicate system busy
                {
                    let mut state = self.state.lock().map_err(|_| BitchatError::Transport(TransportError::InvalidConfiguration { 
                        reason: "UI state lock poisoned".to_string() 
                    }))?;
                    state.system_status = SystemStatus::Busy("Core Logic overloaded".to_string());
                    if !state.busy_operations.contains(&"Core Logic overloaded".to_string()) {
                        state.busy_operations.push("Core Logic overloaded".to_string());
                    }
                }
                
                self.logger.log_task_event(
                    TaskId::UI,
                    LogLevel::Warn,
                    "Command channel full - Core Logic overloaded"
                );
                
                Err(BitchatError::Transport(TransportError::SendBufferFull { 
                    capacity: 0 // Channel full
                }))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(BitchatError::Transport(TransportError::Shutdown { 
                    reason: "Command channel closed".to_string() 
                }))
            }
        }
    }

    /// Perform periodic UI maintenance
    async fn perform_ui_maintenance(&mut self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return, // Poisoned lock
        };

        // Clear resolved busy operations
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let discovery_active = state.discovery_active;
        state.busy_operations.retain(|op| {
            // Keep operations that are still relevant
            !op.contains("Discovery") || discovery_active
        });

        // Update system status based on busy operations
        if state.busy_operations.is_empty() {
            if matches!(state.system_status, SystemStatus::Busy(_)) {
                state.system_status = SystemStatus::Ready;
            }
        }

        // Clean up old peers that haven't been seen recently
        let timeout_threshold = 300 * 1000; // 5 minutes in milliseconds
        let mut stale_peers = Vec::new();
        
        for (peer_id, peer_state) in &state.peers {
            if let Some(last_seen) = peer_state.last_seen {
                if current_time - last_seen > timeout_threshold {
                    if matches!(peer_state.status, ConnectionStatus::Disconnected) {
                        stale_peers.push(*peer_id);
                    }
                }
            }
        }

        for peer_id in stale_peers {
            state.peers.remove(&peer_id);
        }
    }

    /// Get current UI state snapshot (non-blocking)
    pub fn get_state_snapshot(&self) -> Option<UIState> {
        self.state.try_lock().ok().map(|state| state.clone())
    }

    /// Check if terminal interface task is running
    pub fn is_running(&self) -> bool {
        self.running
    }
}

// ----------------------------------------------------------------------------
// User Action Handlers
// ----------------------------------------------------------------------------

impl TerminalInterfaceTask {
    /// Handle user action to send message
    pub async fn handle_send_message(&self, recipient: PeerId, content: String) -> BitchatResult<()> {
        let command = Command::SendMessage { recipient, content };
        self.send_command(command).await
    }

    /// Handle user action to connect to peer
    pub async fn handle_connect_to_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        let command = Command::ConnectToPeer { peer_id };
        self.send_command(command).await
    }

    /// Handle user action to start discovery
    pub async fn handle_start_discovery(&self) -> BitchatResult<()> {
        let command = Command::StartDiscovery;
        self.send_command(command).await
    }

    /// Handle user action to stop discovery
    pub async fn handle_stop_discovery(&self) -> BitchatResult<()> {
        let command = Command::StopDiscovery;
        self.send_command(command).await
    }

    /// Handle user action to disconnect from peer
    pub async fn handle_disconnect_from_peer(&self, peer_id: PeerId) -> BitchatResult<()> {
        let command = Command::DisconnectFromPeer { peer_id };
        self.send_command(command).await
    }

    /// Handle user action to shutdown
    pub async fn handle_shutdown(&self) -> BitchatResult<()> {
        let command = Command::Shutdown;
        self.send_command(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::internal::{create_command_channel, create_app_event_channel, ChannelConfig};
    use bitchat_core::internal::{ConsoleLogger, LogLevel};
    use bitchat_runtime::logic::LoggerWrapper;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[tokio::test]
    async fn test_terminal_interface_app_event_processing() {
        let config = ChannelConfig {
            command_buffer_size: 10,
            event_buffer_size: 10,
            effect_buffer_size: 10,
            app_event_buffer_size: 10,
        };

        let (command_sender, _command_receiver) = create_command_channel(&config);
        let (_app_event_sender, app_event_receiver) = create_app_event_channel(&config);
        let logger = LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Debug));
        
        let our_peer_id = create_test_peer_id(1);
        let mut terminal_interface = TerminalInterfaceTask::new(our_peer_id, command_sender, app_event_receiver, logger);

        // Test message received processing
        let from_peer = create_test_peer_id(2);
        let app_event = AppEvent::MessageReceived {
            from: from_peer,
            content: "Hello".to_string(),
            timestamp: 12345,
        };

        terminal_interface.process_app_event(app_event).await.unwrap();

        let state = terminal_interface.get_state_snapshot().unwrap();
        assert_eq!(state.recent_messages.len(), 1);
        assert_eq!(state.recent_messages[0].content, "Hello");
        assert_eq!(state.recent_messages[0].direction, MessageDirection::Incoming);
    }

    #[test]
    fn test_ui_state_default() {
        let state = UIState::default();
        assert!(!state.discovery_active);
        assert!(state.peers.is_empty());
        assert!(state.recent_messages.is_empty());
        assert_eq!(state.system_status, SystemStatus::Starting);
    }
}