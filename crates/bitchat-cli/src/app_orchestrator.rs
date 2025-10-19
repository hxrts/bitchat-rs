//! CLI Application Orchestrator
//!
//! Provides a reusable orchestrator for setting up and managing the BitChat CLI application

use bitchat_core::{
    PeerId, ChannelTransportType, BitchatResult, BitchatError,
    EventSender, TransportTask,
    config::TestConfig,
    internal::{
        ChannelConfig, SessionConfig, DeliveryConfig, RateLimitConfig,
        EffectSender,
        create_command_channel, create_event_channel, create_effect_channel, create_effect_receiver, create_app_event_channel,
        ConsoleLogger, LogLevel, TransportError
    }
};
use bitchat_runtime::logic::{CoreLogicTask, LoggerWrapper};
use bitchat_ble::BleTransportTask;
use crate::terminal_interface::TerminalInterfaceTask;
use tokio::task::JoinHandle;

// ----------------------------------------------------------------------------
// CLI Application Orchestrator
// ----------------------------------------------------------------------------

/// Transport configuration for CLI
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// List of enabled transports
    pub enabled_transports: Vec<ChannelTransportType>,
    /// BLE-specific configuration
    pub ble_enabled: bool,
    /// Nostr-specific configuration  
    pub nostr_enabled: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enabled_transports: vec![ChannelTransportType::Ble], // Default to BLE only
            ble_enabled: true,
            nostr_enabled: false,
        }
    }
}

impl TransportConfig {
    /// Enable all available transports
    pub fn all_transports() -> Self {
        Self {
            enabled_transports: vec![ChannelTransportType::Ble, ChannelTransportType::Nostr],
            ble_enabled: true,
            nostr_enabled: true,
        }
    }

    /// Enable only BLE transport
    pub fn ble_only() -> Self {
        Self {
            enabled_transports: vec![ChannelTransportType::Ble],
            ble_enabled: true,
            nostr_enabled: false,
        }
    }

    /// Enable only Nostr transport
    pub fn nostr_only() -> Self {
        Self {
            enabled_transports: vec![ChannelTransportType::Nostr],
            ble_enabled: false,
            nostr_enabled: true,
        }
    }
}

/// Orchestrator for the BitChat CLI application
pub struct CliAppOrchestrator {
    /// Peer identity
    peer_id: PeerId,
    /// Configuration
    config: TestConfig,
    /// Transport configuration
    transport_config: TransportConfig,
    /// Verbose logging enabled
    verbose: bool,
    /// Core Logic task handle
    core_logic_handle: Option<JoinHandle<BitchatResult<()>>>,
    /// Transport task handles
    transport_handles: Vec<JoinHandle<BitchatResult<()>>>,
    /// Terminal interface for external interaction
    terminal_interface: Option<TerminalInterfaceTask>,
    /// Running state
    running: bool,
}

impl CliAppOrchestrator {
    /// Create new CLI orchestrator with default transport configuration
    pub fn new(peer_id: PeerId, verbose: bool) -> Self {
        Self::with_transports(peer_id, verbose, TransportConfig::default())
    }

    /// Create new CLI orchestrator with specific transport configuration
    pub fn with_transports(peer_id: PeerId, verbose: bool, transport_config: TransportConfig) -> Self {
        let config = if verbose {
            TestConfig::new().with_peer_id(peer_id).with_logging()
        } else {
            TestConfig::new().with_peer_id(peer_id)
        };

        Self {
            peer_id,
            config,
            transport_config,
            verbose,
            core_logic_handle: None,
            transport_handles: Vec::new(),
            terminal_interface: None,
            running: false,
        }
    }

    /// Create orchestrator from TestConfig with default transports
    pub fn from_config(config: TestConfig, verbose: bool) -> Self {
        Self::from_config_with_transports(config, verbose, TransportConfig::default())
    }

    /// Create orchestrator from TestConfig with specific transport configuration
    pub fn from_config_with_transports(config: TestConfig, verbose: bool, transport_config: TransportConfig) -> Self {
        Self {
            peer_id: config.peer_id,
            config,
            transport_config,
            verbose,
            core_logic_handle: None,
            transport_handles: Vec::new(),
            terminal_interface: None,
            running: false,
        }
    }

