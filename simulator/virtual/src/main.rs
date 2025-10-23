//! BitChat Scenario Runner
//!
//! Fast, deterministic protocol simulation for BitChat testing.
//! Implements the simulation side of the unified architecture described in simulator/ARCHITECTURE.md

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, error};
use std::path::PathBuf;

mod scenario_config;
mod network_router;
mod network_analysis;
mod clock;
mod random;

mod simulation_executor;

use bitchat_simulator_shared::{ScenarioExecutor, TestReport, ActionResultType, ExecutorData};
use simulation_executor::SimulationExecutor;
use scenario_config::ScenarioConfig;

// Local conversion function
fn convert_to_shared_config(local: &ScenarioConfig) -> bitchat_simulator_shared::ScenarioConfig {
    use bitchat_simulator_shared::{ScenarioConfig as SharedConfig, ScenarioMetadata, SharedPeerConfig, TestStep, ValidationConfig, StateValidation};
    
    SharedConfig {
        metadata: ScenarioMetadata {
            name: local.metadata.name.clone(),
            description: local.metadata.description.clone(),
            version: local.metadata.version.clone(),
        },
        peers: local.peers.iter().map(|p| SharedPeerConfig {
            name: p.name.clone(),
            platform: None, // TODO: Extract from local config if available
            start_delay_seconds: p.start_delay_seconds,
        }).collect(),
        sequence: local.sequence.iter().map(|s| TestStep {
            name: s.name.clone(),
            at_time_seconds: s.at_time_seconds,
            action: convert_action(&s.action),
        }).collect(),
        validation: ValidationConfig {
            final_checks: local.validation.final_checks.iter().map(|v| StateValidation {
                check: convert_validation_check(&v.check),
            }).collect(),
        },
    }
}

