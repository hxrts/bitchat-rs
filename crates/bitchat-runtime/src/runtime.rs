//! BitChat Runtime
//!
//! Provides core runtime management for setting up and coordinating BitChat system
//! components across different environments (CLI, tests, WASM, etc.)
//!
//! ## Generic Architecture
//!
//! The [`BitchatRuntime`] can manage any number of transport implementations, allowing different
//! applications to plug in their own network transports while keeping the core logic unchanged.
//!
//! ### For Application Developers
//!
//! When building an application (CLI, GUI, web server), you register the transport tasks
//! you need and start the runtime:
//!
//! ```rust,no_run
//! use bitchat_core::{
//!     TransportTask, PeerId,
//!     internal::BitchatConfig
//! };
//! use bitchat_runtime::BitchatRuntime;
//!
//! // Example: Using BLE and Nostr transports
//! # struct BleTransportTask;
//! # struct NostrTransportTask;
//! # #[async_trait::async_trait]
//! # impl TransportTask for BleTransportTask {
//! #     fn attach_channels(
//! #         &mut self,
//! #         _event_sender: bitchat_core::EventSender,
//! #         _effect_receiver: bitchat_core::EffectReceiver,
//! #     ) -> bitchat_core::BitchatResult<()> { Ok(()) }
//! #     async fn run(&mut self) -> bitchat_core::BitchatResult<()> { Ok(()) }
//! #     fn transport_type(&self) -> bitchat_core::ChannelTransportType {
//! #         bitchat_core::ChannelTransportType::Ble
//! #     }
//! # }
//! # #[async_trait::async_trait]
//! # impl TransportTask for NostrTransportTask {
//! #     fn attach_channels(
//! #         &mut self,
//! #         _event_sender: bitchat_core::EventSender,
//! #         _effect_receiver: bitchat_core::EffectReceiver,
//! #     ) -> bitchat_core::BitchatResult<()> { Ok(()) }
//! #     async fn run(&mut self) -> bitchat_core::BitchatResult<()> { Ok(()) }
//! #     fn transport_type(&self) -> bitchat_core::ChannelTransportType {
//! #         bitchat_core::ChannelTransportType::Nostr
//! #     }
//! # }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create runtime
//! let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//! let config = BitchatConfig::default();
//! let mut runtime = BitchatRuntime::new(peer_id, config);
//!
//! // Register transport tasks
//! # let ble_transport = BleTransportTask;
//! # let nostr_transport = NostrTransportTask;
//! runtime.add_transport(ble_transport);
//! runtime.add_transport(nostr_transport);
//!
//! // Start the runtime (which spawns transport.run() for each transport)
//! runtime.start().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### For Testing
//!
//! Convenience functions provide ready-to-use runtimes with stub transports for testing.

use crate::logic::{CoreLogicTask, LoggerWrapper};
use bitchat_core::{
    internal::{
        create_app_event_channel, create_command_channel, create_effect_channel,
        create_effect_receiver, create_event_channel, AppEventReceiver, BitchatConfig,
        CommandSender, ConsoleLogger, LogLevel, NoOpLogger, TaskId, TaskLogger, TransportError,
    },
    BitchatError, BitchatResult, ChannelTransportType, EffectReceiver, EventSender, PeerId,
    TransportTask,
};
use std::collections::HashMap;
use tokio::task::JoinHandle;

// ----------------------------------------------------------------------------
// BitChat Runtime
// ----------------------------------------------------------------------------

/// Core runtime for coordinating BitChat system components
///
/// The runtime can manage any number of transport implementations, allowing different
/// applications (CLI, WASM, tests) to plug in their own transport tasks while keeping
/// the core logic unchanged.
///
/// ## Design Trade-offs
///
/// The current architecture serializes all core logic through a single `CoreLogicTask`.
/// This provides excellent correctness guarantees (no race conditions, no shared state bugs)
/// but may become a performance bottleneck under very high load. The design anticipates
/// future decomposition if needed - see the internal structure of `CoreState` which already
/// separates `SessionManager`, `DeliveryTracker`, and `MessageStore` for potential extraction.
pub struct BitchatRuntime {
    /// Configuration for the application
    config: BitchatConfig,
    /// Peer identity
    peer_id: PeerId,
    /// Logger wrapper
    logger: LoggerWrapper,
    /// Registered transport tasks (before start)
    pending_transports: Vec<Box<dyn TransportTask>>,
    /// Running transport task handles (after start)
    transport_handles: HashMap<ChannelTransportType, JoinHandle<BitchatResult<()>>>,
    /// Core Logic task handle
    core_logic_handle: Option<JoinHandle<BitchatResult<()>>>,
    /// Command sender for external use
    command_sender: Option<CommandSender>,
    /// App event receiver for external use
    app_event_receiver: Option<AppEventReceiver>,
    /// Running state
    running: bool,
}