    /// Start the CLI application
    pub async fn start(&mut self) -> BitchatResult<()> {
        if self.running {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "CLI application already running".to_string(),
            }));
        }

        // Create channels optimized for CLI usage
        let channel_config = ChannelConfig {
            command_buffer_size: 50,
            event_buffer_size: 100,
            effect_buffer_size: 100,
            app_event_buffer_size: 200, // Larger for responsive UI
        };

        let (command_sender, command_receiver) = create_command_channel(&channel_config);
        let (event_sender, event_receiver) = create_event_channel(&channel_config);
        let (effect_sender, _initial_effect_receiver) = create_effect_channel(&channel_config);
        let (app_event_sender, app_event_receiver) = create_app_event_channel(&channel_config);
        
        // Clone effect_sender for creating transport subscriptions
        let effect_sender_for_transports = effect_sender.clone();
        
        // Create logger
        let logger = if self.verbose {
            LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Debug).with_timestamps(false))
        } else {
            LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Info).with_timestamps(false))
        };

        // Start Core Logic task
        let mut core_logic = CoreLogicTask::new(
            self.peer_id,
            command_receiver,
            event_receiver,
            effect_sender,
            app_event_sender,
            logger.clone(),
            SessionConfig::default(),
            DeliveryConfig::default(),
            RateLimitConfig::default(),
        )?;

        let core_handle = tokio::spawn(async move {
            core_logic.run().await
        });
        self.core_logic_handle = Some(core_handle);

        // Start transport tasks based on configuration
        self.start_configured_transports(event_sender, effect_sender_for_transports, logger.clone()).await?;

        // Create and store terminal interface for external access
        let terminal_interface = TerminalInterfaceTask::new(
            self.peer_id,
            command_sender,
            app_event_receiver,
            logger,
        );
        self.terminal_interface = Some(terminal_interface);

        // Note: We don't start the terminal interface task here - it will be managed externally

        self.running = true;

        Ok(())
    }

    /// Stop the CLI application
    pub async fn stop(&mut self) -> BitchatResult<()> {
        if !self.running {
            return Ok(());
        }

        self.running = false;

        // Clear terminal interface
        self.terminal_interface = None;

        // Stop all transport tasks
        for handle in self.transport_handles.drain(..) {
            handle.abort();
        }

        // Stop core logic task
        if let Some(handle) = self.core_logic_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Get terminal interface for sending commands
    pub fn terminal_interface(&self) -> Option<&TerminalInterfaceTask> {
        self.terminal_interface.as_ref()
    }

    /// Check if application is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Start configured transport tasks
    async fn start_configured_transports(
        &mut self,
        event_sender: EventSender,
        effect_sender: EffectSender,
        logger: LoggerWrapper,
    ) -> BitchatResult<()> {
        if self.transport_config.enabled_transports.is_empty() {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "No transports enabled in configuration".to_string(),
            }));
        }

        // Start multiple transports, each with its own subscription to the broadcast channel
        for transport_type in &self.transport_config.enabled_transports {
            match transport_type {
                ChannelTransportType::Ble if self.transport_config.ble_enabled => {
                    let handle = self.start_ble_transport(
                        event_sender.clone(),
                        effect_sender.clone(),
                        logger.clone(),
                    ).await?;
                    self.transport_handles.push(handle);
                }
                ChannelTransportType::Nostr if self.transport_config.nostr_enabled => {
                    // Note: Nostr transport would be started here
                    // let handle = self.start_nostr_transport(
                    //     event_sender.clone(),
                    //     effect_sender.clone(),
                    //     logger.clone(),
                    // ).await?;
                    // self.transport_handles.push(handle);
                    continue;
                }
                _ => {
                    // Skip disabled or unsupported transports
                    continue;
                }
            }
        }

        if self.transport_handles.is_empty() {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "No supported transports could be started".to_string(),
            }));
        }

        Ok(())
    }

    /// Start BLE transport task
    async fn start_ble_transport(
        &self,
        event_sender: EventSender,
        effect_sender: EffectSender,
        logger: LoggerWrapper,
    ) -> BitchatResult<JoinHandle<BitchatResult<()>>> {
        let mut ble_task = BleTransportTask::new();
        let effect_receiver = create_effect_receiver(&effect_sender);
        ble_task.attach_channels(event_sender, effect_receiver)?;

        let handle = tokio::spawn(async move {
            ble_task.run().await
        });

        Ok(handle)
    }
}

// ----------------------------------------------------------------------------
// Convenience Functions
// ----------------------------------------------------------------------------

/// Create and start a CLI application
pub async fn start_cli_application(peer_id: PeerId, verbose: bool) -> BitchatResult<CliAppOrchestrator> {
    let mut orchestrator = CliAppOrchestrator::new(peer_id, verbose);
    orchestrator.start().await?;
    Ok(orchestrator)
}

/// Create and start a CLI application from config
pub async fn start_cli_application_with_config(config: TestConfig, verbose: bool) -> BitchatResult<CliAppOrchestrator> {
    let mut orchestrator = CliAppOrchestrator::from_config(config, verbose);
    orchestrator.start().await?;
    Ok(orchestrator)
}

/// Create and start a CLI application with specific transport configuration
pub async fn start_cli_application_with_transports(
    peer_id: PeerId, 
    verbose: bool, 
    transport_config: TransportConfig
) -> BitchatResult<CliAppOrchestrator> {
    let mut orchestrator = CliAppOrchestrator::with_transports(peer_id, verbose, transport_config);
    orchestrator.start().await?;
    Ok(orchestrator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let orchestrator = CliAppOrchestrator::new(peer_id, false);
        
        assert_eq!(orchestrator.peer_id(), peer_id);
        assert!(!orchestrator.is_running());
    }

    #[tokio::test]
    async fn test_orchestrator_lifecycle() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let mut orchestrator = CliAppOrchestrator::new(peer_id, false);

        assert!(!orchestrator.is_running());
        
        // Note: We can't easily test start() here without proper transport setup
        // This would require integration testing with actual transport crates
    }
}
