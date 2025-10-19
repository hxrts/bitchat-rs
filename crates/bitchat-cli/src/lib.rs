//! BitChat CLI Library
//!
//! Provides terminal user interface components and application orchestration for BitChat CLI.

pub mod config;
pub mod terminal_interface;
pub mod app_orchestrator;

pub use config::{CliAppConfig, CliConfig, IdentityConfig, RuntimeConfig, ConfigError};
pub use terminal_interface::{
    TerminalInterfaceTask, UIState, PeerUIState, UIMessage, 
    MessageDirection, SystemStatus
};
pub use app_orchestrator::{
    CliAppOrchestrator, TransportConfig,
    start_cli_application, start_cli_application_with_config, start_cli_application_with_transports
};