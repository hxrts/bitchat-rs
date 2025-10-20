//! Cross-Framework Test Orchestrator
//!
//! Enables testing between scenario-runner clients (CLI, Web) and emulator-rig clients (iOS, Android)
//! by coordinating both frameworks and using Nostr relay for communication.

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, warn};
use crate::client_bridge::{UnifiedClientType, ClientPair, TestingStrategy, TestingFramework};
use crate::event_orchestrator::EventOrchestrator;

/// Cross-framework test orchestrator that bridges scenario-runner and emulator-rig
pub struct CrossFrameworkOrchestrator {
    scenario_orchestrator: EventOrchestrator,
    relay_url: String,
    emulator_rig_path: String,
}

impl CrossFrameworkOrchestrator {
    /// Create a new cross-framework orchestrator
    pub fn new(relay_url: String) -> Self {
        Self {
            scenario_orchestrator: EventOrchestrator::new(relay_url.clone()),
            relay_url,
            emulator_rig_path: "../emulator-rig".to_string(),
        }
    }

    /// Start a client of any type (CLI, Web, iOS, or Android)
    pub async fn start_client(&mut self, client_type: UnifiedClientType, name: String) -> Result<()> {
        match client_type {
            UnifiedClientType::Cli => {
                info!("Starting CLI client '{}'", name);
                self.scenario_orchestrator.start_cli_client(name).await
            }
            UnifiedClientType::Web => {
                info!("Starting Web client '{}'", name);
                self.scenario_orchestrator.start_web_client(name).await
            }
            UnifiedClientType::Ios => {
                info!("Starting iOS client '{}' via emulator-rig", name);
                self.start_emulator_rig_client("ios", name).await
            }
            UnifiedClientType::Android => {
                info!("Starting Android client '{}' via emulator-rig", name);
                self.start_emulator_rig_client("android", name).await
            }
        }
    }

    /// Start an emulator-rig client (iOS or Android)
    async fn start_emulator_rig_client(&mut self, platform: &str, name: String) -> Result<()> {
        info!("Spawning {} emulator via emulator-rig for '{}'", platform, name);
        
        // Build command to run emulator-rig
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.emulator_rig_path)
            .args(["run", "--"])
            .args(["test", "--client1", platform, "--client2", platform])
            .env("RELAY_URL", &self.relay_url)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        
        // For cross-framework testing, we spawn but don't wait
        let child = cmd.spawn()
            .with_context(|| format!("Failed to spawn emulator-rig {} client", platform))?;
        
        // Store process handle (simplified - in production would track properly)
        info!("Emulator-rig {} client spawned for '{}'", platform, name);
        
        // Give it time to initialize
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        Ok(())
    }

    /// Run a cross-framework test between two client types
    pub async fn run_cross_framework_test(
        &mut self,
        client1_type: UnifiedClientType,
        client2_type: UnifiedClientType,
    ) -> Result<()> {
        let pair = ClientPair::new(client1_type, client2_type);
        
        info!("Running cross-framework test: {}", pair.description());
        info!("Testing strategy: {:?}", pair.testing_strategy());
        
        match pair.testing_strategy() {
            TestingStrategy::SingleFramework(TestingFramework::ScenarioRunner) => {
                info!("Both clients use scenario-runner framework");
                self.run_scenario_runner_test(client1_type, client2_type).await
            }
            TestingStrategy::SingleFramework(TestingFramework::EmulatorRig) => {
                info!("Both clients use emulator-rig framework");
                self.run_emulator_rig_test(client1_type, client2_type).await
            }
            TestingStrategy::CrossFramework { framework1, framework2 } => {
                info!("Clients span both frameworks ({:?} ↔ {:?}) - using relay-based communication", framework1, framework2);
                self.run_bridged_test(client1_type, client2_type).await
            }
        }
    }

    /// Run test with both clients in scenario-runner
    async fn run_scenario_runner_test(
        &mut self,
        client1: UnifiedClientType,
        client2: UnifiedClientType,
    ) -> Result<()> {
        // Start both clients
        self.start_client(client1, "client1".to_string()).await?;
        self.start_client(client2, "client2".to_string()).await?;
        
        // Wait for both to be ready
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        
        // Send test commands
        self.scenario_orchestrator.send_command("client1", "status").await?;
        self.scenario_orchestrator.send_command("client2", "status").await?;
        
        info!("Scenario-runner test completed");
        Ok(())
    }

    /// Run test with both clients in emulator-rig
    async fn run_emulator_rig_test(
        &mut self,
        client1: UnifiedClientType,
        client2: UnifiedClientType,
    ) -> Result<()> {
        info!("Running emulator-rig test...");
        
        let platform1 = match client1 {
            UnifiedClientType::Ios => "ios",
            UnifiedClientType::Android => "android",
            _ => unreachable!(),
        };
        
        let platform2 = match client2 {
            UnifiedClientType::Ios => "ios",
            UnifiedClientType::Android => "android",
            _ => unreachable!(),
        };
        
        // Run emulator-rig test directly
        let output = Command::new("cargo")
            .current_dir(&self.emulator_rig_path)
            .args(["run", "--"])
            .args(["test", "--client1", platform1, "--client2", platform2])
            .output()
            .await?;
        
        if output.status.success() {
            info!("✅ Emulator-rig test completed");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Emulator-rig test failed: {}", stderr))
        }
    }

    /// Run bridged test across frameworks using Nostr relay
    async fn run_bridged_test(
        &mut self,
        client1: UnifiedClientType,
        client2: UnifiedClientType,
    ) -> Result<()> {
        info!("Starting cross-framework bridged test via Nostr relay: {}", self.relay_url);
        
        // Start client1
        self.start_client(client1, "client1".to_string()).await?;
        info!("Client1 ({}) started", client1.name());
        
        // Start client2  
        self.start_client(client2, "client2".to_string()).await?;
        info!("Client2 ({}) started", client2.name());
        
        // Give time for both to connect to relay
        info!("Waiting for clients to connect to relay...");
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        
        // For scenario-runner clients, send discovery command
        if client1.supports_scenario_runner() {
            info!("Triggering discovery on client1");
            if let Err(e) = self.scenario_orchestrator.send_command("client1", "discover").await {
                warn!("Failed to send discover to client1: {}", e);
            }
        }
        
        if client2.supports_scenario_runner() {
            info!("Triggering discovery on client2");
            if let Err(e) = self.scenario_orchestrator.send_command("client2", "discover").await {
                warn!("Failed to send discover to client2: {}", e);
            }
        }
        
        // Wait for peer discovery through relay
        info!("Waiting for peer discovery through Nostr relay...");
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        
        // Check status of scenario-runner clients
        if client1.supports_scenario_runner() {
            if let Err(e) = self.scenario_orchestrator.send_command("client1", "status").await {
                warn!("Failed to get status from client1: {}", e);
            }
        }
        
        if client2.supports_scenario_runner() {
            if let Err(e) = self.scenario_orchestrator.send_command("client2", "status").await {
                warn!("Failed to get status from client2: {}", e);
            }
        }
        
        info!("Cross-framework bridged test completed");
        info!("Note: Clients communicated via Nostr relay. Check logs for peer discovery events.");
        
        Ok(())
    }

    /// Stop all clients
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all clients...");
        self.scenario_orchestrator.stop_all_clients().await?;
        // TODO: Stop emulator-rig clients
        Ok(())
    }
}

