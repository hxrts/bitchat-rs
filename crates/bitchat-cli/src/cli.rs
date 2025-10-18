//! Command-line interface definitions and parsing

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<String>,

    /// Disable BLE transport
    #[arg(long)]
    pub no_ble: bool,

    /// Disable Nostr transport
    #[arg(long)]
    pub no_nostr: bool,

    /// Use only local Nostr relay
    #[arg(long)]
    pub local_relay: bool,

    /// Data directory for state persistence
    #[arg(short, long)]
    pub data_dir: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start interactive chat mode with TUI
    Chat {
        /// Your display name
        #[arg(short, long, default_value = "Anonymous")]
        name: String,
    },
    /// Send a single message and exit
    Send {
        /// Recipient peer ID (hex format)
        #[arg(short, long)]
        to: Option<String>,
        /// Message content
        message: String,
    },
    /// List discovered peers
    Peers {
        /// Watch for new peers continuously
        #[arg(short, long)]
        watch: bool,
    },
    /// Run transport tests
    Test {
        /// Run specific transport test
        #[arg(short, long)]
        transport: Option<String>,
    },
    /// Show application status
    Status,
    /// Start interactive command-line mode (for testing/automation)
    Interactive {
        /// Your display name
        #[arg(short, long, default_value = "TestClient")]
        name: String,
    },
}
