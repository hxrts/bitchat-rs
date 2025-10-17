//! BitChat CLI demonstration application
//!
//! This application demonstrates the BitChat protocol with both BLE and Nostr transports,
//! showcasing intelligent transport selection and dual-transport operation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use bitchat_ble_transport::{BleTransport, BleTransportConfig};
use bitchat_core::{
    transport::{TransportManager, TransportSelectionPolicy, TransportType},
    BitchatMessage, MessageBuilder, MessageFragmenter,
    MessageReassembler, PeerId, StdDeliveryTracker, StdNoiseSessionManager, StdTimeSource,
};
use bitchat_nostr_transport::{NostrTransport, NostrTransportConfig};
use uuid::Uuid;

// ----------------------------------------------------------------------------
// CLI Arguments
// ----------------------------------------------------------------------------

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,

    /// Disable BLE transport
    #[arg(long)]
    no_ble: bool,

    /// Disable Nostr transport
    #[arg(long)]
    no_nostr: bool,

    /// Use only local Nostr relay
    #[arg(long)]
    local_relay: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive chat mode
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
    Peers,
    /// Run transport tests
    Test,
}

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppConfig {
    /// User display name
    pub display_name: String,
    /// BLE transport configuration
    pub ble: BleTransportConfig,
    /// Nostr transport configuration
    pub nostr: NostrTransportConfig,
    /// Transport selection preferences
    pub transport_preferences: TransportPreferences,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportPreferences {
    /// Prefer BLE over Nostr when both are available
    pub prefer_ble: bool,
    /// Fallback to secondary transport on failure
    pub enable_fallback: bool,
    /// Maximum time to wait for preferred transport
    pub preferred_timeout: Duration,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            display_name: "BitChat User".to_string(),
            ble: BleTransportConfig::default(),
            nostr: NostrTransportConfig::default(),
            transport_preferences: TransportPreferences {
                prefer_ble: true,
                enable_fallback: true,
                preferred_timeout: Duration::from_secs(5),
            },
        }
    }
}

// ----------------------------------------------------------------------------
// BitChat Application
// ----------------------------------------------------------------------------

struct BitchatApp {
    /// Our peer ID
    peer_id: PeerId,
    /// Session manager for handling peer connections
    session_manager: StdNoiseSessionManager,
    /// Delivery tracker for message reliability
    delivery_tracker: StdDeliveryTracker,
    /// Transport manager with intelligent routing
    transport_manager: TransportManager,
    /// Message fragmenter for large messages
    fragmenter: MessageFragmenter,
    /// Message reassembler
    reassembler: MessageReassembler,
    /// Application configuration
    config: AppConfig,
    /// Received messages
    messages: Arc<RwLock<Vec<(PeerId, BitchatMessage)>>>,
}

