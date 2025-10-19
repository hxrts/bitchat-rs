//! Runtime Builder API
//!
//! Provides a builder-style API for consumers (CLI/web/tests) to register
//! transports via harness builders and get command/app-event handles.

use crate::supervisor::SupervisorTask;
use alloc::{boxed::Box, string::String, vec::Vec};
use bitchat_core::{
    internal::{
        create_app_event_channel, create_command_channel, create_effect_channel,
        create_effect_receiver, create_event_channel, AppEventReceiver, BitchatConfig,
        CommandSender, LogLevel,
    },
    BitchatError, BitchatResult, Command, PeerId, TransportTask,
};
use bitchat_harness::{TransportBuilder, TransportHandle};
use std::collections::HashMap;
use tokio::{task::JoinHandle, time::Duration};

#[cfg(not(feature = "std"))]
use log::{info, warn};
#[cfg(feature = "std")]
use tracing::info;

// ----------------------------------------------------------------------------
// Runtime Builder
// ----------------------------------------------------------------------------

/// Builder for creating BitChat runtime with decomposed task architecture
pub struct RuntimeBuilder {
    peer_id: PeerId,
    config: BitchatConfig,
    transports: Vec<Box<dyn TransportTask>>,
    transport_builders: HashMap<String, TransportBuilder>,
    enable_logging: bool,
    monitoring_config: MonitoringConfig,
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub health_check_interval: Duration,
    pub restart_failed_tasks: bool,
    pub max_restart_attempts: u32,
    pub channel_buffer_size: usize,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            restart_failed_tasks: true,
            max_restart_attempts: 3,
            channel_buffer_size: 1000,
        }
    }
}

impl RuntimeBuilder {
    /// Create a new runtime builder
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            config: BitchatConfig::default(),
            transports: Vec::new(),
            transport_builders: HashMap::new(),
            enable_logging: false,
            monitoring_config: MonitoringConfig::default(),
        }
    }

    /// Set the BitChat configuration
    pub fn with_config(mut self, config: BitchatConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a transport task
    pub fn add_transport(mut self, transport: Box<dyn TransportTask>) -> Self {
        self.transports.push(transport);
        self
    }

    /// Register a transport builder for configuration
    pub fn register_transport_builder(mut self, name: String, builder: TransportBuilder) -> Self {
        self.transport_builders.insert(name, builder);
        self
    }

    /// Configure console logging
    pub fn with_console_logging(mut self, _level: LogLevel) -> Self {
        self.enable_logging = true;
        self
    }

    /// Disable logging
    pub fn with_no_logging(mut self) -> Self {
        self.enable_logging = false;
        self
    }

    /// Configure monitoring behavior
    pub fn with_monitoring(mut self, config: MonitoringConfig) -> Self {
        self.monitoring_config = config;
        self
    }

    /// Set monitoring interval
    pub fn monitoring_interval(mut self, interval: Duration) -> Self {
        self.monitoring_config.health_check_interval = interval;
        self
    }

    /// Enable/disable automatic task restart
    pub fn restart_failed_tasks(mut self, enabled: bool) -> Self {
        self.monitoring_config.restart_failed_tasks = enabled;
        self
    }

    /// Set channel buffer sizes
    pub fn channel_buffer_size(mut self, size: usize) -> Self {
        self.monitoring_config.channel_buffer_size = size;
        self
    }

    /// Build and start the runtime
    pub async fn build_and_start(self) -> BitchatResult<RuntimeHandle> {
        info!("Building BitChat runtime with decomposed architecture");

        let _buffer_size = self.monitoring_config.channel_buffer_size;

        // Create main channels using the bitchat-core channel configuration
        let channel_config = self.config.channels.clone();
        let (command_sender, command_receiver) = create_command_channel(&channel_config);
        let (app_event_sender, app_event_receiver) = create_app_event_channel(&channel_config);
        let (event_sender, event_receiver) = create_event_channel(&channel_config);
        let (effect_sender, _effect_receiver) = create_effect_channel(&channel_config);

        // Create and start supervisor
        let mut supervisor = SupervisorTask::new(
            self.peer_id,
            self.monitoring_config.health_check_interval,
            self.monitoring_config.restart_failed_tasks,
            self.monitoring_config.max_restart_attempts,
        );

        // Start the supervisor with decomposed tasks
        supervisor
            .start(
                command_receiver,
                event_receiver,
                effect_sender.clone(),
                app_event_sender,
                self.config.session.clone(),
                self.config.delivery.clone(),
                self.config.rate_limiting.clone(),
            )
            .await?;

        // Set up transports with harness
        let mut transport_handles = Vec::new();
        for mut transport in self.transports {
            let effect_receiver = create_effect_receiver(&effect_sender);
            let _transport_handle =
                TransportHandle::new(event_sender.clone(), create_effect_receiver(&effect_sender));

            transport.attach_channels(event_sender.clone(), effect_receiver)?;

            let handle = tokio::spawn(async move { transport.run().await });

            transport_handles.push(handle);
        }

        // Start supervisor task
        let supervisor_handle = tokio::spawn(async move { supervisor.run().await });

        info!("BitChat runtime started successfully");

        Ok(RuntimeHandle {
            peer_id: self.peer_id,
            command_sender,
            app_event_receiver: Some(app_event_receiver),
            supervisor_handle: Some(supervisor_handle),
            transport_handles,
            running: true,
        })
    }
}

