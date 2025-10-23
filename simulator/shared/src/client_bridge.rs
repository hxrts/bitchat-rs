//! Universal Client Bridge
//!
//! Provides a unified abstraction layer for launching and communicating with any BitChat client type:
//! - CLI (native Rust)
//! - iOS (simulator)
//! - Android (emulator)
//! - Web (Node.js/browser)
//! - Kotlin (JVM)
//!
//! This allows any client type to be tested against any other client type.

use anyhow::{Result, Context, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;
use std::collections::HashMap;
use tracing::{info, error, warn};

/// Universal client type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UniversalClientType {
    /// Native Rust CLI client
    Cli,
    /// iOS simulator client
    Ios,
    /// Android emulator client
    Android,
    /// Web/Node.js client
    Web,
    /// Kotlin/JVM client
    Kotlin,
}

impl UniversalClientType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Ios => "iOS",
            Self::Android => "Android",
            Self::Web => "Web",
            Self::Kotlin => "Kotlin",
        }
    }

    pub fn supports_stdin_commands(&self) -> bool {
        match self {
            Self::Cli | Self::Web | Self::Kotlin => true,
            Self::Ios | Self::Android => false,
        }
    }

    pub fn requires_emulator(&self) -> bool {
        matches!(self, Self::Ios | Self::Android)
    }
    
    pub fn supports_scenario_runner(&self) -> bool {
        matches!(self, Self::Cli | Self::Web | Self::Kotlin)
    }
}

/// Client command response
#[derive(Debug, Clone)]
pub struct ClientResponse {
    pub client_name: String,
    pub client_type: UniversalClientType,
    pub command: String,
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

/// Universal client trait - all client adapters must implement this
#[async_trait]
pub trait UniversalClient: Send + Sync {
    /// Get the client type
    fn client_type(&self) -> UniversalClientType;
    
    /// Get the client name/identifier
    fn name(&self) -> &str;
    
    /// Launch/start the client
    async fn start(&mut self) -> Result<()>;
    
    /// Send a command to the client
    async fn send_command(&mut self, command: &str) -> Result<()>;
    
    /// Wait for a response from the client (with timeout)
    async fn wait_for_response(&mut self, timeout_secs: u64) -> Result<ClientResponse>;
    
    /// Check if the client is still running/alive
    async fn is_alive(&self) -> bool;
    
    /// Stop/terminate the client
    async fn stop(&mut self) -> Result<()>;
    
    /// Get the relay URL this client is using
    fn relay_url(&self) -> &str;
}

/// CLI Client Adapter
pub struct CliClientAdapter {
    name: String,
    relay_url: String,
    stdin_tx: Option<mpsc::UnboundedSender<String>>,
    event_rx: Option<mpsc::UnboundedReceiver<Value>>,
    process: Option<tokio::process::Child>,
}

impl CliClientAdapter {
    pub fn new(name: String, relay_url: String) -> Self {
        Self {
            name,
            relay_url,
            stdin_tx: None,
            event_rx: None,
            process: None,
        }
    }
}

#[async_trait]
impl UniversalClient for CliClientAdapter {
    fn client_type(&self) -> UniversalClientType {
        UniversalClientType::Cli
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("Starting CLI client '{}'", self.name);
        
        let cmd_args = vec![
            "run", "-p", "bitchat-cli", "--", "interactive",
            "--automation-mode", "--name", &self.name, "--relay", &self.relay_url
        ];
        
        let mut cmd = Command::new("cargo");
        cmd.args(&cmd_args)
            .current_dir("../../")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);
        
        let mut process = cmd.spawn()
            .context("Failed to start CLI client")?;
        
        let stdout = process.stdout.take()
            .context("Failed to get stdout")?;
        let stdin = process.stdin.take()
            .context("Failed to get stdin")?;
        
        // Create communication channels
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel();
        
