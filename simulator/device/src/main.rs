//! BitChat Emulator Rig
//!
//! Real-world end-to-end testing with actual iOS and Android devices.
//! Implements the real-world side of the unified architecture described in simulator/ARCHITECTURE.md

use clap::{Parser, Subcommand};
use tracing::{info, error};
use std::path::PathBuf;
use bitchat_emulator_harness::{ClientType, EmulatorOrchestrator, TestConfig};

// Import shared types from simulator-shared crate (would need to be added to Cargo.toml)
// For now, we'll define a minimal version here

#[derive(Debug, Clone)]
struct ScenarioConfig {
    pub metadata: ScenarioMetadata,
    pub peers: Vec<PeerConfig>,
}

#[derive(Debug, Clone)]
struct ScenarioMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]  
struct PeerConfig {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub platform: Option<String>,
}

/// BitChat Emulator Rig - Real-World E2E Testing
#[derive(Parser)]
#[command(name = "bitchat-emulator-rig")]
#[command(about = "Real-world end-to-end testing with actual iOS and Android devices")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a TOML scenario file with real devices (NEW UNIFIED INTERFACE)
    Execute {
        /// Path to TOML scenario file
        scenario_file: PathBuf,
        /// Client types to use (comma-separated, e.g., "ios,android")
        #[arg(long)]
        clients: String,
    },
    /// Setup emulator environment
    Setup,
    /// Clean up emulators and test data
    Cleanup,
    
    // ============================================================================
    // Legacy commands (deprecated - use Execute instead)
    // ============================================================================
    /// [DEPRECATED] Run test scenario with flexible client type combinations
    #[command(hide = true)]
    Test {
        /// First client type
        #[arg(long, value_enum)]
        client1: ClientType,
        /// Second client type
        #[arg(long, value_enum)]
        client2: ClientType,
        /// Specific test scenario to run
        #[arg(short, long)]
        scenario: Option<String>,
    },
    /// [DEPRECATED] Run iOS emulator tests
    #[command(hide = true)]
    Ios {
        /// Specific test scenario to run
        #[arg(short, long)]
        scenario: Option<String>,
    },
    /// [DEPRECATED] Run Android emulator tests
    #[command(hide = true)]
    Android {
        /// Specific test scenario to run
        #[arg(short, long)]
        scenario: Option<String>,
    },
    /// [DEPRECATED] Run full compatibility matrix
    #[command(hide = true)]
    Matrix {
        /// Filter to specific platform combinations
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// [DEPRECATED] Run iOS ↔ iOS stability test
    #[command(hide = true)]
    IosToIos,
    /// [DEPRECATED] Run Android ↔ Android stability test
    #[command(hide = true)]
    AndroidToAndroid,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Commands::Execute { scenario_file, clients } => {
            info!("Executing scenario: {} with clients: {}", scenario_file.display(), clients);
            execute_scenario_file(scenario_file, clients).await?
        }
        Commands::Setup => {
            info!("Setting up emulator environment...");
            setup_environment().await?
        }
        Commands::Cleanup => {
            info!("Cleaning up emulator environment...");
            cleanup_environment().await?
        }
        
        // Legacy commands with deprecation warnings
        Commands::Test { client1, client2, scenario } => {
            eprintln!("DEPRECATED: Use 'execute' with TOML scenario instead");
            info!("Starting {} ↔ {} emulator testing...", client1, client2);
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_client_combination_test(client1, client2, scenario).await?;
        }
        Commands::Ios { scenario } => {
            eprintln!("DEPRECATED: Use 'execute scenarios/messaging_basic.toml --clients ios,ios' instead");
            info!("Starting iOS emulator testing...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_ios_tests(scenario).await?;
        }
        Commands::Android { scenario } => {
            eprintln!("DEPRECATED: Use 'execute scenarios/messaging_basic.toml --clients android,android' instead");
            info!("Starting Android emulator testing...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_android_tests(scenario).await?;
        }
        Commands::Matrix { filter } => {
            eprintln!("DEPRECATED: Use multiple 'execute' commands for different client combinations");
            info!("Running full emulator compatibility matrix...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_compatibility_matrix(filter).await?;
        }
        Commands::IosToIos => {
            eprintln!("DEPRECATED: Use 'execute scenarios/messaging_basic.toml --clients ios,ios' instead");
            info!("Starting iOS ↔ iOS stability test...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_ios_to_ios_test().await?;
        }
        Commands::AndroidToAndroid => {
            eprintln!("DEPRECATED: Use 'execute scenarios/messaging_basic.toml --clients android,android' instead");
            info!("Starting Android ↔ Android stability test...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_android_to_android_test().await?;
        }
    }

    Ok(())
}

/// Execute a TOML scenario file with real devices using the new unified interface
async fn execute_scenario_file(scenario_file: PathBuf, clients: String) -> anyhow::Result<()> {
    // Parse client types
    let client_types: Result<Vec<ClientType>, _> = clients
        .split(',')
        .map(|s| s.trim())
        .map(|s| match s.to_lowercase().as_str() {
            "ios" => Ok(ClientType::Ios),
            "android" => Ok(ClientType::Android),
            _ => Err(anyhow::anyhow!("Unknown client type: {}", s)),
        })
        .collect();
    
    let client_types = client_types?;
    
    if client_types.is_empty() {
        return Err(anyhow::anyhow!("No client types specified"));
    }
    
    if client_types.len() < 2 {
        return Err(anyhow::anyhow!("At least 2 client types required for E2E testing"));
    }
    
    // Load scenario from TOML file (simplified loading for now)
    let scenario = load_scenario_file(&scenario_file)?;
    
    info!("Running E2E test: {} ({:?})", scenario.metadata.name, client_types);
    
    // Create emulator orchestrator
    let config = TestConfig::default();
    let mut orchestrator = EmulatorOrchestrator::new(config);
    
    // For now, use legacy client combination testing
    // In full implementation, this would use the new RealWorldExecutor
    if client_types.len() >= 2 {
        let result = orchestrator.run_client_combination_test(
            client_types[0].clone(),
            client_types[1].clone(),
            Some(scenario.metadata.name.clone()),
        ).await;
        
        match result {
            Ok(_) => {
                println!("Real-world E2E test PASSED");
                std::process::exit(0);
            }
            Err(e) => {
                error!("Real-world E2E test FAILED: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    Ok(())
}

/// Load a scenario file (simplified implementation)
fn load_scenario_file(path: &PathBuf) -> anyhow::Result<ScenarioConfig> {
    // In a full implementation, this would use the shared ScenarioConfig
    // with proper TOML parsing. For now, we'll create a minimal version.
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Scenario file not found: {}", path.display()));
    }
    
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    
    // Create a minimal scenario config
    Ok(ScenarioConfig {
        metadata: ScenarioMetadata {
            name: filename.to_string(),
            description: format!("Real-world test scenario: {}", filename),
        },
        peers: vec![
            PeerConfig {
                name: "peer1".to_string(),
                platform: Some("mobile".to_string()),
            },
            PeerConfig {
                name: "peer2".to_string(), 
                platform: Some("mobile".to_string()),
            },
        ],
    })
}

/// Setup emulator environment
async fn setup_environment() -> anyhow::Result<()> {
    println!("Setting up emulator environment...");
    
    // Check iOS environment
    println!("Checking iOS environment...");
    let ios_check = std::process::Command::new("xcodebuild")
        .arg("-version")
        .output();
    
    match ios_check {
        Ok(output) if output.status.success() => {
            println!("iOS development environment ready");
            let version = String::from_utf8_lossy(&output.stdout);
            println!("   {}", version.lines().next().unwrap_or("Unknown version"));
        }
        _ => {
            println!("iOS development environment not ready");
            println!("   Install Xcode from the App Store");
        }
    }
    
    // Check Android environment
    println!("Checking Android environment...");
    let android_home = std::env::var("ANDROID_HOME").ok();
    match android_home {
        Some(path) if std::path::Path::new(&path).exists() => {
            println!("Android SDK found at: {}", path);
        }
        _ => {
            println!("Android SDK not found");
            println!("   Set ANDROID_HOME environment variable");
            println!("   Install Android Studio or standalone SDK");
        }
    }
    
    // Check Appium
    let appium_check = std::process::Command::new("appium")
        .arg("--version")
        .output();
    
    match appium_check {
        Ok(output) if output.status.success() => {
            println!("Appium ready");
            let version = String::from_utf8_lossy(&output.stdout);
            println!("   Version: {}", version.trim());
        }
        _ => {
            println!("Appium not found (optional for some tests)");
            println!("   Install with: npm install -g appium");
        }
    }
    
    println!();
    println!("Environment setup complete!");
    println!("   Use 'just build-e2e' to build mobile apps");
    println!("   Use 'just test-e2e SCENARIO CLIENT1,CLIENT2' to run tests");
    
    Ok(())
}

/// Cleanup emulator environment
async fn cleanup_environment() -> anyhow::Result<()> {
    println!("Cleaning up emulator environment...");
    
    // Shutdown iOS simulators
    println!("Shutting down iOS simulators...");
    let _ = std::process::Command::new("xcrun")
        .args(&["simctl", "shutdown", "all"])
        .output();
    println!("iOS simulators shutdown");
    
    // Stop Android emulators
    println!("Stopping Android emulators...");
    let _ = std::process::Command::new("adb")
        .args(&["emu", "kill"])
        .output();
    println!("Android emulators stopped");
    
    // Clean build artifacts
    println!("Cleaning build artifacts...");
    let _ = std::fs::remove_dir_all("./vendored/bitchat-ios/build");
    let _ = std::fs::remove_dir_all("./vendored/bitchat-android/build");
    println!("Build artifacts cleaned");
    
    println!();
    println!("Cleanup complete!");
    
    Ok(())
}