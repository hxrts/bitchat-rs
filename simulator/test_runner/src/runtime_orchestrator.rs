//! In-memory BitChat runtime orchestrator for comprehensive integration testing
//!
//! Replaces process-based testing with direct BitchatRuntime instantiation
//! for faster, more reliable, and more comprehensive validation.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};
use anyhow::{Context, Result};

use bitchat_core::{
    PeerId, Command, AppEvent, ChannelTransportType, ConnectionStatus,
    internal::{BitchatConfig, TestConfig, AppEventReceiver, CommandSender},
};
use bitchat_runtime::BitchatRuntime;
use bitchat_ble::BleTransportTask;
use bitchat_nostr::{NostrTransportTask, NostrConfig};

/// In-memory runtime orchestrator for comprehensive integration testing
pub struct RuntimeOrchestrator {
    runtimes: HashMap<String, RuntimeInstance>,
    event_timeout: Duration,
    relay_url: String,
}

/// Instance of a BitchatRuntime with associated channels
struct RuntimeInstance {
    name: String,
    peer_id: PeerId,
    runtime: BitchatRuntime,
    command_sender: CommandSender,
    app_event_receiver: AppEventReceiver,
    enabled_transports: Vec<ChannelTransportType>,
}

/// Structured event from runtime instance
#[derive(Debug, Clone)]
pub struct RuntimeEvent {
    pub client_name: String,
    pub event: AppEvent,
    pub timestamp: u64,
}

impl RuntimeOrchestrator {
    /// Create new runtime orchestrator
    pub fn new(relay_url: String) -> Self {
        Self {
            runtimes: HashMap::new(),
            event_timeout: Duration::from_secs(5), // Shorter timeout for testing
            relay_url,
        }
    }

    /// Start a BitChat runtime instance with BLE and Nostr transports
    pub async fn start_runtime(&mut self, name: String) -> Result<()> {
        self.start_runtime_with_transports(name, vec![ChannelTransportType::Ble, ChannelTransportType::Nostr]).await
    }

    /// Start a BitChat runtime instance with specific transports
    pub async fn start_runtime_with_transports(
        &mut self, 
        name: String, 
        transports: Vec<ChannelTransportType>
    ) -> Result<()> {
        info!("Starting runtime instance '{}'", name);

        // Generate deterministic peer ID from name for consistency
        let peer_id = self.generate_peer_id_from_name(&name);

        // Create test-optimized configuration
        let mut config = BitchatConfig::testing();
        config.test = Some(TestConfig::new().with_peer_id(peer_id).with_transports(transports.clone()));

        // Create runtime instance
        let mut runtime = BitchatRuntime::new(peer_id, config);

        // Create mock transports for testing
        // Note: The actual channel setup happens in BitchatRuntime.start()
        // These are just placeholder transports for the runtime to manage
        for transport_type in &transports {
            match transport_type {
                ChannelTransportType::Ble => {
                    let ble_transport = BleTransportTask::new();
                    runtime.add_transport(ble_transport)
                        .with_context(|| format!("Failed to add BLE transport to runtime '{}'", name))?;
                }
                ChannelTransportType::Nostr => {
                    let nostr_config = NostrConfig::default_with_relay(&self.relay_url);
                    let nostr_transport = NostrTransportTask::new(nostr_config)
                        .with_context(|| format!("Failed to create Nostr transport for runtime '{}'", name))?;
                    runtime.add_transport(nostr_transport)
                        .with_context(|| format!("Failed to add Nostr transport to runtime '{}'", name))?;
                }
            }
        }

        // Start the runtime
        runtime.start().await
            .with_context(|| format!("Failed to start runtime '{}'", name))?;

        // Extract channels for testing
        let command_sender = runtime.command_sender()
            .ok_or_else(|| anyhow::anyhow!("No command sender available for runtime '{}'", name))?
            .clone();
        
        let app_event_receiver = runtime.take_app_event_receiver()
            .ok_or_else(|| anyhow::anyhow!("No app event receiver available for runtime '{}'", name))?;

        let instance = RuntimeInstance {
            name: name.clone(),
            peer_id,
            runtime,
            command_sender,
            app_event_receiver,
            enabled_transports: transports,
        };

        self.runtimes.insert(name.clone(), instance);
        info!("Runtime instance '{}' started with peer ID {}", name, peer_id);
        Ok(())
    }

