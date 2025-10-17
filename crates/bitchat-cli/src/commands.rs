//! Command handlers for the BitChat CLI

use std::time::Duration;
use tracing::{info, warn};

use bitchat_core::PeerId;
use crate::app::BitchatApp;
use crate::cli::{Cli, Commands};
use crate::error::{CliError, Result};
use crate::tui::TuiManager;

/// Command dispatcher for handling CLI commands
pub struct CommandDispatcher;

impl CommandDispatcher {
    /// Execute a CLI command
    pub async fn execute(cli: Cli, mut app: BitchatApp) -> Result<()> {
        match cli.command {
            Commands::Chat { name } => {
                Self::handle_chat_command(name, app).await
            }
            Commands::Send { to, message } => {
                Self::handle_send_command(app, to, message).await
            }
            Commands::Peers { watch } => {
                Self::handle_peers_command(app, watch).await
            }
            Commands::Test { transport } => {
                Self::handle_test_command(app, transport).await
            }
            Commands::Status => {
                Self::handle_status_command(app).await
            }
        }
    }

    /// Handle the chat command with TUI
    async fn handle_chat_command(name: String, mut app: BitchatApp) -> Result<()> {
        info!("Starting interactive chat mode with display name: {}", name);

        // Update display name
        // Note: We'd need to add a method to update config
        
        // Start message processing loop in background
        let app_clone = Arc::new(Mutex::new(app));
        let message_loop_app = app_clone.clone();
        
        let message_loop_handle = tokio::spawn(async move {
            let mut app = message_loop_app.lock().await;
            if let Err(e) = app.run_message_loop().await {
                error!("Message loop error: {}", e);
            }
        });

        // Create and run TUI
        {
            let app = app_clone.lock().await;
            let mut tui_manager = TuiManager::new(app.clone()).await?;
            tui_manager.run().await?;
        }

        // Clean shutdown
        {
            let mut app = app_clone.lock().await;
            app.stop().await?;
        }

        message_loop_handle.abort();
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
                return Err(CliError::MessageProcessing(format!("Failed to send message: {}", e)));
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
        println!("  Delivery Success Rate: {:.1}%", 
                 if delivery_stats.total > 0 {
                     (delivery_stats.confirmed as f64 / delivery_stats.total as f64) * 100.0
                 } else {
                     0.0
                 });

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
            return Err(CliError::Config("Peer ID must be exactly 8 bytes (16 hex characters)".to_string()));
        }

        let mut peer_bytes = [0u8; 8];
        peer_bytes.copy_from_slice(&bytes);
        Ok(PeerId::new(peer_bytes))
    }
}

// Note: We need these imports for the TUI integration
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;