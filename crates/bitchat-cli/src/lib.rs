//! BitChat CLI library
//!
//! This library provides the core components for the BitChat command-line interface,
//! including the TUI, message handling, and state management.

pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod state;
#[cfg(feature = "tui")]
pub mod tui;

pub use app::BitchatApp;
pub use cli::{Cli, Commands};
pub use config::AppConfig;
pub use error::{CliError, Result};

// Re-export commonly used types
pub use bitchat_core::{
    transport::{TransportManager, TransportSelectionPolicy, TransportType},
    BitchatMessage, MessageBuilder, MessageFragmenter, MessageReassembler, PeerId,
    StdDeliveryTracker, StdNoiseSessionManager, StdTimeSource,
};