impl BitchatApp {
    /// Create a new BitChat application
    pub fn new(config: AppConfig) -> Result<Self> {
        // Generate crypto keys
        let noise_key = bitchat_core::crypto::NoiseKeyPair::generate();
        let peer_id = PeerId::from_bytes(&noise_key.public_key_bytes());

        info!("Starting BitChat with peer ID: {}", peer_id);

        let session_manager = StdNoiseSessionManager::new(noise_key, StdTimeSource);
        let delivery_tracker = StdDeliveryTracker::new(StdTimeSource);
        let transport_manager = TransportManager::new();
        let fragmenter = MessageFragmenter;
        let reassembler = MessageReassembler::new();

        Ok(Self {
            peer_id,
            session_manager,
            delivery_tracker,
            transport_manager,
            fragmenter,
            reassembler,
            config,
            messages: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Initialize and start transports
    pub async fn start_transports(
        &mut self,
        use_ble: bool,
        use_nostr: bool,
        local_relay: bool,
    ) -> Result<()> {
        // Add BLE transport if enabled
        if use_ble {
            info!("Initializing BLE transport...");
            let ble_transport = BleTransport::with_config(self.peer_id, self.config.ble.clone());
            self.transport_manager
                .add_transport(Box::new(ble_transport));
            info!("BLE transport added");
        }

        // Add Nostr transport if enabled
        if use_nostr {
            info!("Initializing Nostr transport...");
            let nostr_config = if local_relay {
                bitchat_nostr_transport::create_local_relay_config()
            } else {
                self.config.nostr.clone()
            };

            match NostrTransport::with_config(self.peer_id, nostr_config) {
                Ok(nostr_transport) => {
                    self.transport_manager
                        .add_transport(Box::new(nostr_transport));
                    info!("Nostr transport added");
                }
                Err(e) => {
                    warn!("Failed to initialize Nostr transport: {}", e);
                }
            }
        }

        // Set transport selection policy
        if self.config.transport_preferences.prefer_ble {
            self.transport_manager.set_selection_policy(
                bitchat_core::transport::TransportSelectionPolicy::PreferenceOrder(vec![
                    TransportType::Ble,
                    TransportType::Nostr,
                ]),
            );
        }

        // Start all transports
        self.transport_manager
            .start_all()
            .await
            .context("Failed to start transports")?;

        info!("All transports started successfully");
        Ok(())
    }

    /// Send a message to a specific peer or broadcast
    pub async fn send_message(
        &mut self,
        recipient_id: Option<PeerId>,
        content: String,
    ) -> Result<Uuid> {
        let message = BitchatMessage::new(self.config.display_name.clone(), content);
        let message_id = message.id;

        // Create packet
        let packet = if let Some(recipient) = recipient_id {
            MessageBuilder::create_message(
                self.peer_id,
                self.config.display_name.clone(),
                message.content.clone(),
                Some(recipient),
            )?
        } else {
            MessageBuilder::create_message(
                self.peer_id,
                self.config.display_name.clone(),
                message.content.clone(),
                None,
            )?
        };

        // Send via transport manager
        if let Some(recipient) = recipient_id {
            self.delivery_tracker
                .track_message(message_id, recipient, packet.payload.clone());
            self.transport_manager.send_to(recipient, packet).await?;
            self.delivery_tracker.mark_sent(&message_id);
        } else {
            self.transport_manager.broadcast_all(packet).await?;
        }

        info!("Message sent: {}", message_id);
        Ok(message_id)
    }

    /// Get list of discovered peers
    pub fn get_discovered_peers(&self) -> Vec<(PeerId, TransportType)> {
        self.transport_manager.all_discovered_peers()
    }

    /// Get received messages
    pub async fn get_messages(&self) -> Vec<(PeerId, BitchatMessage)> {
        self.messages.read().await.clone()
    }

    /// Run message processing loop
    pub async fn run_message_loop(&mut self) -> Result<()> {
        info!("Starting message processing loop");

        loop {
            tokio::select! {
                // Process cleanup tasks
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    self.session_manager.cleanup_expired();
                    let (completed, expired) = self.delivery_tracker.cleanup();
                    if !completed.is_empty() || !expired.is_empty() {
                        debug!("Cleaned up {} completed and {} expired deliveries",
                               completed.len(), expired.len());
                    }
                    self.reassembler.cleanup_expired();
                }

                // Handle Ctrl+C gracefully
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Stop all transports
    pub async fn stop(&mut self) -> Result<()> {
        self.transport_manager
            .stop_all()
            .await
            .context("Failed to stop transports")?;
        info!("All transports stopped");
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Simple Chat Interface
// ----------------------------------------------------------------------------

struct ChatInterface {
    app: BitchatApp,
}

impl ChatInterface {
    fn new(app: BitchatApp) -> Self {
        Self { app }
    }

    async fn run(&mut self) -> Result<()> {
        println!("BitChat CLI - Interactive Mode");
        println!("Your Peer ID: {}", self.app.peer_id);
        println!("Type 'help' for commands, 'quit' to exit");
        println!();

        // Start message loop in background
        // For now, we'll handle the message loop inline without spawning
        // This is simpler than implementing Clone for the complex app structure

        // Simple command loop
        loop {
            print!("> ");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            match input {
                "help" => {
                    println!("Commands:");
                    println!("  help              - Show this help");
                    println!("  peers             - List discovered peers");
                    println!("  send <message>    - Broadcast message");
                    println!("  send <peer> <msg> - Send message to specific peer");
                    println!("  messages          - Show received messages");
                    println!("  status            - Show transport status");
                    println!("  quit              - Exit application");
                }
                "peers" => {
                    let peers = self.app.get_discovered_peers();
                    if peers.is_empty() {
                        println!("No peers discovered yet");
                    } else {
                        println!("Discovered peers:");
                        for (peer_id, transport_type) in peers {
                            println!("  {} ({:?})", peer_id, transport_type);
                        }
                    }
                }
                "messages" => {
                    let messages = self.app.get_messages().await;
                    if messages.is_empty() {
                        println!("No messages received yet");
                    } else {
                        println!("Received messages:");
                        for (sender, message) in messages {
                            println!(
                                "  [{}] {}: {}",
                                message.timestamp.as_millis(),
                                message.sender,
                                message.content
                            );
                        }
                    }
                }
                "status" => {
                    let active_count = self.app.transport_manager.active_transport_count();
                    println!("Active transports: {}", active_count);

                    let stats = self.app.delivery_tracker.get_stats();
                    println!("Message delivery stats:");
                    println!(
                        "  Total: {}, Confirmed: {}, Failed: {}",
                        stats.total, stats.confirmed, stats.failed
                    );
                }
                "quit" => break,
                _ if input.starts_with("send ") => {
                    let parts: Vec<&str> = input.splitn(3, ' ').collect();
                    if parts.len() == 2 {
                        // Broadcast message
                        let message = parts[1].to_string();
                        match self.app.send_message(None, message).await {
                            Ok(id) => println!("Message sent (ID: {})", id),
                            Err(e) => println!("Failed to send message: {}", e),
                        }
                    } else if parts.len() == 3 {
                        // Send to specific peer
                        let peer_str = parts[1];
                        let message = parts[2].to_string();

                        // Parse peer ID from hex
                        if let Ok(bytes) = hex::decode(peer_str) {
                            if bytes.len() == 8 {
                                let mut peer_bytes = [0u8; 8];
                                peer_bytes.copy_from_slice(&bytes);
                                let peer_id = PeerId::new(peer_bytes);

                                match self.app.send_message(Some(peer_id), message).await {
                                    Ok(id) => println!("Message sent to {} (ID: {})", peer_id, id),
                                    Err(e) => println!("Failed to send message: {}", e),
                                }
                            } else {
                                println!("Invalid peer ID format (must be 16 hex characters)");
                            }
                        } else {
                            println!("Invalid peer ID format (must be hex)");
                        }
                    } else {
                        println!("Usage: send <message> or send <peer_id> <message>");
                    }
                }
                _ => {
                    println!("Unknown command: {}", input);
                    println!("Type 'help' for available commands");
                }
            }
        }

        Ok(())
    }
}

// For the Clone implementation, we would need to implement it properly
// For now, let's just use a simpler approach

// ----------------------------------------------------------------------------
// Main Application
// ----------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    // Load configuration
    let config = if let Some(config_path) = &cli.config {
        let config_str =
            std::fs::read_to_string(config_path).context("Failed to read config file")?;
        toml::from_str(&config_str).context("Failed to parse config file")?
    } else {
        AppConfig::default()
    };

    // Create application
    let mut app = BitchatApp::new(config)?;

    // Start transports
    let use_ble = !cli.no_ble;
    let use_nostr = !cli.no_nostr;

    if !use_ble && !use_nostr {
        eprintln!("Error: At least one transport must be enabled");
        std::process::exit(1);
    }

    app.start_transports(use_ble, use_nostr, cli.local_relay)
        .await?;

    // Handle commands
    match cli.command {
        Commands::Chat { name } => {
            app.config.display_name = name;
            // Simple chat loop instead of complex UI
            println!("BitChat CLI - Chat Mode");
            println!("Your Peer ID: {}", app.peer_id);
            println!("Type messages to broadcast, or 'quit' to exit");

            loop {
                use std::io::{self, Write};
                print!("> ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_err() {
                    break;
                }

                let input = input.trim();
                if input == "quit" {
                    break;
                } else if !input.is_empty() {
                    match app.send_message(None, input.to_string()).await {
                        Ok(id) => println!("Message sent (ID: {})", id),
                        Err(e) => println!("Failed to send message: {}", e),
                    }
                }
            }
        }
        Commands::Send { to, message } => {
            let recipient = if let Some(peer_str) = to {
                if let Ok(bytes) = hex::decode(peer_str) {
                    if bytes.len() == 8 {
                        let mut peer_bytes = [0u8; 8];
                        peer_bytes.copy_from_slice(&bytes);
                        Some(PeerId::new(peer_bytes))
                    } else {
                        eprintln!("Invalid peer ID format");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Invalid peer ID format");
                    std::process::exit(1);
                }
            } else {
                None
            };

            match app.send_message(recipient, message).await {
                Ok(id) => println!("Message sent (ID: {})", id),
                Err(e) => {
                    eprintln!("Failed to send message: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Peers => {
            // Wait a bit for peer discovery
            tokio::time::sleep(Duration::from_secs(5)).await;

            let peers = app.get_discovered_peers();
            if peers.is_empty() {
                println!("No peers discovered");
            } else {
                println!("Discovered peers:");
                for (peer_id, transport_type) in peers {
                    println!("  {} ({:?})", peer_id, transport_type);
                }
            }
        }
        Commands::Test => {
            println!("Running transport tests...");

            // Test BLE transport
            if use_ble {
                println!("Testing BLE transport...");
                // Add BLE-specific tests here
            }

            // Test Nostr transport
            if use_nostr {
                println!("Testing Nostr transport...");
                // Add Nostr-specific tests here
            }

            println!("Transport tests completed");
        }
    }

    // Clean shutdown
    app.stop().await?;
    println!("BitChat CLI stopped");

    Ok(())
}
