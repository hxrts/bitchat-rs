//! BitChat Emulator Harness Library
//!
//! Provides Android and iOS emulator testing capabilities for BitChat protocol development.

use clap::ValueEnum;

#[derive(Debug, Clone, ValueEnum)]
pub enum ClientType {
    /// iOS device/simulator
    Ios,
    /// Android device/emulator
    Android,
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Ios => write!(f, "iOS"),
            ClientType::Android => write!(f, "Android"),
        }
    }
}

pub mod config;
pub mod orchestrator;
pub mod emulator;
pub mod network;
pub mod appium;
pub mod realworld_executor;

// Re-export main types for easier use
pub use config::TestConfig;
pub use orchestrator::{EmulatorOrchestrator, TestSession, Platform};
pub use emulator::{AndroidEmulator, IosSimulator};
pub use network::NetworkProxy;
pub use appium::AppiumController;
pub use realworld_executor::RealWorldExecutor;