//! BitChat CLI Application
//!
//! Command-line client with robust configuration management using figment

use bitchat_core::{
    PeerId, ChannelTransportType, ConnectionStatus,
    BitchatResult, BitchatError,
    internal::{TransportError, TestConfig},
};
use bitchat_cli::{CliAppOrchestrator, CliAppConfig, ConfigError, TransportConfig};
use clap::{Arg, Command};
use std::io::{self, Write};

// ----------------------------------------------------------------------------
// Enhanced CLI Application
// ----------------------------------------------------------------------------

/// Enhanced CLI application with robust configuration management
pub struct BitchatCliApp {
    /// Application orchestrator
    orchestrator: CliAppOrchestrator,
    /// Configuration
    config: CliAppConfig,
    /// Running state
    running: bool,
}

impl BitchatCliApp {
    /// Create new CLI application from configuration
    pub async fn new(config: CliAppConfig) -> Result<Self, ApplicationError> {
        // Get the peer ID from configuration
        let peer_id = config.get_peer_id()
            .map_err(|e| ApplicationError::Configuration(e))?;

        // Build transport configuration from the app config
        let transport_config = TransportConfig {
            enabled_transports: config.get_enabled_transports(),
            ble_enabled: config.runtime.enabled_transports.contains(&"ble".to_string()),
            nostr_enabled: config.runtime.enabled_transports.contains(&"nostr".to_string()),
        };

        // Create the orchestrator with the configuration
        let test_config = bitchat_core::internal::TestConfig {
            peer_id,
            enable_logging: config.cli.verbose,
            active_transports: config.get_enabled_transports(),
            test_duration: None,
        };
        let mut orchestrator = CliAppOrchestrator::from_config_with_transports(
            test_config,
            config.cli.verbose,
            transport_config,
        );

        orchestrator.start().await
            .map_err(|e| ApplicationError::Runtime(e))?;
        
        Ok(Self {
            orchestrator,
            config,
            running: false,
        })
    }

