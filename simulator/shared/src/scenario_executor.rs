//! Scenario Executor Trait Interface
//!
//! This module defines the unified interface for executing test scenarios
//! across both simulation and real-world environments.

use async_trait::async_trait;
use std::time::Duration;
use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};

// Re-export types from the scenario config (would be in scenario-runner crate)
pub use crate::types::{TestAction, ValidationCheck};

// ----------------------------------------------------------------------------
// Core Trait Definition
// ----------------------------------------------------------------------------

/// Unified interface for executing test scenarios
/// 
/// This trait is implemented by both SimulationExecutor (scenario-runner)
/// and RealWorldExecutor (emulator-rig) to provide a consistent interface
/// for running the same TOML scenarios in different environments.
#[async_trait]
pub trait ScenarioExecutor {
    /// Execute a complete scenario and return the test report
    async fn execute_scenario(&mut self, scenario: &ScenarioConfig) -> Result<TestReport, ExecutorError>;
    
    /// Execute a single test action
    async fn execute_action(&mut self, action: &TestAction) -> Result<(), ExecutorError>;
    
    /// Validate a set of checks against current state
    async fn validate_checks(&self, checks: &[ValidationCheck]) -> Result<Vec<ValidationResult>, ExecutorError>;
    
    /// Setup the executor with scenario configuration
    async fn setup(&mut self, scenario: &ScenarioConfig) -> Result<(), ExecutorError>;
    
    /// Cleanup resources after scenario execution
    async fn cleanup(&mut self) -> Result<(), ExecutorError>;
    
    /// Get current execution state for debugging
    fn get_state(&self) -> ExecutorState;
    
    /// Wait for a specific amount of time (implementation-dependent)
    async fn wait(&mut self, duration: Duration) -> Result<(), ExecutorError>;
    
    /// Check if executor is ready to run scenarios
    fn is_ready(&self) -> bool;
}

// ----------------------------------------------------------------------------
// Data Types
// ----------------------------------------------------------------------------

/// Simplified scenario config for shared use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub metadata: ScenarioMetadata,
    pub peers: Vec<PeerConfig>,
    pub sequence: Vec<TestStep>,
    pub validation: ValidationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub name: String,
    pub platform: Option<String>, // "ios", "android", "cli", "web"
    pub start_delay_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    pub name: String,
    pub at_time_seconds: f64,
    pub action: TestAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub final_checks: Vec<StateValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateValidation {
    pub check: ValidationCheck,
}

/// Test execution report containing results and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    /// Scenario metadata
    pub scenario_name: String,
    pub scenario_version: String,
    
    /// Execution timing
    pub start_time: std::time::SystemTime,
    pub end_time: std::time::SystemTime,
    pub duration: Duration,
    
    /// Overall result
    pub result: TestResult,
    
    /// Individual action results
    pub action_results: Vec<ActionResult>,
    
    /// Validation results
    pub validation_results: Vec<ValidationResult>,
    
    /// Performance metrics
    pub metrics: PerformanceMetrics,
    
    /// Error details if failed
    pub error_details: Option<String>,
    
    /// Executor-specific data
    pub executor_data: ExecutorData,
}

/// Overall test result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestResult {
    /// Test passed completely
    Passed,
    /// Test failed with specific reasons
    Failed(Vec<String>),
    /// Test was skipped (e.g., missing dependencies)
    Skipped(String),
    /// Test execution was aborted
    Aborted(String),
}

/// Result of executing a single action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Action that was executed
    pub action_name: String,
    pub action_type: String,
    
    /// Execution timing
    pub start_time: std::time::SystemTime,
    pub duration: Duration,
    
    /// Result
    pub result: ActionResultType,
    
    /// Optional error message
    pub error_message: Option<String>,
    
    /// Executor-specific action data
    pub action_data: HashMap<String, String>,
}

/// Type of action result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionResultType {
    Success,
    Failed,
    Skipped,
    Timeout,
}

/// Result of validation checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Type of validation performed
    pub validation_type: String,
    
    /// Whether validation passed
    pub passed: bool,
    
    /// Details about the validation
    pub details: String,
    
    /// Expected vs actual values (if applicable)
    pub expected: Option<String>,
    pub actual: Option<String>,
    
    /// Validation timing
    pub check_time: std::time::SystemTime,
}

/// Performance metrics collected during execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Number of messages sent
    pub messages_sent: u64,
    
    /// Number of messages received
    pub messages_received: u64,
    
    /// Average message latency
    pub avg_latency_ms: Option<f64>,
    
    /// Packet loss rate (0.0-1.0)
    pub packet_loss_rate: Option<f32>,
    
    /// Memory usage statistics
    pub memory_usage: MemoryUsage,
    
    /// CPU usage statistics
    pub cpu_usage: CpuUsage,
    
    /// Network statistics
    pub network_stats: NetworkStats,
}

