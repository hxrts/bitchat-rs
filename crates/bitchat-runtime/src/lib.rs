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
mod runtime;
pub mod rate_limiter;

pub use runtime::*;
pub use managers::*;

// Re-export core types for convenience
pub use bitchat_core::{
    BitchatError, BitchatResult, Command, AppEvent, Event, Effect,
    PeerId, TransportTask, EventSender, EffectReceiver
};