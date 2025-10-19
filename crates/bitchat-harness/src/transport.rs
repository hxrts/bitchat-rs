//! Transport harness utilities
//!
//! Provides shared transport infrastructure including channel wiring,
//! lifecycle hooks, and common utilities like reconnection and heartbeats.

use crate::messages::{
    ChannelTransportType, InboundMessage, OutboundMessage, RawInboundMessage, RawOutboundMessage,
};
use bitchat_core::{
    internal::TransportError, BitchatError, BitchatResult, EffectReceiver, EventSender,
};

#[cfg(not(feature = "std"))]
use core::time::Duration;
#[cfg(feature = "std")]
use std::time::Duration;

// ----------------------------------------------------------------------------
// Transport Handle
// ----------------------------------------------------------------------------

/// Handle to the channels provided to a transport task.
#[derive(Debug)]
pub struct TransportHandle {
    event_sender: EventSender,
    effect_receiver: Option<EffectReceiver>,
}

impl TransportHandle {
    pub fn new(event_sender: EventSender, effect_receiver: EffectReceiver) -> Self {
        Self {
            event_sender,
            effect_receiver: Some(effect_receiver),
        }
    }

    pub fn event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    pub fn take_effect_receiver(&mut self) -> Option<EffectReceiver> {
        self.effect_receiver.take()
    }
}

// ----------------------------------------------------------------------------
// Transport Lifecycle
// ----------------------------------------------------------------------------

/// Transport lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Initialized,
    Preparing,
    Running,
    Stopping,
    Stopped,
    Failed,
}

/// Transport lifecycle hooks
pub trait TransportLifecycle {
    /// Prepare the transport for operation (e.g., initialize connections)
    fn prepare(&mut self) -> impl core::future::Future<Output = BitchatResult<()>> + Send;

    /// Run the transport main loop
    fn run(&mut self) -> impl core::future::Future<Output = BitchatResult<()>> + Send;

    /// Shutdown the transport gracefully
    fn shutdown(&mut self) -> impl core::future::Future<Output = BitchatResult<()>> + Send;

    /// Get current transport state
    fn state(&self) -> TransportState;

    /// Get transport type identifier
    fn transport_type(&self) -> ChannelTransportType;
}

// ----------------------------------------------------------------------------
// Message Processing Utilities
// ----------------------------------------------------------------------------

/// Helper for processing raw transport messages
pub struct MessageProcessor {
    transport_type: ChannelTransportType,
    event_sender: EventSender,
}

impl MessageProcessor {
    pub fn new(transport_type: ChannelTransportType, event_sender: EventSender) -> Self {
        Self {
            transport_type,
            event_sender,
        }
    }

    /// Get the transport type for this processor
    pub fn transport_type(&self) -> ChannelTransportType {
        self.transport_type
    }

    /// Process a raw inbound message and forward it as an Event
    pub async fn process_inbound(&self, raw: RawInboundMessage) -> BitchatResult<()> {
        // Validate the raw message
        raw.validate()?;

        // Normalize to canonical Event
        let inbound = InboundMessage::from_transport(raw)?;

        // Send the event
        self.event_sender.try_send(inbound.event).map_err(|_| {
            BitchatError::Transport(TransportError::SendBufferFull {
                capacity: 0, // We don't have access to capacity here
            })
        })?;

        Ok(())
    }

    /// Convert an Effect to a raw outbound message
    pub fn prepare_outbound(
        &self,
        effect: crate::messages::Effect,
    ) -> BitchatResult<RawOutboundMessage> {
        let outbound = OutboundMessage::to_transport(effect)?;
        outbound.extract_raw()
    }
}

// ----------------------------------------------------------------------------
// Reconnection Utilities
// ----------------------------------------------------------------------------

/// Reconnection strategy configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initial delay before first reconnect attempt
    pub initial_delay: Duration,
    /// Maximum delay between attempts
    pub max_delay: Duration,
    /// Delay multiplier for exponential backoff
    pub backoff_multiplier: f32,
    /// Maximum number of reconnection attempts (None = unlimited)
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            max_attempts: None,
        }
    }
}

/// Reconnection manager for transports
#[derive(Debug)]
pub struct ReconnectManager {
    config: ReconnectConfig,
    current_delay: Duration,
    attempt_count: u32,
}

impl ReconnectManager {
    pub fn new(config: ReconnectConfig) -> Self {
        let current_delay = config.initial_delay;
        Self {
            config,
            current_delay,
            attempt_count: 0,
        }
    }

    /// Calculate next reconnection delay
    pub fn next_delay(&mut self) -> Option<Duration> {
        if let Some(max_attempts) = self.config.max_attempts {
            if self.attempt_count >= max_attempts {
                return None;
            }
        }

        let delay = self.current_delay;

        // Update for next attempt
        self.attempt_count += 1;
        let next_delay_millis =
            (self.current_delay.as_millis() as f32 * self.config.backoff_multiplier) as u64;
        self.current_delay =
            Duration::from_millis(next_delay_millis.min(self.config.max_delay.as_millis() as u64));

        Some(delay)
    }

    /// Reset the reconnection state (call on successful connection)
    pub fn reset(&mut self) {
        self.current_delay = self.config.initial_delay;
        self.attempt_count = 0;
    }

    /// Get current attempt count
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
}