        // Spawn stdin handler
        let client_name = self.name.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut stdin = stdin;
            while let Some(command) = stdin_rx.recv().await {
                info!("CLI '{}' ← {}", client_name, command);
                if let Err(e) = stdin.write_all(format!("{}\n", command).as_bytes()).await {
                    error!("Failed to write to CLI stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush CLI stdin: {}", e);
                    break;
                }
            }
        });
        
        // Spawn stdout reader
        let client_name = self.name.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                info!("CLI '{}' → {}", client_name, line);
                if line.starts_with("{") || line.starts_with("[") {
                    if let Ok(json) = serde_json::from_str::<Value>(&line) {
                        let _ = event_tx.send(json);
                    }
                }
            }
        });
        
        self.stdin_tx = Some(stdin_tx);
        self.event_rx = Some(event_rx);
        self.process = Some(process);
        
        // Give client time to initialize
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        Ok(())
    }
    
    async fn send_command(&mut self, command: &str) -> Result<()> {
        let stdin_tx = self.stdin_tx.as_ref()
            .ok_or_else(|| anyhow!("Client not started"))?;
        
        stdin_tx.send(command.to_string())
            .map_err(|e| anyhow!("Failed to send command: {}", e))?;
        
        Ok(())
    }
    
    async fn wait_for_response(&mut self, timeout_secs: u64) -> Result<ClientResponse> {
        let event_rx = self.event_rx.as_mut()
            .ok_or_else(|| anyhow!("Client not started"))?;
        
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs));
        tokio::pin!(timeout);
        
        tokio::select! {
            Some(event) = event_rx.recv() => {
                Ok(ClientResponse {
                    client_name: self.name.clone(),
                    client_type: UniversalClientType::Cli,
                    command: "".to_string(),
                    success: true,
                    data: Some(event),
                    error: None,
                })
            }
            _ = &mut timeout => {
                Err(anyhow!("Timeout waiting for response"))
            }
        }
    }
    
    async fn is_alive(&self) -> bool {
        self.process.is_some()
    }
    
    async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping CLI client '{}'", self.name);
            process.kill().await.ok();
        }
        Ok(())
    }
    
    fn relay_url(&self) -> &str {
        &self.relay_url
    }
}

/// iOS Client Adapter
pub struct IosClientAdapter {
    name: String,
    relay_url: String,
    simulator_id: Option<String>,
    bundle_id: String,
}

impl IosClientAdapter {
    pub fn new(name: String, relay_url: String) -> Self {
        Self {
            name,
            relay_url,
            simulator_id: None,
            bundle_id: "chat.bitchat".to_string(),
        }
    }
}

#[async_trait]
impl UniversalClient for IosClientAdapter {
    fn client_type(&self) -> UniversalClientType {
        UniversalClientType::Ios
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("Starting iOS simulator client '{}'", self.name);
        
        // Create simulator
        let device_name = format!("BitChat-{}", self.name);
        let output = Command::new("xcrun")
            .args(["simctl", "create", &device_name, "iPhone 15 Pro"])
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(anyhow!("Failed to create iOS simulator"));
        }
        
        let simulator_id = String::from_utf8(output.stdout)?.trim().to_string();
        info!("Created iOS simulator: {}", simulator_id);
        
        // Boot simulator
        Command::new("xcrun")
            .args(["simctl", "boot", &simulator_id])
            .output()
            .await?;
        
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        // Install app
        let app_path = "../../device/vendored/bitchat-ios/build/BitChat.app";
        Command::new("xcrun")
            .args(["simctl", "install", &simulator_id, app_path])
            .output()
            .await?;
        
        // Launch app
        Command::new("xcrun")
            .args(["simctl", "launch", &simulator_id, &self.bundle_id])
            .output()
            .await?;
        