// ----------------------------------------------------------------------------
// Runtime Handle
// ----------------------------------------------------------------------------

/// Handle to a running BitChat runtime instance
pub struct RuntimeHandle {
    peer_id: PeerId,
    command_sender: CommandSender,
    app_event_receiver: Option<AppEventReceiver>,
    supervisor_handle: Option<JoinHandle<BitchatResult<()>>>,
    transport_handles: Vec<JoinHandle<BitchatResult<()>>>,
    running: bool,
}

impl RuntimeHandle {
    /// Get the peer ID for this runtime
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Get a command sender for sending commands to the runtime
    pub fn command_sender(&self) -> CommandSender {
        self.command_sender.clone()
    }

    /// Take the app event receiver (can only be called once)
    pub fn take_app_event_receiver(&mut self) -> Option<AppEventReceiver> {
        self.app_event_receiver.take()
    }

    /// Send a command to the runtime
    pub async fn send_command(&self, command: Command) -> BitchatResult<()> {
        self.command_sender
            .send(command)
            .await
            .map_err(|_| BitchatError::Channel {
                message: "Failed to send command to runtime".to_string(),
            })
    }

    /// Check if the runtime is still running
    pub fn is_running(&self) -> bool {
        self.running
            && self
                .supervisor_handle
                .as_ref()
                .is_some_and(|h| !h.is_finished())
    }

    /// Wait for the runtime to complete
    pub async fn wait(&mut self) -> BitchatResult<()> {
        if let Some(handle) = self.supervisor_handle.take() {
            match handle.await {
                Ok(result) => result,
                Err(e) => Err(BitchatError::Channel {
                    message: format!("Supervisor task panicked: {}", e),
                }),
            }
        } else {
            Ok(())
        }
    }

    /// Shutdown the runtime gracefully
    pub async fn shutdown(&mut self) -> BitchatResult<()> {
        info!("Shutting down BitChat runtime");

        // Send shutdown command
        let _ = self.send_command(Command::Shutdown).await;

        // Wait for supervisor to complete
        if let Some(handle) = self.supervisor_handle.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), handle).await;
        }

        // Abort any remaining transport tasks
        for handle in &self.transport_handles {
            handle.abort();
        }

        self.running = false;
        info!("BitChat runtime shut down");
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Convenience Functions
// ----------------------------------------------------------------------------

/// Create a runtime with default configuration for testing
pub async fn create_test_runtime(peer_id: PeerId) -> BitchatResult<RuntimeHandle> {
    RuntimeBuilder::new(peer_id)
        .with_no_logging()
        .monitoring_interval(Duration::from_millis(100)) // Fast monitoring for tests
        .restart_failed_tasks(false) // Don't restart in tests
        .channel_buffer_size(100)
        .build_and_start()
        .await
}

/// Create a runtime with console logging for CLI applications
pub async fn create_cli_runtime(
    peer_id: PeerId,
    config: BitchatConfig,
) -> BitchatResult<RuntimeHandle> {
    RuntimeBuilder::new(peer_id)
        .with_config(config)
        .with_console_logging(LogLevel::Info)
        .build_and_start()
        .await
}

/// Create a runtime optimized for browser/WASM environments
pub async fn create_browser_runtime(peer_id: PeerId) -> BitchatResult<RuntimeHandle> {
    let config = BitchatConfig::browser_optimized();

    RuntimeBuilder::new(peer_id)
        .with_config(config)
        .with_no_logging() // Reduce WASM bundle size
        .monitoring_interval(Duration::from_secs(60)) // Less frequent monitoring
        .restart_failed_tasks(true)
        .channel_buffer_size(500)
        .build_and_start()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_builder() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        let mut runtime = RuntimeBuilder::new(peer_id)
            .with_no_logging()
            .monitoring_interval(Duration::from_millis(50))
            .build_and_start()
            .await
            .expect("Failed to build runtime");

        assert_eq!(runtime.peer_id(), peer_id);
        assert!(runtime.is_running());

        // Test command sending
        let command = Command::StartDiscovery;
        runtime
            .send_command(command)
            .await
            .expect("Failed to send command");

        // Shutdown
        runtime.shutdown().await.expect("Failed to shutdown");
        assert!(!runtime.is_running());
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // Test runtime
        let mut test_runtime = create_test_runtime(peer_id)
            .await
            .expect("Failed to create test runtime");
        assert!(test_runtime.is_running());
        test_runtime
            .shutdown()
            .await
            .expect("Failed to shutdown test runtime");

        // Browser runtime
        let mut browser_runtime = create_browser_runtime(peer_id)
            .await
            .expect("Failed to create browser runtime");
        assert!(browser_runtime.is_running());
        browser_runtime
            .shutdown()
            .await
            .expect("Failed to shutdown browser runtime");
    }

    #[tokio::test]
    async fn test_app_event_receiver() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        let mut runtime = create_test_runtime(peer_id)
            .await
            .expect("Failed to create runtime");

        // Take app event receiver
        let _app_events = runtime
            .take_app_event_receiver()
            .expect("Failed to get app event receiver");

        // Should only be able to take once
        assert!(runtime.take_app_event_receiver().is_none());

        // Send a command that should generate an app event
        runtime
            .send_command(Command::StartDiscovery)
            .await
            .expect("Failed to send command");

        // Give some time for processing
        tokio::time::sleep(Duration::from_millis(10)).await;

        runtime.shutdown().await.expect("Failed to shutdown");
    }
}