impl BitchatRuntime {
    /// Create new BitChat runtime with custom configuration
    pub fn new(peer_id: PeerId, config: BitchatConfig) -> Self {
        let logger = if config
            .test
            .as_ref()
            .map(|t| t.enable_logging)
            .unwrap_or(false)
        {
            LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Debug))
        } else {
            LoggerWrapper::NoOp(NoOpLogger)
        };

        Self {
            config,
            peer_id,
            logger,
            pending_transports: Vec::new(),
            transport_handles: HashMap::new(),
            core_logic_handle: None,
            command_sender: None,
            app_event_receiver: None,
            running: false,
        }
    }

    /// Create runtime optimized for CLI usage
    pub fn for_cli(peer_id: PeerId, verbose: bool) -> Self {
        let mut config = BitchatConfig::default();

        // Optimize for interactive CLI usage
        config.channels.command_buffer_size = 50;
        config.channels.app_event_buffer_size = 200; // Larger for responsive UI
        config.monitoring = if verbose {
            bitchat_core::internal::MonitoringConfig::detailed()
        } else {
            bitchat_core::internal::MonitoringConfig::minimal()
        };

        let logger = if verbose {
            LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Debug).with_timestamps(false))
        } else {
            LoggerWrapper::Console(ConsoleLogger::new(LogLevel::Info).with_timestamps(false))
        };

        Self {
            config,
            peer_id,
            logger,
            pending_transports: Vec::new(),
            transport_handles: HashMap::new(),
            core_logic_handle: None,
            command_sender: None,
            app_event_receiver: None,
            running: false,
        }
    }

    /// Create runtime optimized for testing
    pub fn for_testing(peer_id: PeerId) -> Self {
        Self::new(peer_id, BitchatConfig::testing())
    }

    /// Add a transport task to the runtime
    ///
    /// Transport tasks must be added before calling `start()`. Each transport type
    /// can only be registered once to avoid conflicts.
    pub fn add_transport<T: TransportTask + 'static>(&mut self, transport: T) -> BitchatResult<()> {
        if self.running {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Cannot add transports to a running runtime".to_string(),
                },
            ));
        }

        let transport_type = transport.transport_type();

        // Check for duplicate transport types
        for existing in &self.pending_transports {
            if existing.transport_type() == transport_type {
                return Err(BitchatError::Transport(
                    TransportError::InvalidConfiguration {
                        reason: format!(
                            "Transport type {:?} is already registered",
                            transport_type
                        ),
                    },
                ));
            }
        }

        self.pending_transports.push(Box::new(transport));
        Ok(())
    }

    //     /// Special start method for convenience functions that adds a stub transport
    //     pub async fn start_with_stub_transport(&mut self) -> BitchatResult<()> {
    //         if self.running {
    //             return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
    //                 reason: "Runtime already running".to_string(),
    //             }));
    //         }
    //
    //         // Validate configuration
    //         self.config.validate().map_err(|e| BitchatError::Configuration { reason: e })?;
    //
    //         // Create channels
    //         let (command_sender, command_receiver) = create_command_channel(&self.config.channels);
    //         let (event_sender, event_receiver) = create_event_channel(&self.config.channels);
    //         let (effect_sender, effect_receiver) = create_effect_channel(&self.config.channels);
    //         let (app_event_sender, app_event_receiver) = create_app_event_channel(&self.config.channels);
    //
    //         // Store channels for external access
    //         self.command_sender = Some(command_sender);
    //         self.app_event_receiver = Some(app_event_receiver);
    //
    //         // Create stub transport with proper channels
    //         let stub_transport = crate::testing::StubTransportTask::new(
    //             event_sender.clone(),
    //             effect_receiver,
    //             self.logger.clone(),
    //         );
    //         let transport_type = stub_transport.transport_type();
    //
    //         // Start Core Logic task
    //         let mut core_logic = CoreLogicTask::new(
    //             self.peer_id,
    //             command_receiver,
    //             event_receiver,
    //             effect_sender,
    //             app_event_sender,
    //             self.logger.clone(),
    //             self.config.session.clone(),
    //             self.config.delivery.clone(),
    //             self.config.rate_limiting.clone(),
    //         )?;
    //
    //         let core_handle = tokio::spawn(async move {
    //             core_logic.run().await
    //         });
    //         self.core_logic_handle = Some(core_handle);
    //
    //         // Start the stub transport
    //         let (_, _transport_effect_receiver) = create_effect_channel(&self.config.channels);
    //         let handle = self.start_transport_task(Box::new(stub_transport), event_sender, _transport_effect_receiver).await?;
    //         self.transport_handles.insert(transport_type, handle);
    //
    //         self.running = true;
    //
    //         if let LoggerWrapper::Console(ref logger) = self.logger {
    //             logger.log_task_event(
    //                 bitchat_core::internal::TaskId::CoreLogic,
    //                 LogLevel::Info,
    //                 &format!("BitChat application started for peer {} with stub transport", self.peer_id)
    //             );
    //         }
    //
    //         Ok(())
    //     }

    /// Start the BitChat application
    pub async fn start(&mut self) -> BitchatResult<()> {
        if self.running {
            return Err(BitchatError::Transport(
                TransportError::InvalidConfiguration {
                    reason: "Runtime already running".to_string(),
                },
            ));
        }

        if self.pending_transports.is_empty() {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "No transport tasks registered. Use add_transport() to register at least one transport.".to_string(),
            }));
        }

        // Validate configuration
        self.config
            .validate()
            .map_err(|e| BitchatError::Configuration { reason: e })?;

        // Create channels following dependency injection pattern
        let (command_sender, command_receiver) = create_command_channel(&self.config.channels);
        let (event_sender, event_receiver) = create_event_channel(&self.config.channels);
        let (effect_sender, _initial_effect_receiver) =
            create_effect_channel(&self.config.channels);
        let (app_event_sender, app_event_receiver) =
            create_app_event_channel(&self.config.channels);

        // Clone effect_sender for creating transport subscriptions
        let effect_sender_for_transports = effect_sender.clone();

        // Store channels for external access
        self.command_sender = Some(command_sender);
        self.app_event_receiver = Some(app_event_receiver);

        // Start Core Logic task
        let mut core_logic = CoreLogicTask::new(
            self.peer_id,
            command_receiver,
            event_receiver,
            effect_sender,
            app_event_sender,
            self.logger.clone(),
            self.config.session.clone(),
            self.config.delivery.clone(),
            self.config.rate_limiting.clone(),
        )?;

        let core_handle = tokio::spawn(async move { core_logic.run().await });
        self.core_logic_handle = Some(core_handle);

        // Collect transport info before the loop to avoid borrow checker issues
        let transport_info: Vec<_> = self
            .pending_transports
            .iter()
            .map(|t| t.transport_type())
            .collect();
        let transports = self.pending_transports.drain(..).collect::<Vec<_>>();

        // Start all registered transport tasks
        for (i, transport) in transports.into_iter().enumerate() {
            let transport_type = transport_info[i];
            // Each transport gets its own subscription to the broadcast effect channel
            let transport_effect_receiver = create_effect_receiver(&effect_sender_for_transports);
            let handle = self
                .start_transport_task(transport, event_sender.clone(), transport_effect_receiver)
                .await?;
            self.transport_handles.insert(transport_type, handle);
        }

        self.running = true;

        if let LoggerWrapper::Console(ref logger) = self.logger {
            logger.log_task_event(
                bitchat_core::internal::TaskId::CoreLogic,
                LogLevel::Info,
                &format!(
                    "BitChat application started for peer {} with {} transport(s)",
                    self.peer_id,
                    self.transport_handles.len()
                ),
            );
        }

        Ok(())
    }

    /// Stop the BitChat application
    pub async fn stop(&mut self) -> BitchatResult<()> {
        if !self.running {
            return Ok(());
        }

        self.running = false;

        // Stop all transport tasks
        for (transport_type, handle) in self.transport_handles.drain() {
            if let LoggerWrapper::Console(ref logger) = self.logger {
                logger.log_task_event(
                    bitchat_core::internal::TaskId::Transport(transport_type),
                    LogLevel::Debug,
                    &format!("Stopping {:?} transport task", transport_type),
                );
            }
            handle.abort();
        }

        // Stop core logic task
        if let Some(handle) = self.core_logic_handle.take() {
            handle.abort();
        }

        // Clear channels
        self.command_sender = None;
        self.app_event_receiver = None;

        if let LoggerWrapper::Console(ref logger) = self.logger {
            logger.log_task_event(
                bitchat_core::internal::TaskId::CoreLogic,
                LogLevel::Info,
                &format!("BitChat application stopped for peer {}", self.peer_id),
            );
        }

        Ok(())
    }

    /// Get command sender for external use
    pub fn command_sender(&self) -> Option<&CommandSender> {
        self.command_sender.as_ref()
    }

    /// Take app event receiver for external use
    pub fn take_app_event_receiver(&mut self) -> Option<AppEventReceiver> {
        self.app_event_receiver.take()
    }

    /// Check if application is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Get configuration
    pub fn config(&self) -> &BitchatConfig {
        &self.config
    }

    /// Get list of registered transport types
    pub fn transport_types(&self) -> Vec<ChannelTransportType> {
        if self.running {
            self.transport_handles.keys().cloned().collect()
        } else {
            self.pending_transports
                .iter()
                .map(|t| t.transport_type())
                .collect()
        }
    }

    /// Check if a specific transport type is registered
    pub fn has_transport(&self, transport_type: ChannelTransportType) -> bool {
        if self.running {
            self.transport_handles.contains_key(&transport_type)
        } else {
            self.pending_transports
                .iter()
                .any(|t| t.transport_type() == transport_type)
        }
    }

    /// Start a single transport task
    async fn start_transport_task(
        &self,
        mut transport: Box<dyn TransportTask>,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<JoinHandle<BitchatResult<()>>> {
        let transport_type = transport.transport_type();

        transport.attach_channels(event_sender, effect_receiver)?;

        if let LoggerWrapper::Console(ref logger) = self.logger {
            logger.log_task_event(
                TaskId::Transport(transport_type),
                LogLevel::Debug,
                &format!("Starting {:?} transport task", transport_type),
            );
        }

        let handle = tokio::spawn(async move {
            // Simply run the transport's main event loop
            // The transport is responsible for its own lifecycle management
            transport.run().await
        });

        Ok(handle)
    }
}

