//! BitChat Simulator Shared Library
//!
//! Common abstractions used by both scenario-runner (simulation) and emulator-rig (real-world E2E).
//! 
//! This crate enforces architectural boundaries by providing:
//! - Universal client abstraction (any client type)
//! - Scenario executor trait (unified execution interface)
//! - Common types and utilities

pub mod client_bridge;
pub mod scenario_executor;
pub mod types;

// Re-export key types for convenience
pub use client_bridge::{
    UniversalClientType, UniversalClientBridge, UniversalClient, ClientResponse,
    CliClientAdapter, IosClientAdapter, ClientPair, TestingFramework, TestingStrategy,
    ClientTypeBridgeError, UnifiedClientType, // Backwards compatibility alias
};

pub use scenario_executor::{
    ScenarioExecutor, TestReport, ValidationResult, PerformanceMetrics, ExecutorError,
    ActionResult, ActionResultType, TestResult, ExecutorData, ExecutorState,
    ScenarioConfig, ScenarioMetadata, PeerConfig as SharedPeerConfig, TestStep, ValidationConfig, StateValidation,
    action_to_string, validation_to_string,
};

pub use types::{
    Action, TestAction, ValidationCheck, NetworkCondition, PeerConfig,
};

/// Version of the shared library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

