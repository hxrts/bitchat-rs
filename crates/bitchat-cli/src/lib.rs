//! BitChat CLI library
//!
//! This library provides the core components for the BitChat command-line interface,
//! including the TUI, message handling, and state management.

pub mod app;
pub mod cli;
pub mod config;
pub mod tui;
pub mod state;
pub mod commands;
pub mod error;

pub use app::BitchatApp;
pub use cli::{Cli, Commands};
pub use config::AppConfig;
pub use error::{CliError, Result};

// Re-export commonly used types
pub use bitchat_core::{
    BitchatMessage, MessageBuilder, MessageFragmenter, MessageReassembler,
    PeerId, StdDeliveryTracker, StdNoiseSessionManager, StdTimeSource,
    transport::{TransportManager, TransportSelectionPolicy, TransportType},
};