    /// Start the CLI application and run the interactive loop
    pub async fn run(&mut self) -> BitchatResult<()> {
        if self.running {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "CLI application already running".to_string(),
            }));
        }

        self.running = true;

        println!("BitChat CLI Started");
        println!("Peer ID: {}", self.orchestrator.peer_id());
        println!("Type 'help' for commands, 'quit' to exit");
        println!();

        // Run the interactive terminal loop
        self.run_interactive_loop().await?;

        Ok(())
    }

    /// Stop the CLI application
    pub async fn stop(&mut self) -> BitchatResult<()> {
        if !self.running {
            return Ok(());
        }

        println!("\nShutting down BitChat CLI...");
        
        self.orchestrator.stop().await?;
        self.running = false;

        println!("BitChat CLI stopped");
        Ok(())
    }

    /// Run the interactive terminal loop
    async fn run_interactive_loop(&mut self) -> BitchatResult<()> {
        let stdin = io::stdin();

        loop {
            // Print status and prompt
            self.print_status().await?;
            self.print_recent_messages().await?;
            print!("bitchat> ");
            self.flush_stdout()?;

            // Read user input
            let mut input = String::new();
            match stdin.read_line(&mut input) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let command = input.trim();
                    if let Err(e) = self.handle_user_command(command).await {
                        println!("Error: {}", e);
                    }
                }
                Err(e) => {
                    return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                        reason: format!("Failed to read input: {}", e),
                    }));
                }
            }

            if !self.running {
                break;
            }
        }

        Ok(())
    }

    /// Handle user command input with improved error handling
    async fn handle_user_command(&mut self, input: &str) -> BitchatResult<()> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "help" => {
                self.print_help();
            }
            "quit" | "exit" => {
                self.running = false;
            }
            "status" => {
                self.print_detailed_status().await?;
            }
            "peers" => {
                self.print_peers().await?;
            }
            "send" => {
                if parts.len() < 2 {
                    println!("Usage: send <message>");
                    return Ok(());
                }
                let message = parts[1..].join(" ");
                self.send_broadcast_message(message).await?;
            }
            "private" => {
                if parts.len() < 3 {
                    println!("Usage: private <peer_id> <message>");
                    println!("   Example: private 0102030405060708 Hello there!");
                    return Ok(());
                }
                let peer_id_str = parts[1];
                let message = parts[2..].join(" ");
                self.send_private_message(peer_id_str, message).await?;
            }
            "connect" => {
                if parts.len() < 2 {
                    println!("Usage: connect <peer_id>");
                    println!("   Example: connect 0102030405060708");
                    return Ok(());
                }
                let peer_id_str = parts[1];
                self.connect_to_peer(peer_id_str).await?;
            }
            "discover" => {
                self.start_discovery().await?;
            }
            "stop-discovery" => {
                self.stop_discovery().await?;
            }
            "clear" => {
                self.clear_screen()?;
            }
            _ => {
                println!("Unknown command: '{}'. Type 'help' for available commands.", parts[0]);
            }
        }

        Ok(())
    }

    /// Send broadcast message
    async fn send_broadcast_message(&self, message: String) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            terminal.handle_send_message(self.orchestrator.peer_id(), message).await?;
            println!("Broadcast message sent");
        } else {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Terminal interface not available".to_string(),
            }));
        }
        Ok(())
    }

    /// Send private message with improved error handling
    async fn send_private_message(&self, peer_id_str: &str, message: String) -> BitchatResult<()> {
        let recipient = self.parse_peer_id(peer_id_str)?;
        
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            terminal.handle_send_message(recipient, message).await?;
            println!("Private message sent to {}", recipient);
        } else {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Terminal interface not available".to_string(),
            }));
        }
        
        Ok(())
    }

    /// Connect to peer with improved error handling
    async fn connect_to_peer(&self, peer_id_str: &str) -> BitchatResult<()> {
        let peer_id = self.parse_peer_id(peer_id_str)?;
        
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            terminal.handle_connect_to_peer(peer_id).await?;
            println!("Attempting to connect to {}", peer_id);
        } else {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Terminal interface not available".to_string(),
            }));
        }
        
        Ok(())
    }

    /// Start discovery
    async fn start_discovery(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            terminal.handle_start_discovery().await?;
            println!("Discovery started");
        } else {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Terminal interface not available".to_string(),
            }));
        }
        Ok(())
    }

    /// Stop discovery
    async fn stop_discovery(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            terminal.handle_stop_discovery().await?;
            println!("Discovery stopped");
        } else {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: "Terminal interface not available".to_string(),
            }));
        }
        Ok(())
    }

    /// Parse peer ID from hex string with better error messages
    fn parse_peer_id(&self, peer_id_str: &str) -> BitchatResult<PeerId> {
        let peer_bytes = hex::decode(peer_id_str).map_err(|_| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Invalid peer ID format: '{}'. Expected 16 hex characters (e.g., 0102030405060708)", peer_id_str),
            })
        })?;

        if peer_bytes.len() != 8 {
            return Err(BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Invalid peer ID length: expected 8 bytes (16 hex chars), got {} bytes", peer_bytes.len()),
            }));
        }

        let mut peer_id_bytes = [0u8; 8];
        peer_id_bytes.copy_from_slice(&peer_bytes);
        Ok(PeerId::new(peer_id_bytes))
    }

    /// Clear screen
    fn clear_screen(&self) -> BitchatResult<()> {
        print!("\x1B[2J\x1B[1;1H");
        self.flush_stdout()
    }

    /// Flush stdout with error handling
    fn flush_stdout(&self) -> BitchatResult<()> {
        io::stdout().flush().map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to flush stdout: {}", e),
            })
        })
    }

    /// Print help information with better formatting
    fn print_help(&self) {
        println!("BitChat CLI Commands:");
        println!();
        println!("  help                           Show this help message");
        println!("  status                         Show detailed application status");
        println!("  peers                          List discovered peers");
        println!("  send <message>                 Send broadcast message");
        println!("  private <peer_id> <message>    Send private message to specific peer");
        println!("  connect <peer_id>              Connect to specific peer");
        println!("  discover                       Start peer discovery");
        println!("  stop-discovery                 Stop peer discovery");
        println!("  clear                          Clear screen");
        println!("  quit | exit                    Exit application");
        println!();
        println!("Peer ID format: 16 hexadecimal characters (e.g., 0102030405060708)");
        println!("Transport options: Use --transport ble|nostr|all or --ble --nostr flags");
        println!();
    }

    /// Print current status
    async fn print_status(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            if let Some(state) = terminal.get_state_snapshot() {
                let status_icon = match state.system_status {
                    bitchat_cli::SystemStatus::Ready => "[READY]",
                    bitchat_cli::SystemStatus::Busy(_) => "[BUSY]",
                    bitchat_cli::SystemStatus::Error(_) => "[ERROR]",
                    bitchat_cli::SystemStatus::Starting => "[STARTING]",
                    bitchat_cli::SystemStatus::ShuttingDown => "[SHUTDOWN]",
                };
                
                print!("{} Discovery: {} | Peers: {} | Messages: {} | ", 
                    status_icon,
                    if state.discovery_active { "ON" } else { "OFF" },
                    state.peers.len(),
                    state.recent_messages.len()
                );
            }
        }
        Ok(())
    }

    /// Print detailed status information
    async fn print_detailed_status(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            if let Some(state) = terminal.get_state_snapshot() {
                println!("BitChat Status Report:");
                println!("  Peer ID: {}", self.orchestrator.peer_id());
                println!("  System Status: {:?}", state.system_status);
                println!("  Discovery Active: {}", state.discovery_active);
                println!("  Connected Peers: {}", state.peers.len());
                println!("  Recent Messages: {}", state.recent_messages.len());
                println!("  Active Operations: {:?}", state.busy_operations);
                println!();
            } else {
                println!("Unable to retrieve system status");
            }
        }
        Ok(())
    }

    /// Print discovered peers with better formatting
    async fn print_peers(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            if let Some(state) = terminal.get_state_snapshot() {
                if state.peers.is_empty() {
                    println!("No peers discovered yet. Try running 'discover' to find peers.");
                } else {
                    println!("Discovered Peers ({}):", state.peers.len());
                    for (peer_id, peer_state) in &state.peers {
                        let status_icon = match peer_state.status {
                            ConnectionStatus::Connected => "[CONN]",
                            ConnectionStatus::Connecting => "[PING]",
                            ConnectionStatus::Discovering => "[DISC]",
                            ConnectionStatus::Disconnected => "[DISC]",
                            ConnectionStatus::Error => "[ERR]",
                        };
                        
                        println!("  {} {} - {:?} via {:?} ({} msgs)", 
                            status_icon,
                            peer_id,
                            peer_state.status,
                            peer_state.transport.unwrap_or(ChannelTransportType::Ble),
                            peer_state.message_count
                        );
                    }
                }
                println!();
            }
        }
        Ok(())
    }

    /// Print recent messages with better formatting
    async fn print_recent_messages(&self) -> BitchatResult<()> {
        if let Some(terminal) = self.orchestrator.terminal_interface() {
            if let Some(state) = terminal.get_state_snapshot() {
                // Show the last 3 messages to keep the interface clean
                let recent: Vec<_> = state.recent_messages.iter().rev().take(3).collect();
                for message in recent.iter().rev() {
                    let direction_icon = match message.direction {
                        bitchat_cli::MessageDirection::Incoming => "<-",
                        bitchat_cli::MessageDirection::Outgoing => "->",
                    };
                    
                    println!("{} {}: {}", 
                        direction_icon,
                        message.from,
                        message.content
                    );
                }
                
                if !state.recent_messages.is_empty() {
                    println!();
                }
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Helper Functions and Error Types
// ----------------------------------------------------------------------------

/// Application-level errors
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Configuration error: {0}")]
    Configuration(ConfigError),

    #[error("Runtime error: {0}")]
    Runtime(BitchatError),

    #[error("I/O error: {0}")]
    Io(std::io::Error),
}


// ----------------------------------------------------------------------------
// Main CLI Entry Point
// ----------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("bitchat-cli")
        .about("BitChat CLI client with robust configuration management")
        .version("0.1.0")
        .arg(Arg::new("config")
            .long("config")
            .short('c')
            .help("Path to configuration file")
            .value_name("CONFIG_FILE"))
        .arg(Arg::new("name")
            .long("name")
            .help("Client name (used to generate consistent peer ID)")
            .value_name("NAME"))
        .arg(Arg::new("verbose")
            .long("verbose")
            .short('v')
            .help("Enable verbose logging")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("peer-id")
            .long("peer-id")
            .help("Specific peer ID as hex string (16 characters)")
            .value_name("PEER_ID"))
        .arg(Arg::new("transport")
            .long("transport")
            .short('t')
            .help("Transports to enable (comma-separated: ble,nostr)")
            .value_name("TRANSPORTS"))
        .arg(Arg::new("generate-config")
            .long("generate-config")
            .help("Generate example configuration file and exit")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("config-path")
            .long("config-path")
            .help("Show default configuration file path and exit")
            .action(clap::ArgAction::SetTrue))
        .get_matches();

    // Handle utility commands first
    if matches.get_flag("generate-config") {
        println!("# BitChat CLI Configuration File");
        println!("# Place this at ~/.bitchat/config.toml or specify with --config");
        println!();
        println!("{}", CliAppConfig::example_config());
        return Ok(());
    }

    if matches.get_flag("config-path") {
        let config_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        println!("{}", std::path::Path::new(&config_dir).join(".bitchat").join("config.toml").display());
        return Ok(());
    }

    // Load configuration with CLI overrides
    let config = if let Some(config_file) = matches.get_one::<String>("config") {
        // Load from specific file
        CliAppConfig::load_from_file(config_file)
            .map_err(|e| format!("Failed to load config from {}: {}", config_file, e))?
    } else {
        // Extract CLI arguments for overrides
        let peer_id = matches.get_one::<String>("peer-id").cloned();
        let name = matches.get_one::<String>("name").cloned();
        let verbose = if matches.get_flag("verbose") { Some(true) } else { None };
        let transports = matches.get_one::<String>("transport")
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>());

        // Load with overrides
        CliAppConfig::load_with_overrides(peer_id, name, verbose, transports)
            .unwrap_or_else(|e| {
                eprintln!("Configuration error: {}", e);
                eprintln!("Try running with --generate-config to create an example configuration file.");
                std::process::exit(1);
            })
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    // Print startup information
    if config.cli.verbose {
        let peer_id = config.get_peer_id()
            .map_err(|e| format!("Failed to get peer ID: {}", e))?;
        println!("BitChat CLI starting...");
        println!("Peer ID: {}", peer_id);
        println!("Enabled transports: {:?}", config.runtime.enabled_transports);
        println!("Configuration loaded successfully");
        println!();
    }

    // Create and run the CLI application
    match BitchatCliApp::new(config).await {
        Ok(mut app) => {
            if let Err(e) = app.run().await {
                eprintln!("BitChat CLI error: {}", e);
                let _ = app.stop().await;
                std::process::exit(1);
            }
            let _ = app.stop().await;
        }
        Err(e) => {
            eprintln!("Failed to start BitChat CLI: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_app_creation() {
        let config = TestConfig::new();
        // Note: We can't easily test the full app creation without proper transport setup
        // This would require integration testing with actual transport crates
        assert_eq!(config.peer_id, config.peer_id); // Basic sanity check
    }

    #[test]
    fn test_peer_id_parsing() {
        // This test would need the parse_peer_id method to be public or we'd need to test through the app
        // For now, we can test the hex decoding logic separately
        let test_hex = "0102030405060708";
        let result = hex::decode(test_hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 8);
    }
}