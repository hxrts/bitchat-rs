//! Test orchestrator for managing BitChat client processes and test execution

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Configuration for a client process
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[allow(dead_code)]
    pub relay_url: String,
}

/// Output from a client process
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClientOutput {
    pub client_name: String,
    pub line: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Test orchestrator that manages client processes and coordinates tests
pub struct TestOrchestrator {
    relay_url: String,
    timeout_duration: Duration,
    bitchat_cli_path: PathBuf,
    clients: HashMap<String, ClientProcess>,
}

struct ClientProcess {
    #[allow(dead_code)]
    config: ClientConfig,
    child: Option<Child>,
    stdin_sender: Option<mpsc::UnboundedSender<String>>,
    stdout_receiver: Option<mpsc::UnboundedReceiver<ClientOutput>>,
}

impl TestOrchestrator {
    /// Create a new test orchestrator
    pub fn new(relay_url: String, timeout_seconds: u64) -> Result<Self> {
        Ok(Self {
            relay_url,
            timeout_duration: Duration::from_secs(timeout_seconds),
            bitchat_cli_path: PathBuf::from("cargo"),
            clients: HashMap::new(),
        })
    }

    /// Set the path to the BitChat CLI binary
    pub fn set_bitchat_cli_path(&mut self, path: PathBuf) {
        self.bitchat_cli_path = path;
    }

    /// Start a Rust BitChat CLI client
    pub async fn start_rust_client(&mut self, client_name: &str) -> Result<()> {
        let config = ClientConfig {
            name: client_name.to_string(),
            command: "cargo".to_string(),
            args: vec![
                "run".to_string(),
                "-p".to_string(),
                "bitchat-cli".to_string(),
                "--no-default-features".to_string(),
                "--features".to_string(),
                "non-interactive".to_string(),
                "--bin".to_string(),
                "bitchat".to_string(),
                "--".to_string(),
                "interactive".to_string(),
                "--name".to_string(),
                client_name.to_string(),
            ],
            relay_url: self.relay_url.clone(),
        };

        self.start_client(config).await
    }

    /// Start a Swift BitChat CLI client
    pub async fn start_swift_client(&mut self, client_name: &str) -> Result<()> {
        let swift_cli_path = PathBuf::from("clients/swift-cli/.build/release/bitchat-swift-cli");

        if !swift_cli_path.exists() {
            anyhow::bail!(
                "Swift CLI not found at {:?}. Run 'just build-swift-cli' first.",
                swift_cli_path
            );
        }

        let config = ClientConfig {
            name: client_name.to_string(),
            command: swift_cli_path.to_string_lossy().to_string(),
            args: vec![
                "--relay".to_string(),
                self.relay_url.clone(),
                "--name".to_string(),
                client_name.to_string(),
            ],
            relay_url: self.relay_url.clone(),
        };

        self.start_client(config).await
    }

    /// Start a Kotlin BitChat CLI client  
    pub async fn start_kotlin_client(&mut self, client_name: &str) -> Result<()> {
        let kotlin_cli_path = PathBuf::from(
            "clients/kotlin-cli/build/install/bitchat-kotlin-cli/bin/bitchat-kotlin-cli",
        );

        if !kotlin_cli_path.exists() {
            anyhow::bail!(
                "Kotlin CLI not found at {:?}. Run 'just build-kotlin-cli' first.",
                kotlin_cli_path
            );
        }

        let config = ClientConfig {
            name: client_name.to_string(),
            command: kotlin_cli_path.to_string_lossy().to_string(),
            args: vec![
                "--relay".to_string(),
                self.relay_url.clone(),
                "--name".to_string(),
                client_name.to_string(),
            ],
            relay_url: self.relay_url.clone(),
        };

        self.start_client(config).await
    }

    /// Start a client process with the given configuration
    async fn start_client(&mut self, config: ClientConfig) -> Result<()> {
        info!("Starting client: {}", config.name);
        debug!("Command: {} {:?}", config.command, config.args);

        // Determine working directory based on client type
        let working_dir = if config.command == "cargo" {
            // We're already running from the project root (via `cargo run` from the root)
            // The test runner binary is in target/release/ but gets executed from the project root
            let current = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            debug!("Current directory: {:?}", current);
            current // Use current directory directly
        } else {
            std::path::PathBuf::from("simulator")
        };

        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start client {}", config.name))?;

        // Set up stdin communication
        let stdin = child.stdin.take().context("Failed to get stdin handle")?;
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        let client_name_for_stdin = config.name.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(command) = stdin_rx.recv().await {
                debug!("Sending command to {}: {}", client_name_for_stdin, command);
                if let Err(e) = stdin.write_all(format!("{}\n", command).as_bytes()).await {
                    error!("Failed to write to {} stdin: {}", client_name_for_stdin, e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush {} stdin: {}", client_name_for_stdin, e);
                    break;
                }
            }
        });

        // Set up stdout monitoring
        let stdout = child.stdout.take().context("Failed to get stdout handle")?;
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel::<ClientOutput>();

        let client_name_for_stdout = config.name.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let output = ClientOutput {
                    client_name: client_name_for_stdout.clone(),
                    line,
                    timestamp: chrono::Utc::now(),
                };

                debug!("Output from {}: {}", output.client_name, output.line);

                if stdout_tx.send(output).is_err() {
                    debug!("Stdout receiver for {} was dropped", client_name_for_stdout);
                    break;
                }
            }
        });

        // Set up stderr monitoring
        let stderr = child.stderr.take().context("Failed to get stderr handle")?;
        let client_name_for_stderr = config.name.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                warn!("Stderr from {}: {}", client_name_for_stderr, line);
            }
        });

        let client_process = ClientProcess {
            config: config.clone(),
            child: Some(child),
            stdin_sender: Some(stdin_tx),
            stdout_receiver: Some(stdout_rx),
        };

        self.clients.insert(config.name.clone(), client_process);

        // Wait a moment for the client to start up
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("Client {} started successfully", config.name);
        Ok(())
    }

    /// Send a command to a client
    pub async fn send_command(&self, client_name: &str, command: &str) -> Result<()> {
        let client = self
            .clients
            .get(client_name)
            .with_context(|| format!("Client {} not found", client_name))?;

        if let Some(ref sender) = client.stdin_sender {
            sender
                .send(command.to_string())
                .with_context(|| format!("Failed to send command to {}", client_name))?;
            debug!("Sent command '{}' to client {}", command, client_name);
        } else {
            anyhow::bail!("Client {} has no stdin sender", client_name);
        }

        Ok(())
    }

    /// Wait for specific output from a client
    pub async fn wait_for_output(
        &mut self,
        client_name: &str,
        expected_pattern: &str,
    ) -> Result<ClientOutput> {
        let client = self
            .clients
            .get_mut(client_name)
            .with_context(|| format!("Client {} not found", client_name))?;

        if let Some(ref mut receiver) = client.stdout_receiver {
            let result = timeout(self.timeout_duration, async {
                while let Some(output) = receiver.recv().await {
                    debug!("Checking output from {}: {}", client_name, output.line);

                    if output.line.contains(expected_pattern) {
                        return Ok(output);
                    }
                }
                anyhow::bail!("Client {} stdout channel closed", client_name)
            })
            .await;

            match result {
                Ok(output) => output,
                Err(_) => anyhow::bail!(
                    "Timeout waiting for pattern '{}' from client {}",
                    expected_pattern,
                    client_name
                ),
            }
        } else {
            anyhow::bail!("Client {} has no stdout receiver", client_name);
        }
    }

    /// Wait for any output from a client within the timeout
    #[allow(dead_code)]
    pub async fn get_next_output(&mut self, client_name: &str) -> Result<Option<ClientOutput>> {
        let client = self
            .clients
            .get_mut(client_name)
            .with_context(|| format!("Client {} not found", client_name))?;

        if let Some(ref mut receiver) = client.stdout_receiver {
            let result = timeout(Duration::from_secs(5), receiver.recv()).await;

            match result {
                Ok(Some(output)) => Ok(Some(output)),
                Ok(None) => Ok(None), // Channel closed
                Err(_) => Ok(None),   // Timeout
            }
        } else {
            Ok(None)
        }
    }

    /// Stop a specific client
    pub async fn stop_client(&mut self, client_name: &str) -> Result<()> {
        if let Some(mut client) = self.clients.remove(client_name) {
            info!("Stopping client: {}", client_name);

            // Drop the stdin sender to signal shutdown
            client.stdin_sender.take();

            if let Some(mut child) = client.child.take() {
                // Try graceful shutdown first
                if let Err(e) = child.kill().await {
                    warn!("Failed to kill client {}: {}", client_name, e);
                }

                // Wait for process to exit
                match child.wait().await {
                    Ok(status) => debug!("Client {} exited with status: {}", client_name, status),
                    Err(e) => warn!("Error waiting for client {} to exit: {}", client_name, e),
                }
            }

            info!("Client {} stopped", client_name);
        }

        Ok(())
    }

    /// Stop all running clients
    pub async fn stop_all_clients(&mut self) -> Result<()> {
        let client_names: Vec<String> = self.clients.keys().cloned().collect();

        for client_name in client_names {
            if let Err(e) = self.stop_client(&client_name).await {
                warn!("Error stopping client {}: {}", client_name, e);
            }
        }

        Ok(())
    }

    /// Get list of running clients
    #[allow(dead_code)]
    pub fn get_running_clients(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Check if a client is running
    #[allow(dead_code)]
    pub fn is_client_running(&self, client_name: &str) -> bool {
        self.clients.contains_key(client_name)
    }
}

impl Drop for TestOrchestrator {
    fn drop(&mut self) {
        // Stop all clients when the orchestrator is dropped
        let clients: Vec<String> = self.clients.keys().cloned().collect();
        for client_name in clients {
            if let Some(mut client) = self.clients.remove(&client_name) {
                if let Some(mut child) = client.child.take() {
                    let _ = child.start_kill();
                }
            }
        }
    }
}