/// Memory usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Peak memory usage in MB
    pub peak_mb: Option<f64>,
    
    /// Average memory usage in MB
    pub avg_mb: Option<f64>,
    
    /// Memory usage per peer
    pub per_peer_mb: HashMap<String, f64>,
}

/// CPU usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuUsage {
    /// Peak CPU usage percentage
    pub peak_percent: Option<f32>,
    
    /// Average CPU usage percentage
    pub avg_percent: Option<f32>,
    
    /// CPU usage per peer
    pub per_peer_percent: HashMap<String, f32>,
}

/// Network statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Total packets sent
    pub packets_sent: u64,
    
    /// Total packets received
    pub packets_received: u64,
    
    /// Total bytes sent
    pub bytes_sent: u64,
    
    /// Total bytes received
    pub bytes_received: u64,
    
    /// Connection statistics
    pub connections_established: u32,
    pub connections_failed: u32,
}

/// Executor-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutorData {
    /// Data from simulation executor
    Simulation {
        /// Number of simulated peers
        peer_count: usize,
        /// Simulation time advancement steps
        time_steps: u64,
        /// Internal state snapshots
        state_snapshots: Vec<String>,
    },
    /// Data from real-world executor
    RealWorld {
        /// Connected devices
        device_info: Vec<DeviceInfo>,
        /// Appium session details
        appium_sessions: Vec<String>,
        /// Real execution environment
        environment: String,
    },
}

/// Information about a real device/emulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device identifier
    pub device_id: String,
    /// Device type (iOS, Android, CLI, Web)
    pub device_type: String,
    /// Device version/OS
    pub version: String,
    /// Connection status
    pub connected: bool,
}

/// Current state of the executor
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutorState {
    /// Not initialized
    Uninitialized,
    /// Ready to execute scenarios
    Ready,
    /// Currently executing a scenario
    Executing,
    /// Scenario execution completed
    Completed,
    /// Error state - needs cleanup
    Error(String),
}

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors that can occur during scenario execution
#[derive(Debug, Error)]
pub enum ExecutorError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    /// Setup error
    #[error("Setup failed: {0}")]
    Setup(String),
    
    /// Action execution error
    #[error("Action '{action}' failed: {reason}")]
    ActionFailed { action: String, reason: String },
    
    /// Validation error
    #[error("Validation failed: {0}")]
    Validation(String),
    
    /// Timeout error
    #[error("Operation timed out after {timeout_seconds}s: {operation}")]
    Timeout { timeout_seconds: u64, operation: String },
    
    /// Resource error (memory, CPU, network)
    #[error("Resource error: {0}")]
    Resource(String),
    
    /// Cleanup error
    #[error("Cleanup failed: {0}")]
    Cleanup(String),
    
    /// Executor not ready
    #[error("Executor not ready: {state:?}")]
    NotReady { state: ExecutorState },
    
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
    
    /// External dependency error
    #[error("External dependency error: {0}")]
    ExternalDependency(String),
}

// ----------------------------------------------------------------------------
// Helper Implementations
// ----------------------------------------------------------------------------

impl TestReport {
    /// Create a new test report
    pub fn new(scenario_name: String, scenario_version: String) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            scenario_name,
            scenario_version,
            start_time: now,
            end_time: now,
            duration: Duration::from_secs(0),
            result: TestResult::Passed,
            action_results: Vec::new(),
            validation_results: Vec::new(),
            metrics: PerformanceMetrics::default(),
            error_details: None,
            executor_data: ExecutorData::Simulation {
                peer_count: 0,
                time_steps: 0,
                state_snapshots: Vec::new(),
            },
        }
    }
    
    /// Mark the test as completed
    pub fn complete(&mut self) {
        self.end_time = std::time::SystemTime::now();
        self.duration = self.end_time.duration_since(self.start_time).unwrap_or(Duration::from_secs(0));
    }
    
    /// Add an action result
    pub fn add_action_result(&mut self, result: ActionResult) {
        self.action_results.push(result);
    }
    
    /// Add a validation result
    pub fn add_validation_result(&mut self, result: ValidationResult) {
        // Update overall test result based on validation
        if !result.passed {
            match &mut self.result {
                TestResult::Passed => {
                    self.result = TestResult::Failed(vec![result.details.clone()]);
                }
                TestResult::Failed(ref mut reasons) => {
                    reasons.push(result.details.clone());
                }
                _ => {} // Don't change if already failed/skipped/aborted
            }
        }
        self.validation_results.push(result);
    }
    
    /// Check if the test passed
    pub fn is_success(&self) -> bool {
        matches!(self.result, TestResult::Passed)
    }
    
    /// Get summary string
    pub fn summary(&self) -> String {
        match &self.result {
            TestResult::Passed => format!(
                "PASS {} ({:.2}s, {} actions, {} validations)",
                self.scenario_name,
                self.duration.as_secs_f64(),
                self.action_results.len(),
                self.validation_results.len()
            ),
            TestResult::Failed(reasons) => format!(
                "FAIL {} ({:.2}s): {}",
                self.scenario_name,
                self.duration.as_secs_f64(),
                reasons.join(", ")
            ),
            TestResult::Skipped(reason) => format!(
                "SKIP {} SKIPPED: {}",
                self.scenario_name,
                reason
            ),
            TestResult::Aborted(reason) => format!(
                "🚫 {} ABORTED: {}",
                self.scenario_name,
                reason
            ),
        }
    }
}

