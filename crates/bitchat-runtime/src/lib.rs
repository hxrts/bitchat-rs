//! BitChat Runtime Engine
//!
//! This crate contains the core runtime engine for the BitChat protocol, including:
//! - `BitchatRuntime`: The main orchestrator that manages transport tasks
//! - `CoreLogicTask`: The central state machine handling protocol logic
//! - Session and delivery managers
//! - Connection state management
//!
//! This is the "engine" of BitChat - it orchestrates the protocol logic while
//! `bitchat-core` provides the stable API definitions.

extern crate alloc;

pub mod logic;
pub mod managers;
pub mod rate_limiter;
mod runtime;

// New decomposed architecture modules
pub mod builder;
pub mod supervisor;
pub mod tasks;

pub use builder::{MonitoringConfig, RuntimeBuilder, RuntimeHandle};
pub use managers::*;
pub use runtime::*;
pub use supervisor::SupervisorTask;

// Re-export core types for convenience
pub use bitchat_core::{
    channel::utils::{
        create_app_event_channel, create_command_channel, create_effect_channel,
        create_effect_receiver, create_event_channel, AppEventReceiver, AppEventSender,
        ChannelError, ChannelStats, CommandReceiver, CommandSender, EffectReceiver, EffectSender,
        EventReceiver, EventSender, NonBlockingSend,
    },
    channel::{
        AppEvent, ChannelTransportType, Command, ConnectionStatus, Effect, Event, TransportStatus,
    },
    BitchatError, BitchatResult, PeerId, TransportTask,
};
