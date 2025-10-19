//! BitChat CLI Library
//!
//! Provides terminal user interface components and application orchestration for BitChat CLI.

pub mod app_orchestrator;
pub mod config;
pub mod terminal_interface;

pub use app_orchestrator::{
    start_cli_application, start_cli_application_with_config,
    start_cli_application_with_transports, CliAppOrchestrator, TransportConfig,
};
pub use config::{CliAppConfig, CliConfig, ConfigError, IdentityConfig, RuntimeConfig};
pub use terminal_interface::{
    MessageDirection, PeerUIState, SystemStatus, TerminalInterfaceTask, UIMessage, UIState,
};
