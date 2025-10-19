//! Core Logic State Management
//!
//! Contains the core application state, statistics, and logger wrapper.

use bitchat_core::{
    PeerId,
    Command, Event, Effect, AppEvent,
    BitchatResult,
    internal::{
        ConnectionState, MessageStore, TimeSource, Timestamp,
        TaskLogger, TaskId, LogLevel, ConsoleLogger, NoOpLogger,
        SessionConfig, DeliveryConfig, AuditEntry
    }
};
use crate::managers::{NoiseSessionManager, DeliveryTracker, SessionTimeouts};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ----------------------------------------------------------------------------
// Core Logic State
// ----------------------------------------------------------------------------

/// Core application state owned by the Core Logic task
pub struct CoreState {
    /// Our peer identity
    pub peer_id: PeerId,
    /// Session manager for handling cryptographic sessions
    pub session_manager: NoiseSessionManager<SystemTimeSource>,
    /// Delivery tracker for message reliability
    pub delivery_tracker: DeliveryTracker<SystemTimeSource>,
    /// Content-addressed message storage
    pub message_store: MessageStore,
    /// Connection states for each peer
    pub connections: HashMap<PeerId, ConnectionState>,
    /// Audit trail for state transitions
    pub audit_trail: Vec<AuditEntry>,
    /// Sequence counter for message ordering
    pub message_sequence: u64,
    /// Task start time
    pub start_time: Timestamp,
    /// Statistics
    pub stats: CoreStats,
}

impl CoreState {
    /// Create new core state with given peer ID and configurations
    pub fn new(
        peer_id: PeerId, 
        session_config: SessionConfig,
        delivery_config: DeliveryConfig,
    ) -> BitchatResult<Self> {
        let time_source = SystemTimeSource;
        
        // Generate or load cryptographic keys (simplified for now)
        let noise_key = bitchat_core::internal::NoiseKeyPair::generate();
        let timeouts = SessionTimeouts {
            handshake_timeout: session_config.handshake_timeout,
            idle_timeout: session_config.idle_timeout,
        };
        let session_manager = NoiseSessionManager::new(noise_key, time_source, timeouts);
        let delivery_tracker = DeliveryTracker::with_config(delivery_config, SystemTimeSource);
        
        Ok(Self {
            peer_id,
            session_manager,
            delivery_tracker,
            message_store: MessageStore::new(),
            connections: HashMap::new(),
            audit_trail: Vec::new(),
            message_sequence: 0,
            start_time: SystemTimeSource.now(),
            stats: CoreStats::default(),
        })
    }
}

/// Logger wrapper for object safety
#[derive(Debug, Clone)]
pub enum LoggerWrapper {
    Console(ConsoleLogger),
    NoOp(NoOpLogger),
}

impl LoggerWrapper {
    pub fn log_receive_command(&self, from: TaskId, to: TaskId, message: &Command, channel_utilization: Option<f32>) {
        match self {
            LoggerWrapper::Console(logger) => logger.log_receive(from, to, message, channel_utilization),
            LoggerWrapper::NoOp(logger) => logger.log_receive(from, to, message, channel_utilization),
        }
    }

    pub fn log_receive_event(&self, from: TaskId, to: TaskId, message: &Event, channel_utilization: Option<f32>) {
        match self {
            LoggerWrapper::Console(logger) => logger.log_receive(from, to, message, channel_utilization),
            LoggerWrapper::NoOp(logger) => logger.log_receive(from, to, message, channel_utilization),
        }
    }

    pub fn log_send_effect(&self, from: TaskId, to: TaskId, message: &Effect, channel_utilization: Option<f32>) {
        match self {
            LoggerWrapper::Console(logger) => logger.log_send(from, to, message, channel_utilization),
            LoggerWrapper::NoOp(logger) => logger.log_send(from, to, message, channel_utilization),
        }
    }

    pub fn log_send_app_event(&self, from: TaskId, to: TaskId, message: &AppEvent, channel_utilization: Option<f32>) {
        match self {
            LoggerWrapper::Console(logger) => logger.log_send(from, to, message, channel_utilization),
            LoggerWrapper::NoOp(logger) => logger.log_send(from, to, message, channel_utilization),
        }
    }

    pub fn log_task_event(&self, task_id: TaskId, level: LogLevel, message: &str) {
        match self {
            LoggerWrapper::Console(logger) => logger.log_task_event(task_id, level, message),
            LoggerWrapper::NoOp(logger) => logger.log_task_event(task_id, level, message),
        }
    }
}

/// Statistics for the Core Logic task
#[derive(Debug, Clone, Default)]
pub struct CoreStats {
    pub commands_processed: u64,
    pub events_processed: u64,
    pub effects_generated: u64,
    pub app_events_generated: u64,
    pub state_transitions: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// System time source implementation
#[derive(Debug, Clone, Copy)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        Timestamp::new(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
    }
}