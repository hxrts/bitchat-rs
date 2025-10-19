//! Channel Module
//!
//! This module contains the CSP (Communicating Sequential Processes) channel infrastructure:
//! - `communication`: Core channel types, commands, events, and effects
//! - `utils`: Channel utilities, wrappers, and implementations

pub mod communication;
pub mod utils;

// Re-export communication types
pub use communication::{
    AppEvent, Command, ConnectionStatus, Effect, Event, TransportStatus,
    TransportType as ChannelTransportType,
};

// Re-export ChannelConfig from config module
pub use crate::config::ChannelConfig;

// Re-export utility types
pub use utils::{
    create_app_event_channel, create_command_channel, create_effect_channel,
    create_effect_receiver, create_event_channel, AppEventReceiver, AppEventSender, ChannelError,
    ChannelStats, CommandReceiver, CommandSender, EffectReceiver, EffectSender, EventReceiver,
    EventSender, NonBlockingSend, TaskSpawner,
};