// ----------------------------------------------------------------------------
// Heartbeat Utilities
// ----------------------------------------------------------------------------

/// Heartbeat configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeat messages
    pub interval: Duration,
    /// Timeout for heartbeat responses
    pub timeout: Duration,
    /// Maximum missed heartbeats before considering connection dead
    pub max_missed: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_missed: 3,
        }
    }
}

/// Heartbeat manager for connection health monitoring
#[derive(Debug)]
pub struct HeartbeatManager {
    config: HeartbeatConfig,
    missed_count: u32,
    last_heartbeat: Option<u64>, // Timestamp
}

impl HeartbeatManager {
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            missed_count: 0,
            last_heartbeat: None,
        }
    }

    /// Record a successful heartbeat
    pub fn record_heartbeat(&mut self, timestamp: u64) {
        self.last_heartbeat = Some(timestamp);
        self.missed_count = 0;
    }

    /// Record a missed heartbeat
    pub fn record_missed(&mut self) {
        self.missed_count += 1;
    }

    /// Check if connection should be considered dead
    pub fn is_connection_dead(&self) -> bool {
        self.missed_count >= self.config.max_missed
    }

    /// Get heartbeat interval
    pub fn interval(&self) -> Duration {
        self.config.interval
    }

    /// Get missed heartbeat count
    pub fn missed_count(&self) -> u32 {
        self.missed_count
    }
}

// ----------------------------------------------------------------------------
// Transport Builder
// ----------------------------------------------------------------------------

/// Builder for creating transport tasks with harness utilities
#[derive(Debug)]
pub struct TransportBuilder {
    transport_type: ChannelTransportType,
    reconnect_config: Option<ReconnectConfig>,
    heartbeat_config: Option<HeartbeatConfig>,
}

impl TransportBuilder {
    pub fn new(transport_type: ChannelTransportType) -> Self {
        Self {
            transport_type,
            reconnect_config: None,
            heartbeat_config: None,
        }
    }

    /// Configure reconnection behavior
    pub fn with_reconnect(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = Some(config);
        self
    }

    /// Configure heartbeat monitoring
    pub fn with_heartbeat(mut self, config: HeartbeatConfig) -> Self {
        self.heartbeat_config = Some(config);
        self
    }

    /// Build a message processor for this transport
    pub fn build_message_processor(&self, event_sender: EventSender) -> MessageProcessor {
        MessageProcessor::new(self.transport_type, event_sender)
    }

    /// Build a reconnect manager (if configured)
    pub fn build_reconnect_manager(&self) -> Option<ReconnectManager> {
        self.reconnect_config
            .as_ref()
            .map(|config| ReconnectManager::new(config.clone()))
    }

    /// Build a heartbeat manager (if configured)
    pub fn build_heartbeat_manager(&self) -> Option<HeartbeatManager> {
        self.heartbeat_config
            .as_ref()
            .map(|config| HeartbeatManager::new(config.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::internal::create_event_channel;

    #[test]
    fn test_reconnect_manager() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
            backoff_multiplier: 2.0,
            max_attempts: Some(3),
        };

        let mut manager = ReconnectManager::new(config);

        // First attempt
        assert_eq!(manager.next_delay(), Some(Duration::from_millis(100)));
        assert_eq!(manager.attempt_count(), 1);

        // Second attempt
        assert_eq!(manager.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(manager.attempt_count(), 2);

        // Third attempt
        assert_eq!(manager.next_delay(), Some(Duration::from_millis(400)));
        assert_eq!(manager.attempt_count(), 3);

        // Should be exhausted now
        assert_eq!(manager.next_delay(), None);
    }

    #[test]
    fn test_reconnect_manager_reset() {
        let config = ReconnectConfig::default();
        let mut manager = ReconnectManager::new(config);

        // Make some attempts
        manager.next_delay();
        manager.next_delay();
        assert_eq!(manager.attempt_count(), 2);

        // Reset and try again
        manager.reset();
        assert_eq!(manager.attempt_count(), 0);
        assert_eq!(manager.next_delay(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn test_heartbeat_manager() {
        let config = HeartbeatConfig {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_missed: 3,
        };

        let mut manager = HeartbeatManager::new(config);
        assert!(!manager.is_connection_dead());

        // Miss some heartbeats
        manager.record_missed();
        assert!(!manager.is_connection_dead());

        manager.record_missed();
        assert!(!manager.is_connection_dead());

        manager.record_missed();
        assert!(manager.is_connection_dead());

        // Recovery
        manager.record_heartbeat(123456);
        assert!(!manager.is_connection_dead());
        assert_eq!(manager.missed_count(), 0);
    }

    #[test]
    fn test_transport_builder() {
        let builder = TransportBuilder::new(ChannelTransportType::Ble)
            .with_reconnect(ReconnectConfig::default())
            .with_heartbeat(HeartbeatConfig::default());

        let config = bitchat_core::internal::ChannelConfig::default();
        let (_tx, _rx) = create_event_channel(&config);
        let _processor = builder.build_message_processor(_tx);
        let reconnect_mgr = builder.build_reconnect_manager();
        let heartbeat_mgr = builder.build_heartbeat_manager();

        assert!(reconnect_mgr.is_some());
        assert!(heartbeat_mgr.is_some());
    }
}
