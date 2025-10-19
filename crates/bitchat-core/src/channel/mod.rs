//! Channel Module
//!
//! This module contains the CSP (Communicating Sequential Processes) channel infrastructure:
//! - `communication`: Core channel types, commands, events, and effects
//! - `utils`: Channel utilities, wrappers, and implementations

pub mod communication;
pub mod utils;

// Re-export communication types
pub use communication::{
    Command, Event, Effect, AppEvent, 
    TransportType as ChannelTransportType, 
    ConnectionStatus, TransportStatus
};

// Re-export ChannelConfig from config module
pub use crate::config::ChannelConfig;

// Re-export utility types
pub use utils::{
    ChannelError, NonBlockingSend, ChannelStats, TaskSpawner,
    CommandSender, CommandReceiver, EventSender, EventReceiver,
    EffectSender, EffectReceiver, AppEventSender, AppEventReceiver,
    create_command_channel, create_event_channel, create_effect_channel, create_effect_receiver, create_app_event_channel
};