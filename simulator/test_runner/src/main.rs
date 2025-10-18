//! BitChat Integration Test Runner
//!
//! This application orchestrates integration tests between different BitChat client implementations
//! (Rust, Swift, Kotlin) to ensure cross-client compatibility and protocol conformance.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn};

mod orchestrator;
mod scenarios;

use orchestrator::TestOrchestrator;

/// BitChat Integration Test Runner
#[derive(Parser)]
#[command(name = "bitchat-test-runner")]
#[command(about = "Integration test runner for BitChat cross-client compatibility")]
#[command(version)]
struct Cli {
    /// Test scenario to run
    #[command(subcommand)]
    command: Option<Commands>,

    /// Relay URL to use for testing
    #[arg(long, default_value = "wss://relay.damus.io")]
    relay: String,

    /// Timeout for individual test operations (seconds)
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Path to BitChat CLI binary
    #[arg(long)]
    bitchat_cli_path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a specific test scenario
    Scenario {
        /// Name of the scenario to run
        name: String,
    },
    /// List available test scenarios
    List,
    /// Run all test scenarios
    All,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("bitchat_test_runner={}", log_level))
        .init();

    info!("Starting BitChat Integration Test Runner");
    info!("Relay: {}", cli.relay);

    // Create test orchestrator
    let mut orchestrator = TestOrchestrator::new(cli.relay, cli.timeout)
        .context("Failed to create test orchestrator")?;

    // Set custom CLI path if provided
    if let Some(cli_path) = cli.bitchat_cli_path {
        orchestrator.set_bitchat_cli_path(cli_path);
    }

    match cli.command.unwrap_or(Commands::List) {
        Commands::Scenario { name } => {
            info!("Running scenario: {}", name);
            run_scenario(&mut orchestrator, &name).await?;
        }
        Commands::List => {
            list_scenarios();
        }
        Commands::All => {
            info!("Running all test scenarios");
            run_all_scenarios(&mut orchestrator).await?;
        }
    }

    info!("Test runner completed");
    Ok(())
}

/// Run a specific test scenario
async fn run_scenario(orchestrator: &mut TestOrchestrator, scenario_name: &str) -> Result<()> {
    match scenario_name {
        "basic-send-receive" => scenarios::basic_send_receive::run(orchestrator).await,
        "handshake-test" => scenarios::handshake_test::run(orchestrator).await,
        "cross-client-test" => scenarios::cross_client_test::run(orchestrator).await,
        _ => {
            anyhow::bail!("Unknown scenario: {}", scenario_name);
        }
    }
}

/// Run all available test scenarios
async fn run_all_scenarios(orchestrator: &mut TestOrchestrator) -> Result<()> {
    let scenarios = ["basic-send-receive", "handshake-test", "cross-client-test"];

    let mut passed = 0;
    let mut failed = 0;

    for scenario in &scenarios {
        info!("Running scenario: {}", scenario);

        match run_scenario(orchestrator, scenario).await {
            Ok(_) => {
                info!("Scenario {} passed", scenario);
                passed += 1;
            }
            Err(e) => {
                warn!("Scenario {} failed: {}", scenario, e);
                failed += 1;
            }
        }
    }

    info!("Test summary: {} passed, {} failed", passed, failed);

    if failed > 0 {
        anyhow::bail!("{} scenarios failed", failed);
    }

    Ok(())
}

/// List all available test scenarios
fn list_scenarios() {
    println!("Available test scenarios:");
    println!("  basic-send-receive  - Test message sending between Rust clients");
    println!("  handshake-test     - Test Noise protocol handshake establishment");
    println!("  cross-client-test  - Test cross-client compatibility (Rust <-> Swift <-> Kotlin)");
    println!();
    println!("Usage:");
    println!("  bitchat-test-runner scenario <name>  - Run specific scenario");
    println!("  bitchat-test-runner all              - Run all scenarios");
}