impl ActionResult {
    /// Create a successful action result
    pub fn success(action_name: String, action_type: String, duration: Duration) -> Self {
        Self {
            action_name,
            action_type,
            start_time: std::time::SystemTime::now() - duration,
            duration,
            result: ActionResultType::Success,
            error_message: None,
            action_data: HashMap::new(),
        }
    }
    
    /// Create a failed action result
    pub fn failed(action_name: String, action_type: String, error: String, duration: Duration) -> Self {
        Self {
            action_name,
            action_type,
            start_time: std::time::SystemTime::now() - duration,
            duration,
            result: ActionResultType::Failed,
            error_message: Some(error),
            action_data: HashMap::new(),
        }
    }
}

impl ValidationResult {
    /// Create a passing validation result
    pub fn passed(validation_type: String, details: String) -> Self {
        Self {
            validation_type,
            passed: true,
            details,
            expected: None,
            actual: None,
            check_time: std::time::SystemTime::now(),
        }
    }
    
    /// Create a failing validation result
    pub fn failed(validation_type: String, details: String, expected: Option<String>, actual: Option<String>) -> Self {
        Self {
            validation_type,
            passed: false,
            details,
            expected,
            actual,
            check_time: std::time::SystemTime::now(),
        }
    }
}

/// Convert TestAction to string for reporting
pub fn action_to_string(action: &TestAction) -> String {
    match action {
        TestAction::SendMessage { from, to, content, .. } => 
            format!("SendMessage({}→{}: '{}')", from, to, content.chars().take(20).collect::<String>()),
        TestAction::SendBroadcast { from, content, .. } => 
            format!("SendBroadcast({}→*: '{}')", from, content.chars().take(20).collect::<String>()),
        TestAction::ConnectPeers { peer1, peer2, .. } => 
            format!("ConnectPeers({}↔{})", peer1, peer2),
        TestAction::DisconnectPeers { peer1, peer2, .. } => 
            format!("DisconnectPeers({}↮{})", peer1, peer2),
        TestAction::StartDiscovery { peer, .. } => 
            format!("StartDiscovery({})", peer),
        TestAction::StopDiscovery { peer, .. } => 
            format!("StopDiscovery({})", peer),
        TestAction::LogCheckpoint { message, .. } => 
            format!("LogCheckpoint('{}')", message),
        TestAction::PauseScenario { duration_seconds } => 
            format!("PauseScenario({}s)", duration_seconds),
        TestAction::ValidateState { validation } => 
            format!("ValidateState({})", validation_to_string(validation)),
        TestAction::WaitForEvent { event_type, timeout_seconds } => 
            format!("WaitForEvent({}, {}s)", event_type, timeout_seconds),
        _ => format!("{:?}", action),
    }
}

/// Convert ValidationCheck to string for reporting
pub fn validation_to_string(check: &ValidationCheck) -> String {
    match check {
        ValidationCheck::MessageDelivered { from, to, content, .. } => 
            format!("MessageDelivered({}→{}: '{}')", from, to, content.chars().take(20).collect::<String>()),
        ValidationCheck::PeerConnected { peer1, peer2, .. } => 
            format!("PeerConnected({}↔{})", peer1, peer2),
        ValidationCheck::PeerCount { peer, expected_count, .. } => 
            format!("PeerCount({}: {})", peer, expected_count),
        ValidationCheck::MessageCount { peer, expected_count, .. } => 
            format!("MessageCount({}: {})", peer, expected_count),
        _ => format!("{:?}", check),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_report_creation() {
        let mut report = TestReport::new("test_scenario".to_string(), "1.0".to_string());
        assert_eq!(report.scenario_name, "test_scenario");
        assert_eq!(report.scenario_version, "1.0");
        assert!(matches!(report.result, TestResult::Passed));
        
        // Add a failing validation
        let validation = ValidationResult::failed(
            "MessageDelivered".to_string(),
            "Message not received".to_string(),
            Some("hello".to_string()),
            None,
        );
        report.add_validation_result(validation);
        
        assert!(matches!(report.result, TestResult::Failed(_)));
        assert!(!report.is_success());
    }
}