    /// Send command to runtime instance
    pub async fn send_command(&mut self, runtime_name: &str, command: Command) -> Result<()> {
        let instance = self.runtimes.get_mut(runtime_name)
            .with_context(|| format!("Runtime '{}' not found", runtime_name))?;

        instance.command_sender.send(command).await
            .with_context(|| format!("Failed to send command to runtime '{}'", runtime_name))?;

        debug!("Sent command to runtime '{}'", runtime_name);
        Ok(())
    }

    /// Wait for a specific AppEvent from a runtime instance
    pub async fn wait_for_event(&mut self, runtime_name: &str, event_predicate: impl Fn(&AppEvent) -> bool) -> Result<RuntimeEvent> {
        let start = Instant::now();
        let instance = self.runtimes.get_mut(runtime_name)
            .with_context(|| format!("Runtime '{}' not found", runtime_name))?;

        loop {
            match timeout(self.event_timeout, instance.app_event_receiver.recv()).await {
                Ok(Some(app_event)) => {
                    debug!("Received event from runtime '{}': {:?}", runtime_name, app_event);
                    if event_predicate(&app_event) {
                        let runtime_event = RuntimeEvent {
                            client_name: runtime_name.to_string(),
                            event: app_event,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        };
                        
                        info!("Received expected event from runtime '{}' in {:?}", 
                              runtime_name, start.elapsed());
                        return Ok(runtime_event);
                    } else {
                        debug!("Ignoring event from runtime '{}' (doesn't match predicate): {:?}", runtime_name, app_event);
                    }
                }
                Ok(None) => {
                    return Err(anyhow::anyhow!("Runtime '{}' app event channel closed", runtime_name));
                }
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Timeout waiting for event from runtime '{}' after {:?}",
                        runtime_name, self.event_timeout
                    ));
                }
            }
        }
    }

    /// Wait for peer discovery event
    pub async fn wait_for_peer_discovered(&mut self, runtime_name: &str, target_peer_id: PeerId) -> Result<RuntimeEvent> {
        self.wait_for_event(runtime_name, |event| {
            match event {
                AppEvent::PeerStatusChanged { peer_id, status, .. } => {
                    *peer_id == target_peer_id && matches!(status, ConnectionStatus::Connecting)
                }
                _ => false,
            }
        }).await
    }

    /// Wait for session establishment event
    pub async fn wait_for_session_established(&mut self, runtime_name: &str, target_peer_id: PeerId) -> Result<RuntimeEvent> {
        self.wait_for_event(runtime_name, |event| {
            match event {
                AppEvent::PeerStatusChanged { peer_id, status, .. } => {
                    *peer_id == target_peer_id && matches!(status, ConnectionStatus::Connected)
                }
                _ => false,
            }
        }).await
    }

    /// Wait for message received event with specific content
    pub async fn wait_for_message_received(&mut self, runtime_name: &str, expected_content: &str) -> Result<RuntimeEvent> {
        let expected_content = expected_content.to_string();
        self.wait_for_event(runtime_name, move |event| {
            match event {
                AppEvent::MessageReceived { content, .. } => {
                    content == &expected_content
                }
                _ => false,
            }
        }).await
    }

    /// Wait for message sent event
    pub async fn wait_for_message_sent(&mut self, runtime_name: &str) -> Result<RuntimeEvent> {
        self.wait_for_event(runtime_name, |event| {
            matches!(event, AppEvent::MessageSent { .. })
        }).await
    }

    /// Wait for all runtimes to be ready (skip this for now since we don't have clear ready events)
    pub async fn wait_for_all_ready(&mut self) -> Result<()> {
        let runtime_names: Vec<String> = self.runtimes.keys().cloned().collect();
        
        for runtime_name in runtime_names {
            // For now, just verify the runtime is properly initialized
            // TODO: Once we have clear "ready" semantics, we can wait for actual events
            info!("Runtime '{}' is assumed ready", runtime_name);
        }
        
        // Small delay to allow runtime initialization to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok(())
    }

    /// Stop a specific runtime instance
    pub async fn stop_runtime(&mut self, runtime_name: &str) -> Result<()> {
        if let Some(mut instance) = self.runtimes.remove(runtime_name) {
            instance.runtime.stop().await
                .with_context(|| format!("Failed to stop runtime '{}'", runtime_name))?;
            info!("Stopped runtime '{}'", runtime_name);
        }
        Ok(())
    }

    /// Stop all runtime instances
    pub async fn stop_all_runtimes(&mut self) -> Result<()> {
        let runtime_names: Vec<String> = self.runtimes.keys().cloned().collect();
        
        for runtime_name in runtime_names {
            if let Err(e) = self.stop_runtime(&runtime_name).await {
                warn!("Failed to stop runtime '{}': {}", runtime_name, e);
            }
        }
        
        Ok(())
    }

    /// Get list of running runtime instances
    pub fn running_runtimes(&self) -> Vec<String> {
        self.runtimes.keys().cloned().collect()
    }

    /// Get runtime instance information
    pub fn get_runtime_info(&self, runtime_name: &str) -> Option<(PeerId, &[ChannelTransportType])> {
        self.runtimes.get(runtime_name).map(|instance| {
            (instance.peer_id, instance.enabled_transports.as_slice())
        })
    }

    /// Set event timeout
    pub fn set_event_timeout(&mut self, timeout: Duration) {
        self.event_timeout = timeout;
    }

    /// Get relay URL
    pub fn relay_url(&self) -> &str {
        &self.relay_url
    }

    /// Generate deterministic peer ID from name
    fn generate_peer_id_from_name(&self, name: &str) -> PeerId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let hash = hasher.finish();
        
        let peer_bytes = [
            (hash >> 56) as u8,
            (hash >> 48) as u8,
            (hash >> 40) as u8,
            (hash >> 32) as u8,
            (hash >> 24) as u8,
            (hash >> 16) as u8,
            (hash >> 8) as u8,
            hash as u8,
        ];
        
        PeerId::new(peer_bytes)
    }

    /// Validate comprehensive runtime state
    pub async fn validate_runtime_state(&self, runtime_name: &str) -> Result<RuntimeValidation> {
        let instance = self.runtimes.get(runtime_name)
            .with_context(|| format!("Runtime '{}' not found", runtime_name))?;

        Ok(RuntimeValidation {
            runtime_name: runtime_name.to_string(),
            peer_id: instance.peer_id,
            is_running: instance.runtime.is_running(),
            enabled_transports: instance.enabled_transports.clone(),
            transport_count: instance.enabled_transports.len(),
        })
    }
}

/// Runtime validation results
#[derive(Debug)]
pub struct RuntimeValidation {
    pub runtime_name: String,
    pub peer_id: PeerId,
    pub is_running: bool,
    pub enabled_transports: Vec<ChannelTransportType>,
    pub transport_count: usize,
}

impl Drop for RuntimeOrchestrator {
    fn drop(&mut self) {
        // Ensure all runtimes are stopped when orchestrator is dropped
        for (name, instance) in self.runtimes.drain() {
            if instance.runtime.is_running() {
                warn!("Force stopping runtime '{}' in orchestrator drop", name);
                // Can't await in Drop, so we'll just abort the runtime
                // The runtime's Drop implementation should handle cleanup
            }
        }
    }
}