/// Convert local TestAction to shared TestAction
fn convert_action(local: &scenario_config::TestAction) -> bitchat_simulator_shared::TestAction {
    use bitchat_simulator_shared::TestAction;
    use scenario_config::TestAction as LocalTestAction;
    
    match local {
        LocalTestAction::SendMessage { from, to, content } => TestAction::SendMessage {
            from: from.clone(),
            to: to.clone(),
            content: content.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::SendBroadcast { from, content } => TestAction::SendBroadcast {
            from: from.clone(),
            content: content.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::ConnectPeers { peer1, peer2 } => TestAction::ConnectPeers {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::DisconnectPeers { peer1, peer2 } => TestAction::DisconnectPeers {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::StartDiscovery { peer } => TestAction::StartDiscovery {
            peer: peer.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::StopDiscovery { peer } => TestAction::StopDiscovery {
            peer: peer.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::LogCheckpoint { message } => TestAction::LogCheckpoint {
            message: message.clone(),
            at_time_seconds: None,
        },
        LocalTestAction::PauseScenario { duration_seconds } => TestAction::PauseScenario {
            duration_seconds: *duration_seconds,
        },
        LocalTestAction::ValidateState { validation } => TestAction::ValidateState {
            validation: convert_validation_check(&validation.check),
        },
        _ => {
            // For unhandled actions, convert to a LogCheckpoint
            TestAction::LogCheckpoint {
                message: format!("Unhandled action: {:?}", local),
                at_time_seconds: None,
            }
        }
    }
}

/// Convert local ValidationCheck to shared ValidationCheck  
fn convert_validation_check(local: &scenario_config::ValidationCheck) -> bitchat_simulator_shared::ValidationCheck {
    use bitchat_simulator_shared::ValidationCheck;
    use scenario_config::ValidationCheck as LocalValidationCheck;
    
    match local {
        LocalValidationCheck::MessageDelivered { from, to, content } => ValidationCheck::MessageDelivered {
            from: from.clone(),
            to: to.clone(),
            content: content.clone(),
            timeout_seconds: 30, // Default timeout
        },
        LocalValidationCheck::PeerConnected { peer1, peer2 } => ValidationCheck::PeerConnected {
            peer1: peer1.clone(),
            peer2: peer2.clone(),
            timeout_seconds: 30,
        },
        LocalValidationCheck::MessageCount { peer, expected_min, expected_max: _ } => ValidationCheck::MessageCount {
            peer: peer.clone(),
            expected_count: expected_min.unwrap_or(0) as usize,
            timeout_seconds: 30,
        },
        _ => ValidationCheck::Custom {
            name: format!("Unhandled validation: {:?}", local),
            parameters: serde_json::Value::Null,
            timeout_seconds: 30,
        }
    }
}

/// BitChat Scenario Runner - Fast Protocol Simulation
#[derive(Parser)]
#[command(name = "bitchat-scenario-runner")]
#[command(about = "Fast, deterministic protocol simulation for BitChat testing")]
#[command(version)]
struct Cli {
    /// Command to run
    #[command(subcommand)]
    command: Commands,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a TOML scenario file (NEW UNIFIED INTERFACE)
    Execute {
        /// Path to TOML scenario file
        scenario_file: PathBuf,
    },
    /// List available built-in scenarios
    List,
    /// Run all available scenarios
    All,
    
    // ============================================================================
    // Legacy commands (deprecated - use Execute instead)
    // ============================================================================
    /// [DEPRECATED] Run a specific scenario by name
    #[command(hide = true)]
    Scenario {
        name: String,
    },
    /// [DEPRECATED] Run deterministic messaging test
    #[command(hide = true)]
    DeterministicMessaging,
    /// [DEPRECATED] Run security conformance test  
    #[command(hide = true)]
    SecurityConformance,
    /// [DEPRECATED] Run all scenarios
    #[command(hide = true)]
    AllScenarios,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let filter = if std::env::var("RUST_LOG").is_err() {
        "info"
    } else {
        "debug"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Execute { scenario_file } => {
            info!("Executing scenario: {}", scenario_file.display());
            execute_scenario_file(scenario_file).await
        }
        Commands::List => {
            list_scenarios().await
        }
        Commands::All => {
            run_all_scenarios().await
        }
        
        // Legacy commands with deprecation warnings
        Commands::Scenario { name } => {
            eprintln!("DEPRECATED: Use 'execute scenarios/{}.toml' instead", name);
            execute_built_in_scenario(&name).await
        }
        Commands::DeterministicMessaging => {
            eprintln!("DEPRECATED: Use 'execute scenarios/deterministic_messaging.toml' instead");
            execute_built_in_scenario("deterministic_messaging").await
        }
        Commands::SecurityConformance => {
            eprintln!("DEPRECATED: Use 'execute scenarios/security_conformance.toml' instead");
            execute_built_in_scenario("security_conformance").await
        }
        Commands::AllScenarios => {
            eprintln!("DEPRECATED: Use 'all' instead");
            run_all_scenarios().await
        }
    }
}

/// Execute a TOML scenario file using the new unified interface
async fn execute_scenario_file(scenario_file: PathBuf) -> Result<()> {
    // Load scenario from TOML file
    let scenario = ScenarioConfig::from_toml_file(&scenario_file)?;
    
    // Create simulation executor
    let mut executor = SimulationExecutor::new();
    
    // Convert to shared config
    let shared_scenario = convert_to_shared_config(&scenario);
    
    // Execute scenario
    match executor.execute_scenario(&shared_scenario).await {
        Ok(report) => {
            print_test_report(&report);
            if report.is_success() {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Scenario execution failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Execute a built-in scenario (legacy support)
async fn execute_built_in_scenario(name: &str) -> Result<()> {
    let scenario_path = PathBuf::from(format!("scenarios/{}.toml", name));
    
    if !scenario_path.exists() {
        error!("Built-in scenario '{}' not found at {}", name, scenario_path.display());
        eprintln!("Available scenarios:");
        list_scenarios().await?;
        std::process::exit(1);
    }
    
    execute_scenario_file(scenario_path).await
}

/// List all available scenarios
async fn list_scenarios() -> Result<()> {
    println!("Available Scenarios:");
    println!("===================");
    
    let scenarios_dir = PathBuf::from("scenarios");
    if !scenarios_dir.exists() {
        println!("No scenarios directory found");
        return Ok(());
    }
    
    let mut entries = std::fs::read_dir(scenarios_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    
    entries.sort_by_key(|entry| entry.file_name());
    
    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let scenario_name = name_str.strip_suffix(".toml").unwrap_or(&name_str);
        
        // Try to load scenario to get description
        if let Ok(scenario) = ScenarioConfig::from_toml_file(&entry.path()) {
            println!("  {} - {}", scenario_name, scenario.metadata.description);
        } else {
            println!("  {} - (error loading scenario)", scenario_name);
        }
    }
    
    println!();
    println!("Usage:");
    println!("  cargo run -- execute scenarios/SCENARIO_NAME.toml");
    println!("  just test-sim SCENARIO_NAME  # From simulator root");
    
    Ok(())
}

/// Run all available scenarios
async fn run_all_scenarios() -> Result<()> {
    let scenarios_dir = PathBuf::from("scenarios");
    if !scenarios_dir.exists() {
        error!("No scenarios directory found");
        return Ok(());
    }
    
    let entries = std::fs::read_dir(scenarios_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    
    if entries.is_empty() {
        println!("No TOML scenarios found");
        return Ok(());
    }
    
    println!("Running {} scenarios...", entries.len());
    println!();
    
    let mut passed = 0;
    let mut failed = 0;
    
    for entry in entries {
        let scenario_name = entry.file_name().to_string_lossy().to_string();
        println!("Running {}...", scenario_name);
        
        match ScenarioConfig::from_toml_file(&entry.path()) {
            Ok(scenario) => {
                let mut executor = SimulationExecutor::new();
                let shared_scenario = convert_to_shared_config(&scenario);
                match executor.execute_scenario(&shared_scenario).await {
                    Ok(report) => {
                        if report.is_success() {
                            println!("PASS {}", report.summary());
                            passed += 1;
                        } else {
                            println!("FAIL {}", report.summary());
                            failed += 1;
                        }
                    }
                    Err(e) => {
                        println!("FAIL {} FAILED: {}", scenario_name, e);
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                println!("ERROR {} LOAD ERROR: {}", scenario_name, e);
                failed += 1;
            }
        }
        println!();
    }
    
    println!("Results: {} passed, {} failed", passed, failed);
    
    if failed > 0 {
        std::process::exit(1);
    }
    
    Ok(())
}

/// Print a test report in a human-readable format
fn print_test_report(report: &TestReport) {
    println!("Scenario Report");
    println!("==================");
    println!("Name: {}", report.scenario_name);
    println!("Version: {}", report.scenario_version);
    println!("Duration: {:.2}s", report.duration.as_secs_f64());
    println!("Result: {}", report.summary());
    println!();
    
    if !report.action_results.is_empty() {
        println!("Actions ({}):", report.action_results.len());
        for action in &report.action_results {
            let status = match action.result {
                ActionResultType::Success => "PASS",
                ActionResultType::Failed => "FAIL",
                ActionResultType::Skipped => "SKIP",
                ActionResultType::Timeout => "TIMEOUT",
            };
            println!("  {} {} ({:.2}s)", status, action.action_type, action.duration.as_secs_f64());
            if let Some(ref error) = action.error_message {
                println!("     Error: {}", error);
            }
        }
        println!();
    }
    
    if !report.validation_results.is_empty() {
        println!("Validations ({}):", report.validation_results.len());
        for validation in &report.validation_results {
            let status = if validation.passed { "PASS" } else { "FAIL" };
            println!("  {} {}", status, validation.validation_type);
            if !validation.passed {
                println!("     {}", validation.details);
                if let (Some(ref expected), Some(ref actual)) = (&validation.expected, &validation.actual) {
                    println!("     Expected: {}", expected);
                    println!("     Actual: {}", actual);
                }
            }
        }
        println!();
    }
    
    // Performance metrics
    if report.metrics.messages_sent > 0 || report.metrics.messages_received > 0 {
        println!("Metrics:");
        println!("  Messages sent: {}", report.metrics.messages_sent);
        println!("  Messages received: {}", report.metrics.messages_received);
        if let Some(latency) = report.metrics.avg_latency_ms {
            println!("  Average latency: {:.1}ms", latency);
        }
        if let Some(loss) = report.metrics.packet_loss_rate {
            println!("  Packet loss: {:.1}%", loss * 100.0);
        }
        println!();
    }
    
    // Executor-specific data
    match &report.executor_data {
        ExecutorData::Simulation { peer_count, time_steps, state_snapshots } => {
            println!("Simulation Data:");
            println!("  Peers: {}", peer_count);
            println!("  Time steps: {}", time_steps);
            println!("  State snapshots: {}", state_snapshots.len());
            
            if !state_snapshots.is_empty() {
                println!("  Latest snapshot: {}", state_snapshots.last().unwrap());
            }
        }
        ExecutorData::RealWorld { device_info, appium_sessions, environment } => {
            println!("Real-World Data:");
            println!("  Environment: {}", environment);
            println!("  Devices: {}", device_info.len());
            println!("  Appium sessions: {}", appium_sessions.len());
        }
    }
}