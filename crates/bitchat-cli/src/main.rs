//! BitChat CLI - Improved modular main entry point

use clap::Parser;
use tracing::{error, info};

use bitchat_cli::{
    app::BitchatApp,
    cli::Cli,
    commands::CommandDispatcher,
    config::AppConfig,
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging
    setup_logging(cli.verbose);

    // Load configuration
    let mut config = load_configuration(&cli)?;

    // Override config directory if specified
    if let Some(data_dir) = &cli.data_dir {
        config.state.state_dir = Some(data_dir.into());
    }

    // Create application
    let mut app = BitchatApp::new(config).await?;

    // Validate transport options
    if cli.no_ble && cli.no_nostr {
        error!("Error: At least one transport must be enabled");
        std::process::exit(1);
    }

    // Start transports
    info!("Initializing BitChat transports...");
    let use_ble = !cli.no_ble;
    let use_nostr = !cli.no_nostr;

    if let Err(e) = app.start_transports(use_ble, use_nostr, cli.local_relay).await {
        error!("Failed to start transports: {}", e);
        std::process::exit(1);
    }

    // Execute the command
    if let Err(e) = CommandDispatcher::execute(cli, app).await {
        error!("Command execution failed: {}", e);
        std::process::exit(1);
    }

    info!("BitChat CLI exited successfully");
    Ok(())
}

/// Setup logging based on verbosity level
fn setup_logging(verbose: bool) {
    let log_level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

/// Load configuration from file or use defaults
fn load_configuration(cli: &Cli) -> Result<AppConfig> {
    if let Some(config_path) = &cli.config {
        info!("Loading configuration from: {}", config_path);
        AppConfig::load_from_file(config_path)
    } else {
        info!("Using default configuration");
        Ok(AppConfig::default())
    }
}