        self.simulator_id = Some(simulator_id);
        
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        
        Ok(())
    }
    
    async fn send_command(&mut self, command: &str) -> Result<()> {
        // iOS apps don't support direct command stdin
        // Commands would need to be sent via deep links or push notifications
        warn!("iOS client '{}' doesn't support direct commands: {}", self.name, command);
        Ok(())
    }
    
    async fn wait_for_response(&mut self, _timeout_secs: u64) -> Result<ClientResponse> {
        // iOS apps communicate via the BitChat protocol over Nostr
        // Responses would be monitored via relay subscriptions
        warn!("iOS client '{}' uses protocol-level communication", self.name);
        Ok(ClientResponse {
            client_name: self.name.clone(),
            client_type: UniversalClientType::Ios,
            command: "".to_string(),
            success: true,
            data: None,
            error: None,
        })
    }
    
    async fn is_alive(&self) -> bool {
        if let Some(ref simulator_id) = self.simulator_id {
            // Check if app is running
            let output = Command::new("xcrun")
                .args(["simctl", "spawn", simulator_id, "launchctl", "list"])
                .output()
                .await;
            
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains(&self.bundle_id);
            }
        }
        false
    }
    
    async fn stop(&mut self) -> Result<()> {
        if let Some(simulator_id) = self.simulator_id.take() {
            info!("Stopping iOS simulator '{}'", self.name);
            
            // Terminate app
            Command::new("xcrun")
                .args(["simctl", "terminate", &simulator_id, &self.bundle_id])
                .output()
                .await.ok();
            
            // Shutdown simulator
            Command::new("xcrun")
                .args(["simctl", "shutdown", &simulator_id])
                .output()
                .await.ok();
            
            // Delete simulator
            Command::new("xcrun")
                .args(["simctl", "delete", &simulator_id])
                .output()
                .await.ok();
        }
        Ok(())
    }
    
    fn relay_url(&self) -> &str {
        &self.relay_url
    }
}

/// Universal Client Bridge - manages all client types
pub struct UniversalClientBridge {
    relay_url: String,
    clients: HashMap<String, Box<dyn UniversalClient>>,
}

impl UniversalClientBridge {
    pub fn new(relay_url: String) -> Self {
        Self {
            relay_url,
            clients: HashMap::new(),
        }
    }
    
    /// Start a client of any type
    pub async fn start_client(
        &mut self,
        client_type: UniversalClientType,
        name: String,
    ) -> Result<()> {
        info!("Starting {} client '{}'", client_type.name(), name);
        
        let mut client: Box<dyn UniversalClient> = match client_type {
            UniversalClientType::Cli => {
                Box::new(CliClientAdapter::new(name.clone(), self.relay_url.clone()))
            }
            UniversalClientType::Ios => {
                Box::new(IosClientAdapter::new(name.clone(), self.relay_url.clone()))
            }
            UniversalClientType::Android => {
                return Err(anyhow!("Android client adapter not yet implemented"));
            }
            UniversalClientType::Web => {
                return Err(anyhow!("Web client adapter not yet implemented"));
            }
            UniversalClientType::Kotlin => {
                return Err(anyhow!("Kotlin client adapter not yet implemented"));
            }
        };
        
        client.start().await?;
        self.clients.insert(name, client);
        
        Ok(())
    }
    
    /// Send a command to a specific client
    pub async fn send_command(&mut self, client_name: &str, command: &str) -> Result<()> {
        let client = self.clients.get_mut(client_name)
            .ok_or_else(|| anyhow!("Client '{}' not found", client_name))?;
        
        client.send_command(command).await
    }
    
    /// Wait for a response from a specific client
    pub async fn wait_for_response(
        &mut self,
        client_name: &str,
        timeout_secs: u64,
    ) -> Result<ClientResponse> {
        let client = self.clients.get_mut(client_name)
            .ok_or_else(|| anyhow!("Client '{}' not found", client_name))?;
        
        client.wait_for_response(timeout_secs).await
    }
    
    /// Check if a client is still alive
    pub async fn is_client_alive(&self, client_name: &str) -> bool {
        if let Some(client) = self.clients.get(client_name) {
            client.is_alive().await
        } else {
            false
        }
    }
    
