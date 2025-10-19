use clap::{Parser, Subcommand};
use tracing::info;
use bitchat_emulator_harness::{ClientType, EmulatorOrchestrator, TestConfig};

#[derive(Parser)]
#[command(name = "bitchat-emulator-harness")]
#[command(about = "BitChat Emulator Testing Harness - Real App Black Box Testing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}


#[derive(Subcommand)]
enum Commands {
    /// Run test scenario with flexible client type combinations
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
    /// Run iOS emulator tests (legacy - use 'test --client1 ios --client2 ios')
    Ios {
        /// Specific test scenario to run
        #[arg(short, long)]
        scenario: Option<String>,
    },
    /// Run Android emulator tests (legacy - use 'test --client1 android --client2 android')
    Android {
        /// Specific test scenario to run
        #[arg(short, long)]
        scenario: Option<String>,
    },
    /// Run full compatibility matrix
    Matrix {
        /// Filter to specific platform combinations
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Setup emulator environment
    Setup,
    /// Clean up emulators and test data
    Cleanup,
    /// Run iOS ↔ iOS stability test (equivalent to old ios-simulator-test)
    IosToIos,
    /// Run Android ↔ Android stability test
    AndroidToAndroid,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Test { client1, client2, scenario } => {
            info!("Starting {} ↔ {} emulator testing...", client1, client2);
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_client_combination_test(client1, client2, scenario).await?;
        }
        Commands::Ios { scenario } => {
            info!("Starting iOS emulator testing (legacy command - consider using 'test --client1 ios --client2 ios')...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_ios_tests(scenario).await?;
        }
        Commands::Android { scenario } => {
            info!("Starting Android emulator testing (legacy command - consider using 'test --client1 android --client2 android')...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_android_tests(scenario).await?;
        }
        Commands::Matrix { filter } => {
            info!("Running full compatibility matrix...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_compatibility_matrix(filter).await?;
        }
        Commands::Setup => {
            info!("Setting up emulator environment...");
            let config = TestConfig::default();
            let orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.setup_environment().await?;
        }
        Commands::Cleanup => {
            info!("Cleaning up emulator environment...");
            let config = TestConfig::default();
            let orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.cleanup_environment().await?;
        }
        Commands::IosToIos => {
            info!("Starting iOS ↔ iOS stability test...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_ios_to_ios_test().await?;
        }
        Commands::AndroidToAndroid => {
            info!("Starting Android ↔ Android stability test...");
            let config = TestConfig::default();
            let mut orchestrator = EmulatorOrchestrator::new(config);
            orchestrator.run_android_to_android_test().await?;
        }
    }

    Ok(())
}