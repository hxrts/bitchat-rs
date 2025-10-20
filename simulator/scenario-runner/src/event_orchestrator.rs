//! Event-driven test orchestrator with structured client communication
//!
//! Replaces brittle stdout parsing with JSON-based automation events

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use serde_json::Value;
use anyhow::{Context, Result};

// ----------------------------------------------------------------------------
// Client Type Classification
// ----------------------------------------------------------------------------

/// Different types of BitChat client implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum ClientType {
    /// Command-line interface application (built as native target)
    Cli,
    /// Web browser application (built as wasm target)
    Web,
}

impl ClientType {
    /// Get human-readable name for the client type
    pub fn name(&self) -> &'static str {
        match self {
            ClientType::Cli => "CLI Application",
            ClientType::Web => "Web Application",
        }
    }

    /// Get short identifier for the client type
    pub fn identifier(&self) -> &'static str {
        match self {
            ClientType::Cli => "cli",
            ClientType::Web => "web",
        }
    }

}

/// Event-driven test orchestrator
pub struct EventOrchestrator {
    clients: HashMap<String, ClientHandle>,
    event_timeout: Duration,
    relay_url: String,
}

/// Handle for managing a client process
#[allow(dead_code)]
struct ClientHandle {
    name: String,
    client_type: ClientType,
    process: Child,
    event_rx: mpsc::UnboundedReceiver<ClientEvent>,
    stdin_tx: mpsc::UnboundedSender<String>,
}

/// Structured event from client automation mode
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClientEvent {
    pub client_name: String,
    pub client_type: ClientType,
    pub event_type: String,
    pub data: Value,
    pub timestamp: u64,
}

impl EventOrchestrator {
    pub fn new(relay_url: String) -> Self {
        Self {
            clients: HashMap::new(),
            event_timeout: Duration::from_secs(30),
            relay_url,
        }
    }

    /// Start a CLI client in automation mode (compatibility alias)
    #[allow(dead_code)]
    pub async fn start_rust_client(&mut self, name: String) -> Result<()> {
        self.start_cli_client(name).await
    }

    /// Start a CLI client in automation mode
    pub async fn start_cli_client(&mut self, name: String) -> Result<()> {
        let relay_url = self.relay_url.clone();
        let client_name = name.clone();
        
        let cmd_args = vec![
            "run", "-p", "bitchat-cli", "--", "interactive", 
            "--automation-mode", "--name", &client_name, "--relay", &relay_url
        ];
        
        // Set working directory to project root
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args(&cmd_args)
            .current_dir("../../");  // Go to project root
            
        self.start_client_with_command(name, ClientType::Cli, cmd).await
    }

    /// Start a Web client in automation mode (Node.js runner)
    pub async fn start_web_client(&mut self, name: String) -> Result<()> {
        let relay_url = self.relay_url.clone();
        let client_name = name.clone();
        
        // Build the WASM package first
        info!("Building BitChat WASM client...");
        let build_result = tokio::process::Command::new("cargo")
            .args([
                "build", 
                "--target", "wasm32-unknown-unknown",
                "--package", "bitchat-web",
                "--features", "experimental"
            ])
            .current_dir("../../")  // Go to project root
            .output()
            .await
            .context("Failed to run cargo build for WASM")?;
        
        if !build_result.status.success() {
            let stderr = String::from_utf8_lossy(&build_result.stderr);
            anyhow::bail!("WASM build failed: {}", stderr);
        }
        
        // Use wasm-pack to generate JS bindings
        info!("Generating WASM bindings...");
        let pack_result = tokio::process::Command::new("wasm-pack")
            .args([
                "build",
                "../../crates/bitchat-web",
                "--target", "nodejs",
                "--out-dir", "../../simulator/wasm-pkg"
            ])
            .output()
            .await
            .context("Failed to run wasm-pack")?;
        
        if !pack_result.status.success() {
            let stderr = String::from_utf8_lossy(&pack_result.stderr);
            anyhow::bail!("wasm-pack failed: {}", stderr);
        }
        
        // Run the Node.js wrapper with the generated WASM
        self.start_client(
            name, 
            ClientType::Web,
            "node", 
            &["../wasm-runner.js", "--automation-mode", "--name", &client_name, "--relay", &relay_url]
        ).await
    }