    /// Stop a specific client
    pub async fn stop_client(&mut self, client_name: &str) -> Result<()> {
        if let Some(mut client) = self.clients.remove(client_name) {
            client.stop().await?;
        }
        Ok(())
    }
    
    /// Stop all clients
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all clients");
        
        for (name, mut client) in self.clients.drain() {
            info!("Stopping client '{}'", name);
            if let Err(e) = client.stop().await {
                error!("Error stopping client '{}': {}", name, e);
            }
        }
        
        Ok(())
    }
    
    /// Run a basic cross-client test
    pub async fn run_cross_client_test(
        &mut self,
        client1_type: UniversalClientType,
        client2_type: UniversalClientType,
    ) -> Result<()> {
        info!("Running cross-client test: {} ↔ {}", client1_type.name(), client2_type.name());
        
        // Start both clients
        self.start_client(client1_type, "client1".to_string()).await?;
        self.start_client(client2_type, "client2".to_string()).await?;
        
        // Wait for initialization
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        // Send test commands to CLI clients (if applicable)
        if client1_type.supports_stdin_commands() {
            self.send_command("client1", "status").await?;
        }
        if client2_type.supports_stdin_commands() {
            self.send_command("client2", "status").await?;
        }
        
        // Monitor for a period
        info!("Monitoring clients for 30 seconds...");
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        
        // Verify both clients are still alive
        let client1_alive = self.is_client_alive("client1").await;
        let client2_alive = self.is_client_alive("client2").await;
        
        info!("Client1 ({}) alive: {}", client1_type.name(), client1_alive);
        info!("Client2 ({}) alive: {}", client2_type.name(), client2_alive);
        
        if !client1_alive || !client2_alive {
            return Err(anyhow!("One or more clients died during test"));
        }
        
        info!("Cross-client test completed successfully");
        Ok(())
    }
}

// ============================================================================
// Backwards Compatibility Aliases
// ============================================================================

/// Alias for backwards compatibility with existing code
pub type UnifiedClientType = UniversalClientType;

/// Testing framework type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestingFramework {
    ScenarioRunner,
    EmulatorRig,
}

/// Testing strategy for client pairs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestingStrategy {
    SingleFramework(TestingFramework),
    CrossFramework {
        framework1: TestingFramework,
        framework2: TestingFramework,
    },
}

/// Client pair for cross-framework testing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientPair {
    pub client1: UniversalClientType,
    pub client2: UniversalClientType,
}

impl ClientPair {
    pub fn new(client1: UniversalClientType, client2: UniversalClientType) -> Self {
        Self { client1, client2 }
    }
    
    pub fn description(&self) -> String {
        format!("{} ↔ {}", self.client1.name(), self.client2.name())
    }
    
    pub fn testing_strategy(&self) -> TestingStrategy {
        let framework1 = Self::client_framework(self.client1);
        let framework2 = Self::client_framework(self.client2);
        
        if framework1 == framework2 {
            TestingStrategy::SingleFramework(framework1)
        } else {
            TestingStrategy::CrossFramework {
                framework1,
                framework2,
            }
        }
    }
    
    fn client_framework(client_type: UniversalClientType) -> TestingFramework {
        match client_type {
            UniversalClientType::Cli | UniversalClientType::Web | UniversalClientType::Kotlin => {
                TestingFramework::ScenarioRunner
            }
            UniversalClientType::Ios | UniversalClientType::Android => {
                TestingFramework::EmulatorRig
            }
        }
    }
}

/// Client bridge error types
#[derive(Debug, thiserror::Error)]
pub enum ClientTypeBridgeError {
    #[error("Unknown client type: {0}")]
    UnknownClientType(String),
    
    #[error("Client not implemented: {0}")]
    NotImplemented(String),
    
    #[error("Client operation failed: {0}")]
    OperationFailed(String),
}
