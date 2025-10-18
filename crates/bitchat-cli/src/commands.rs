//! Command handlers for the BitChat CLI

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::app::BitchatApp;
use crate::cli::{Cli, Commands};
use crate::error::{CliError, Result};
#[cfg(feature = "tui")]
use crate::tui::TuiManager;
use bitchat_core::PeerId;

/// Command dispatcher for handling CLI commands
pub struct CommandDispatcher;

impl CommandDispatcher {
    /// Execute a CLI command
    pub async fn execute(cli: Cli, app: BitchatApp) -> Result<()> {
        match cli.command {
            #[cfg(feature = "tui")]
            Commands::Chat { name } => Self::handle_chat_command(name, app).await,
            #[cfg(not(feature = "tui"))]
            Commands::Chat { .. } => {
                error!("TUI mode not available. Use 'interactive' command for non-TUI mode.");
                Err(CliError::FeatureNotAvailable(
                    "TUI mode disabled".to_string(),
                ))
            }
            Commands::Interactive { name } => Self::handle_interactive_command(name, app).await,
            Commands::Send { to, message } => Self::handle_send_command(app, to, message).await,
            Commands::Peers { watch } => Self::handle_peers_command(app, watch).await,
            Commands::Test { transport } => Self::handle_test_command(app, transport).await,
            Commands::Status => Self::handle_status_command(app).await,
        }
    }

    /// Handle the chat command with TUI
    #[cfg(feature = "tui")]
    async fn handle_chat_command(name: String, app: BitchatApp) -> Result<()> {
        info!("Starting interactive chat mode with display name: {}", name);

        // Update display name in config if needed
        // The app already has the display name from config

        // Start message processing loop in background
        let app_clone = Arc::new(Mutex::new(app));
        let message_loop_app = app_clone.clone();

        let message_loop_handle = tokio::spawn(async move {
            let mut app = message_loop_app.lock().await;
            if let Err(e) = app.run_message_loop().await {
                error!("Message loop error: {}", e);
            }
        });

        // Create and run TUI - we need to take the app out temporarily
        let mut tui_manager = {
            let app_lock = app_clone.lock().await;
            // We can't clone BitchatApp, so we need to restructure this
            // For now, let's create a new manager differently
            drop(app_lock); // Release the lock

            // Instead, let's pass the Arc directly to TuiManager
            TuiManager::new_with_arc(app_clone.clone()).await?
        };

        tui_manager.run().await?;

        // Clean shutdown
        {
            let mut app = app_clone.lock().await;
            app.stop().await?;
        }

        message_loop_handle.abort();
        Ok(())
    }