    /// Start a client process with a pre-configured Command
    async fn start_client_with_command(&mut self, name: String, client_type: ClientType, mut cmd: tokio::process::Command) -> Result<()> {
        info!("Starting {} client '{}' with automation mode", client_type.name(), name);

        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        let mut process = cmd.spawn()
            .with_context(|| format!("Failed to start client '{}'", name))?;

        let stdout = process.stdout.take()
            .context("Failed to get stdout")?;
        
        let stdin = process.stdin.take()
            .context("Failed to get stdin")?;

        // Create channels for event communication
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel();

        // Spawn stdin handler
        let client_name_stdin = name.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(command) = stdin_rx.recv().await {
                eprintln!("Orchestrator sending command '{}' to client '{}'", command, client_name_stdin);
                if let Err(e) = stdin.write_all(format!("{}\n", command).as_bytes()).await {
                    error!("Failed to write to '{}' stdin: {}", client_name_stdin, e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush '{}' stdin: {}", client_name_stdin, e);
                    break;
                }
            }
        });

        // Spawn event parsing task
        let client_name_events = name.clone();
        let client_type_events = client_type;
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                // Try to parse as JSON automation event
                if let Ok(json) = serde_json::from_str::<Value>(&line) {
                    if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
                        let timestamp = json.get("data")
                            .and_then(|data| data.get("timestamp"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or_else(|| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64
                            });

                        let event = ClientEvent {
                            client_name: client_name_events.clone(),
                            client_type: client_type_events,
                            event_type: event_type.to_string(),
                            data: json.clone(),
                            timestamp,
                        };

                        info!("Received event '{}' from client '{}': {}", event_type, client_name_events, json);

                        if event_tx.send(event).is_err() {
                            break; // Receiver dropped
                        }
                    }
                } else {
                    // Non-JSON output (logs, errors) - log but don't parse
                    debug!("Client '{}' log: {}", client_name_events, line);
                }
            }
        });

        let handle = ClientHandle {
            name: name.clone(),
            client_type,
            process,
            event_rx,
            stdin_tx,
        };

        self.clients.insert(name, handle);
        Ok(())
    }

    /// Start a client process with automation mode enabled
    async fn start_client(&mut self, name: String, client_type: ClientType, command: &str, args: &[&str]) -> Result<()> {
        info!("Starting {} client '{}' with automation mode", client_type.name(), name);

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        let mut process = cmd.spawn()
            .with_context(|| format!("Failed to start client '{}'", name))?;

        let stdout = process.stdout.take()
            .context("Failed to get stdout")?;
        
        let stdin = process.stdin.take()
            .context("Failed to get stdin")?;

        // Create channels for event communication
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel();

        // Spawn stdin handler
        let client_name_stdin = name.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(command) = stdin_rx.recv().await {
                eprintln!("Orchestrator sending command '{}' to client '{}'", command, client_name_stdin);
                if let Err(e) = stdin.write_all(format!("{}\n", command).as_bytes()).await {
                    error!("Failed to write to '{}' stdin: {}", client_name_stdin, e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush '{}' stdin: {}", client_name_stdin, e);
                    break;
                }
            }
        });

        // Spawn event parsing task
        let client_name_events = name.clone();
        let client_type_events = client_type;
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                // Try to parse as JSON automation event
                if let Ok(json) = serde_json::from_str::<Value>(&line) {
                    if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
                        let timestamp = json.get("data")
                            .and_then(|data| data.get("timestamp"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or_else(|| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64
                            });

                        let event = ClientEvent {
                            client_name: client_name_events.clone(),
                            client_type: client_type_events,
                            event_type: event_type.to_string(),
                            data: json.clone(),
                            timestamp,
                        };

                        info!("Received event '{}' from client '{}': {}", event_type, client_name_events, json);

                        if event_tx.send(event).is_err() {
                            break; // Receiver dropped
                        }
                    }
                } else {
                    // Non-JSON output (logs, errors) - log but don't parse
                    debug!("Client '{}' log: {}", client_name_events, line);
                }
            }
        });

        let handle = ClientHandle {
            name: name.clone(),
            client_type,
            process,
            event_rx,
            stdin_tx,
        };

        self.clients.insert(name, handle);
        Ok(())
    }

    /// Wait for a specific event from a client
    pub async fn wait_for_event(&mut self, client_name: &str, event_type: &str) -> Result<ClientEvent> {
        self.wait_for_event_with_condition(client_name, event_type, |_| true).await
    }

    /// Wait for an event with a specific peer_id
    pub async fn wait_for_peer_event(
        &mut self,
        client_name: &str,
        event_type: &str,
        peer_id: &str,
    ) -> Result<ClientEvent> {
        self.wait_for_event_with_condition(client_name, event_type, |data| {
            data.get("data")
                .and_then(|data_field| data_field.get("peer_id"))
                .and_then(|v| v.as_str())
                .map(|id| id == peer_id)
                .unwrap_or(false)
        }).await
    }

    /// Wait for an event matching a condition
    pub async fn wait_for_event_with_condition<F>(
        &mut self,
        client_name: &str,
        event_type: &str,
        condition: F,
    ) -> Result<ClientEvent>
    where
        F: Fn(&Value) -> bool,
    {
        let start = Instant::now();
        let client = self.clients.get_mut(client_name)
            .with_context(|| format!("Client '{}' not found", client_name))?;

        loop {
            match timeout(self.event_timeout, client.event_rx.recv()).await {
                Ok(Some(event)) => {
                    if event.event_type == event_type && condition(&event.data) {
                        info!("Received expected event '{}' from client '{}' in {:?}", 
                              event_type, client_name, start.elapsed());
                        return Ok(event);
                    } else {
                        debug!("Ignoring event '{}' from client '{}' (waiting for '{}')", 
                               event.event_type, client_name, event_type);
                    }
                }
                Ok(None) => {
                    return Err(anyhow::anyhow!("Client '{}' disconnected", client_name));
                }
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Timeout waiting for event '{}' from client '{}' after {:?}",
                        event_type, client_name, self.event_timeout
                    ));
                }
            }
        }
    }

    /// Send command to client via stdin
    pub async fn send_command(&mut self, client_name: &str, command: &str) -> Result<()> {
        let client = self.clients.get_mut(client_name)
            .with_context(|| format!("Client '{}' not found", client_name))?;

        client.stdin_tx.send(command.to_string())
            .with_context(|| format!("Failed to send command to client '{}'", client_name))?;

        info!("Sent command '{}' to client '{}'", command, client_name);
        Ok(())
    }

    /// Wait for all clients to be ready
    pub async fn wait_for_all_ready(&mut self) -> Result<()> {
        let client_names: Vec<String> = self.clients.keys().cloned().collect();
        
        for client_name in client_names {
            self.wait_for_event(&client_name, "Ready").await
                .with_context(|| format!("Client '{}' failed to become ready", client_name))?;
            info!("Client '{}' is ready", client_name);
        }
        
        Ok(())
    }

    /// Verify all clients are fully initialized and responsive
    pub async fn verify_all_clients_responsive(&mut self) -> Result<()> {
        let client_names: Vec<String> = self.clients.keys().cloned().collect();
        
        for client_name in client_names {
            self.verify_client_responsive(&client_name).await
                .with_context(|| format!("Client '{}' failed responsiveness check", client_name))?;
            info!("Client '{}' is responsive", client_name);
        }
        
        Ok(())
    }

    /// Verify a specific client is responsive by sending status command
    pub async fn verify_client_responsive(&mut self, client_name: &str) -> Result<()> {
        // Send status command to verify the client is responsive
        self.send_command(client_name, "status").await?;
        
        // Wait for SystemStatusReport response
        self.wait_for_event(client_name, "SystemStatusReport").await
            .with_context(|| format!("Client '{}' did not respond to status command", client_name))?;
        
        Ok(())
    }

    /// Stop a specific client
    pub async fn stop_client(&mut self, client_name: &str) -> Result<()> {
        if let Some(mut client) = self.clients.remove(client_name) {
            client.process.kill().await
                .with_context(|| format!("Failed to kill client '{}'", client_name))?;
            info!("Stopped client '{}'", client_name);
        }
        Ok(())
    }

    /// Stop all clients
    pub async fn stop_all_clients(&mut self) -> Result<()> {
        let client_names: Vec<String> = self.clients.keys().cloned().collect();
        
        for client_name in client_names {
            if let Err(e) = self.stop_client(&client_name).await {
                warn!("Failed to stop client '{}': {}", client_name, e);
            }
        }
        
        Ok(())
    }

    /// Get list of running clients
    pub fn running_clients(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Set event timeout
    #[allow(dead_code)]
    pub fn set_event_timeout(&mut self, timeout: Duration) {
        self.event_timeout = timeout;
    }

    /// Get relay URL
    #[allow(dead_code)]
    pub fn relay_url(&self) -> &str {
        &self.relay_url
    }

    /// Get client type for a given client name
    #[allow(dead_code)]
    pub fn get_client_type(&self, client_name: &str) -> Option<ClientType> {
        self.clients.get(client_name).map(|handle| handle.client_type)
    }

    /// Get all clients grouped by type
    pub fn get_clients_by_type(&self) -> HashMap<ClientType, Vec<String>> {
        let mut result = HashMap::new();
        for (name, handle) in &self.clients {
            result.entry(handle.client_type).or_insert_with(Vec::new).push(name.clone());
        }
        result
    }


    /// Start a client of any supported type
    pub async fn start_client_by_type(&mut self, client_type: ClientType, name: String) -> Result<()> {
        match client_type {
            ClientType::Cli => self.start_cli_client(name).await,
            ClientType::Web => self.start_web_client(name).await,
        }
    }
}