impl Drop for BitchatRuntime {
    fn drop(&mut self) {
        if self.running {
            // Abort tasks if runtime is dropped while running
            for handle in self.transport_handles.values() {
                handle.abort();
            }
            if let Some(ref handle) = self.core_logic_handle {
                handle.abort();
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Convenience Functions
// ----------------------------------------------------------------------------

// /// Create and start a BitChat runtime for CLI usage with stub transport
// ///
// /// Note: In a real CLI application, you would create a runtime and register
// /// appropriate transport tasks (BleTransportTask, NostrTransportTask) from their respective crates.
// pub async fn start_cli_runtime(peer_id: PeerId, verbose: bool) -> BitchatResult<BitchatRuntime> {
//     let mut runtime = BitchatRuntime::for_cli(peer_id, verbose);
//
//     // For convenience functions, we'll use a special approach that adds the transport after start
//     runtime.start_with_stub_transport().await?;
//     Ok(runtime)
// }
//
// // /// Create and start a BitChat runtime for testing with stub transport
// // pub async fn start_test_runtime(peer_id: PeerId) -> BitchatResult<BitchatRuntime> {
// //     let mut runtime = BitchatRuntime::for_testing(peer_id);
// //
// //     // For convenience functions, we'll use a special approach that adds the transport after start
// //     runtime.start_with_stub_transport().await?;
// //     Ok(runtime)
// // }

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[tokio::test]
//     async fn test_runtime_lifecycle() {
//         let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//         let mut runtime = BitchatRuntime::for_testing(peer_id);
//
//         assert!(!runtime.is_running());
//
//         // Use the proper convenience method that handles channel setup
//         runtime.start_with_stub_transport().await.unwrap();
//         assert!(runtime.is_running());
//         assert!(runtime.command_sender().is_some());
//
//         runtime.stop().await.unwrap();
//         assert!(!runtime.is_running());
//     }

//     #[tokio::test]
//     async fn test_cli_runtime_creation() {
//         let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//         let runtime = BitchatRuntime::for_cli(peer_id, true);
//
//         assert_eq!(runtime.peer_id(), peer_id);
//         assert_eq!(runtime.config().channels.app_event_buffer_size, 200);
//     }
//
// #[tokio::test]
// async fn test_transport_registration() {
//     let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//     let runtime = BitchatRuntime::for_testing(peer_id);

//     assert_eq!(runtime.transport_types().len(), 0);

//     // This test doesn't actually start the runtime, so we can't test transport registration
//     // because our new design requires proper channel setup during start().
//     // We'll test this through other integration tests instead.
// }

// #[tokio::test]
// async fn test_start_cli_runtime() {
//     let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//     let mut runtime = start_cli_runtime(peer_id, false).await.unwrap();

//     assert!(runtime.is_running());
//     runtime.stop().await.unwrap();
// }
// }