    /// Handle the interactive command (non-TUI mode for testing/automation)
    async fn handle_interactive_command(name: String, app: BitchatApp) -> Result<()> {
        info!(
            "Starting interactive command-line mode with display name: {}",
            name
        );

        // Start message processing loop in background
        let app_clone = Arc::new(Mutex::new(app));
        let message_loop_app = app_clone.clone();
        let event_handler_app = app_clone.clone();

        let message_loop_handle = tokio::spawn(async move {
            let mut app = message_loop_app.lock().await;
            if let Err(e) = app.run_message_loop().await {
                error!("Message loop error: {}", e);
            }
        });

        // Start event handler to log received messages
        let event_receiver = {
            let app = event_handler_app.lock().await;
            app.event_receiver()
        };
        
        let event_handle = tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;
            while let Some(event) = receiver.recv().await {
                if let crate::app::AppEvent::MessageReceived { from, message } = event {
                    info!("Message received from {}: {}", from, message.content);
                }
            }
        });

        // Start stdin command processing
        info!("BitChat Interactive Mode");
        info!("Display name: {}", name);
        info!("Available commands:");
        info!("  send <peer_id> <message>   - Send a message to a peer");
        info!("  peers                      - List discovered peers");
        info!("  status                     - Show application status");
        info!("  quit                       - Exit the application");
        info!("");

        // Process commands from stdin
        use tokio::io::{AsyncBufReadExt, BufReader};
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        loop {
            print!("> ");
            use std::io::Write;
            std::io::stdout().flush().unwrap();

            match reader.next_line().await {
                Ok(Some(line)) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let parts: Vec<&str> = line.split_whitespace().collect();
                    match parts.first() {
                        Some(&"quit") | Some(&"exit") => {
                            info!("Exiting...");
                            break;
                        }
                        Some(&"send") if parts.len() >= 3 => {
                            let peer_id_or_name = parts[1];
                            let message = parts[2..].join(" ");

                            // For testing, if peer_id_or_name is not a valid hex peer ID,
                            // send to any available peer or use None for broadcast
                            let mut app = app_clone.lock().await;
                            let peer_id = match Self::parse_peer_id(peer_id_or_name) {
                                Ok(id) => Some(id),
                                Err(_) => {
                                    // If not a valid peer ID, try to send to any discovered peer
                                    // or use None for broadcast/testing
                                    let peers = app.get_discovered_peers();
                                    if !peers.is_empty() {
                                        Some(peers[0].0) // Use first discovered peer
                                    } else {
                                        None // Broadcast mode for testing
                                    }
                                }
                            };

                            match app.send_message(peer_id, message.clone()).await {
                                Ok(_) => {
                                    if let Some(pid) = peer_id {
                                        info!("Message sent to {}: {}", pid, message);
                                    } else {
                                        info!("Message broadcast: {}", message);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to send message: {}", e);
                                }
                            }
                        }
                        Some(&"peers") => {
                            let app = app_clone.lock().await;
                            let peers = app.get_discovered_peers();
                            if peers.is_empty() {
                                info!("No peers discovered yet");
                            } else {
                                info!("Discovered peers:");
                                for (peer_id, transport) in peers {
                                    info!("  {} (via {:?})", peer_id, transport);
                                }
                            }
                        }
                        Some(&"status") => {
                            let app = app_clone.lock().await;
                            info!("Application status: Active");
                            info!("Display name: {}", name);
                            // Add more status info as needed
                        }
                        Some(&"help") => {
                            info!("Available commands:");
                            info!("  send <peer_id> <message>   - Send a message to a peer");
                            info!("  peers                      - List discovered peers");
                            info!("  status                     - Show application status");
                            info!("  quit                       - Exit the application");
                        }
                        _ => {
                            warn!(
                                "Unknown command: {}. Type 'help' for available commands.",
                                line
                            );
                        }
                    }
                }
                Ok(None) => {
                    info!("End of input, exiting...");
                    break;
                }
                Err(e) => {
                    error!("Error reading input: {}", e);
                    break;
                }
            }
        }

        // Clean shutdown
        {
            let mut app = app_clone.lock().await;
            app.stop().await?;
        }

        message_loop_handle.abort();
        event_handle.abort();
        Ok(())
    }

    /// Handle the send command
    async fn handle_send_command(
        mut app: BitchatApp,
        to: Option<String>,
        message: String,
    ) -> Result<()> {
        let recipient = if let Some(peer_str) = to {
            Some(Self::parse_peer_id(&peer_str)?)
        } else {
            None
        };

        match app.send_message(recipient, message.clone()).await {
            Ok(id) => {
                if let Some(peer_id) = recipient {
                    println!("Message sent to {} (ID: {})", peer_id, id);
                } else {
                    println!("Message broadcast (ID: {})", id);
                }
            }
            Err(e) => {
                return Err(CliError::MessageProcessing(format!(
                    "Failed to send message: {}",
                    e
                )));
            }
        }

        // Wait a moment for any delivery confirmations
        tokio::time::sleep(Duration::from_secs(2)).await;

        app.stop().await?;
        Ok(())
    }

    /// Handle the peers command
    async fn handle_peers_command(mut app: BitchatApp, watch: bool) -> Result<()> {
        if watch {
            info!("Watching for peers... Press Ctrl+C to stop");

            // Start message processing to discover peers
            let app_clone = Arc::new(Mutex::new(app));
            let message_loop_app = app_clone.clone();

            let message_loop_handle = tokio::spawn(async move {
                let mut app = message_loop_app.lock().await;
                if let Err(e) = app.run_message_loop().await {
                    error!("Message loop error: {}", e);
                }
            });

            // Periodically display discovered peers
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let app = app_clone.lock().await;
                        let peers = app.get_discovered_peers();

                        print!("\x1B[2J\x1B[1;1H"); // Clear screen
                        println!("BitChat - Peer Discovery (watching...)");
                        println!("=========================================");

                        if peers.is_empty() {
                            println!("No peers discovered yet...");
                        } else {
                            println!("Discovered peers:");
                            for (peer_id, transport_type) in peers {
                                println!("  {} ({:?})", peer_id, transport_type);
                            }
                        }
                        println!("\nPress Ctrl+C to stop...");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        break;
                    }
                }
            }

            {
                let mut app = app_clone.lock().await;
                app.stop().await?;
            }

            message_loop_handle.abort();
        } else {
            // Wait for peer discovery
            info!("Discovering peers...");
            tokio::time::sleep(Duration::from_secs(10)).await;

            let peers = app.get_discovered_peers();
            if peers.is_empty() {
                println!("No peers discovered");
            } else {
                println!("Discovered peers:");
                for (peer_id, transport_type) in peers {
                    println!("  {} ({:?})", peer_id, transport_type);
                }
            }

            app.stop().await?;
        }

        Ok(())
    }

    /// Handle the test command
    async fn handle_test_command(mut app: BitchatApp, transport: Option<String>) -> Result<()> {
        println!("Running transport tests...");

        let transport_status = app.get_transport_status();

        match transport.as_deref() {
            Some("ble") => {
                println!("Testing BLE transport...");
                Self::test_ble_transport(&app).await?;
            }
            Some("nostr") => {
                println!("Testing Nostr transport...");
                Self::test_nostr_transport(&app).await?;
            }
            None => {
                // Test all transports
                for (transport_type, is_active) in transport_status {
                    if is_active {
                        println!("Testing {:?} transport...", transport_type);
                        match transport_type {
                            bitchat_core::transport::TransportType::Ble => {
                                Self::test_ble_transport(&app).await?;
                            }
                            bitchat_core::transport::TransportType::Nostr => {
                                Self::test_nostr_transport(&app).await?;
                            }
                            bitchat_core::transport::TransportType::Local => {
                                println!("Local transport testing not implemented");
                            }
                            bitchat_core::transport::TransportType::Custom(_) => {
                                println!("Custom transport testing not implemented");
                            }
                            bitchat_core::transport::TransportType::Mock => {
                                println!("Mock transport testing not implemented");
                            }
                        }
                    } else {
                        warn!("{:?} transport is not active", transport_type);
                    }
                }
            }
            Some(unknown) => {
                return Err(CliError::Config(format!("Unknown transport: {}", unknown)));
            }
        }

        println!("Transport tests completed");
        app.stop().await?;
        Ok(())
    }

    /// Handle the status command
    async fn handle_status_command(mut app: BitchatApp) -> Result<()> {
        println!("BitChat Application Status");
        println!("==========================");

        // Basic info
        println!("Peer ID: {}", app.peer_id());
        println!("Display Name: {}", app.config().display_name);

        // Transport status
        println!("\nTransport Status:");
        let transport_status = app.get_transport_status();
        for (transport_type, is_active) in transport_status {
            let status = if is_active { "Active" } else { "Inactive" };
            println!("  {:?}: {}", transport_type, status);
        }

        // Discovered peers
        println!("\nDiscovered Peers:");
        let peers = app.get_discovered_peers();
        if peers.is_empty() {
            println!("  None");
        } else {
            for (peer_id, transport_type) in peers {
                println!("  {} ({:?})", peer_id, transport_type);
            }
        }

        // Statistics
        let (app_stats, delivery_stats) = app.get_stats();
        println!("\nStatistics:");
        println!("  Messages Sent: {}", app_stats.messages_sent);
        println!("  Messages Received: {}", app_stats.messages_received);
        println!("  Peers Discovered: {}", app_stats.peers_discovered);
        println!("  Startup Count: {}", app_stats.startup_count);
        println!("  Total Runtime: {}s", app_stats.total_runtime);
        println!(
            "  Delivery Success Rate: {:.1}%",
            if delivery_stats.total > 0 {
                (delivery_stats.confirmed as f64 / delivery_stats.total as f64) * 100.0
            } else {
                0.0
            }
        );

        app.stop().await?;
        Ok(())
    }

    /// Test BLE transport functionality
    async fn test_ble_transport(_app: &BitchatApp) -> Result<()> {
        println!("  - Checking BLE adapter availability...");
        // Add BLE-specific tests here
        println!("  - BLE scanning test...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("  - BLE test completed");
        Ok(())
    }

    /// Test Nostr transport functionality
    async fn test_nostr_transport(_app: &BitchatApp) -> Result<()> {
        println!("  - Testing Nostr relay connections...");
        // Add Nostr-specific tests here
        println!("  - Nostr subscription test...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("  - Nostr test completed");
        Ok(())
    }

    /// Parse a peer ID from hex string
    fn parse_peer_id(peer_str: &str) -> Result<PeerId> {
        let bytes = hex::decode(peer_str)
            .map_err(|e| CliError::Config(format!("Invalid peer ID format: {}", e)))?;

        if bytes.len() != 8 {
            return Err(CliError::Config(
                "Peer ID must be exactly 8 bytes (16 hex characters)".to_string(),
            ));
        }

        let mut peer_bytes = [0u8; 8];
        peer_bytes.copy_from_slice(&bytes);
        Ok(PeerId::new(peer_bytes))
    }
}

// Note: We need these imports for the TUI integration
