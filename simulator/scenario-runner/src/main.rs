//! BitChat Scenario Runner
//!
//! Event-driven scenario execution for cross-client compatibility testing

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

mod event_orchestrator;

use event_orchestrator::{EventOrchestrator, ClientType};

/// BitChat Scenario Runner
#[derive(Parser)]
#[command(name = "bitchat-scenario-runner")]
#[command(about = "Event-driven scenario runner for BitChat compatibility testing")]
#[command(version)]
struct Cli {
    /// Test command to run
    #[command(subcommand)]
    command: Option<Commands>,

    /// Relay URL to use for testing
    #[arg(long, default_value = "wss://relay.damus.io")]
    relay: String,

    /// Client type to use for testing
    #[arg(long, value_enum)]
    client_type: Option<ClientType>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a specific test scenario
    Scenario {
        /// Name of the scenario to run
        name: String,
    },
    /// List available test scenarios
    List,
    /// Run deterministic messaging test
    DeterministicMessaging,
    /// Run security conformance test
    SecurityConformance,
    /// Run all deterministic scenarios
    AllScenarios,
    /// Run cross-implementation compatibility test (CLI ↔ WASM)
    CrossImplementationTest,
    /// Run all client types compatibility test
    AllClientTypes,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("bitchat_scenario_runner={}", log_level))
        .init();

    info!("Starting BitChat Scenario Runner");
    info!("Relay: {}", cli.relay);

    // Create event orchestrator
    let mut orchestrator = EventOrchestrator::new(cli.relay.clone());

    match cli.command.unwrap_or(Commands::List) {
        Commands::Scenario { name } => {
            info!("Running scenario: {}", name);
            run_scenario(&mut orchestrator, &name, cli.client_type.unwrap_or(ClientType::RustCli)).await?;
        }
        Commands::List => {
            list_scenarios();
        }
        Commands::DeterministicMessaging => {
            info!("Running deterministic messaging test");
            run_deterministic_messaging(&mut orchestrator, cli.client_type.unwrap_or(ClientType::RustCli)).await?;
        }
        Commands::SecurityConformance => {
            info!("Running security conformance test");
            run_security_conformance(&mut orchestrator).await?;
        }
        Commands::AllScenarios => {
            info!("Running all deterministic scenarios");
            run_all_scenarios_deterministic(orchestrator.relay_url().to_string()).await?;
        }
        Commands::CrossImplementationTest => {
            info!("Running cross-implementation compatibility test (CLI ↔ WASM)");
            run_cross_implementation_test(&mut orchestrator).await?;
        }
        Commands::AllClientTypes => {
            info!("Running all client types compatibility test");
            run_all_client_types_test(&mut orchestrator).await?;
        }
    }

    // Clean shutdown
    orchestrator.stop_all_clients().await?;
    info!("Test runner completed");
    Ok(())
}

/// Run a specific test scenario
async fn run_scenario(orchestrator: &mut EventOrchestrator, scenario_name: &str, client_type: ClientType) -> Result<()> {
    match scenario_name {
        "deterministic-messaging" => run_deterministic_messaging(orchestrator, client_type).await,
        "security-conformance" => run_security_conformance(orchestrator).await,
        "transport-failover" => run_transport_failover_with_orchestrator(orchestrator, client_type).await,
        "session-rekey" => run_session_rekey_with_orchestrator(orchestrator, client_type).await,
        "byzantine-fault" => run_byzantine_fault_with_orchestrator(orchestrator, client_type).await,
        "cross-implementation-test" => run_cross_implementation_test(orchestrator).await,
        "all-client-types" => run_all_client_types_test(orchestrator).await,
        _ => {
            anyhow::bail!("Unknown scenario: {}. Available: deterministic-messaging, security-conformance, transport-failover, session-rekey, byzantine-fault, cross-implementation-test, all-client-types", scenario_name);
        }
    }
}

/// Run deterministic messaging test
async fn run_deterministic_messaging(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    info!("Starting deterministic messaging test with {} clients...", client_type.name());

    // Start clients and wait for ready events (NO SLEEP)
    orchestrator.start_client_by_type(client_type, "alice".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "bob".to_string()).await?;

    // Wait for both clients to be ready - deterministic, no timeouts
    orchestrator.wait_for_all_ready().await?;

    // Wait for peer discovery (EVENT-DRIVEN)
    let _discovery_event = orchestrator
        .wait_for_peer_event("alice", "PeerDiscovered", "bob")
        .await?;
    info!("Alice discovered Bob");

    orchestrator
        .wait_for_peer_event("bob", "PeerDiscovered", "alice")
        .await?;
    info!("Bob discovered Alice");

    // Wait for session establishment (EVENT-DRIVEN)
    orchestrator
        .wait_for_peer_event("alice", "SessionEstablished", "bob")
        .await?;
    orchestrator
        .wait_for_peer_event("bob", "SessionEstablished", "alice")
        .await?;
    info!("Bidirectional sessions established");

    // Send message and verify delivery (EVENT-DRIVEN)
    orchestrator.send_command("alice", "send Hello from Alice").await?;
    
    // Wait for Alice's MessageSent event
    let _sent_event = orchestrator
        .wait_for_event("alice", "MessageSent")
        .await?;

    // Wait for Bob's MessageReceived event
    let received_event = orchestrator
        .wait_for_event("bob", "MessageReceived")
        .await?;
    
    // Verify message content matches
    let received_content = received_event.data.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No content in received event"))?;
    
    if received_content != "Hello from Alice" {
        return Err(anyhow::anyhow!(
            "Message content mismatch: expected 'Hello from Alice', got '{}'",
            received_content
        ));
    }

    info!("Message '{}' delivered successfully", received_content);
    info!("Deterministic messaging test completed successfully");
    Ok(())
}

/// Run security conformance test
async fn run_security_conformance(_orchestrator: &mut EventOrchestrator) -> Result<()> {
    info!("Security conformance test - placeholder");
    // TODO: Implement when real clients are integrated
    Ok(())
}

/// Run all scenarios with deterministic orchestration
async fn run_all_scenarios_deterministic(relay_url: String) -> Result<()> {
    info!("Starting comprehensive deterministic test suite");
    
    let scenarios: Vec<(&str, fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>)> = vec![
        ("deterministic-messaging", |url| Box::pin(run_deterministic_messaging_standalone(url))),
        ("transport-failover", |url| Box::pin(run_transport_failover_standalone(url))),
        ("session-rekey", |url| Box::pin(run_session_rekey_standalone(url))),
        ("byzantine-fault", |url| Box::pin(run_byzantine_fault_standalone(url))),
    ];

    for (name, scenario_fn) in scenarios {
        info!("Running scenario: {}", name);
        
        match scenario_fn(relay_url.clone()).await {
            Ok(()) => {
                info!("Scenario '{}' completed successfully", name);
            }
            Err(e) => {
                eprintln!("Scenario '{}' failed: {}", name, e);
                return Err(e);
            }
        }
    }

    info!("All scenarios completed successfully!");
    Ok(())
}

async fn run_deterministic_messaging_standalone(relay_url: String) -> Result<()> {
    let mut orchestrator = EventOrchestrator::new(relay_url);
    run_deterministic_messaging(&mut orchestrator, ClientType::RustCli).await?;
    orchestrator.stop_all_clients().await?;
    Ok(())
}

async fn run_transport_failover_standalone(relay_url: String) -> Result<()> {
    let mut orchestrator = EventOrchestrator::new(relay_url);
    
    // Event-driven transport failover test
    orchestrator.start_rust_client("client_a".to_string()).await?;
    orchestrator.start_rust_client("client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    orchestrator.send_command("client_a", "/send BLE message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    
    // Simulate transport failure and fallback
    orchestrator.send_command("client_a", "/disable-transport ble").await?;
    orchestrator.wait_for_event("client_a", "TransportStatusChanged").await?;
    
    orchestrator.send_command("client_a", "/send Nostr fallback message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    
    orchestrator.stop_all_clients().await?;
    Ok(())
}

async fn run_session_rekey_standalone(relay_url: String) -> Result<()> {
    let mut orchestrator = EventOrchestrator::new(relay_url);
    
    orchestrator.start_rust_client("client_a".to_string()).await?;
    orchestrator.start_rust_client("client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    orchestrator.send_command("client_a", "/configure rekey-threshold 5").await?;
    
    // Send messages to trigger rekey
    for i in 0..10 {
        orchestrator.send_command("client_a", &format!("/send Message {}", i)).await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    }
    
    orchestrator.wait_for_event("client_a", "SessionRekeyed").await?;
    
    orchestrator.stop_all_clients().await?;
    Ok(())
}

async fn run_byzantine_fault_standalone(relay_url: String) -> Result<()> {
    let mut orchestrator = EventOrchestrator::new(relay_url);
    
    orchestrator.start_rust_client("honest_a".to_string()).await?;
    orchestrator.start_rust_client("honest_b".to_string()).await?;
    orchestrator.start_rust_client("malicious".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("honest_a", "PeerDiscovered", "honest_b").await?;
    orchestrator.send_command("honest_a", "/send Legitimate message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    
    // Test malicious behavior
    orchestrator.send_command("malicious", "/inject-corrupted-packets 5").await?;
    orchestrator.send_command("honest_a", "/send Post-attack message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    
    orchestrator.stop_all_clients().await?;
    Ok(())
}

async fn run_transport_failover_with_orchestrator(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    orchestrator.start_client_by_type(client_type, "client_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    orchestrator.send_command("client_a", "/send BLE message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    
    orchestrator.send_command("client_a", "/disable-transport ble").await?;
    orchestrator.wait_for_event("client_a", "TransportStatusChanged").await?;
    
    orchestrator.send_command("client_a", "/send Nostr fallback message").await?;
    orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    
    Ok(())
}

async fn run_session_rekey_with_orchestrator(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    orchestrator.start_client_by_type(client_type, "client_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "client_b".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("client_a", "PeerDiscovered", "client_b").await?;
    orchestrator.send_command("client_a", "/configure rekey-threshold 5").await?;
    
    for i in 0..10 {
        orchestrator.send_command("client_a", &format!("/send Message {}", i)).await?;
        orchestrator.wait_for_event("client_b", "MessageReceived").await?;
    }
    
    orchestrator.wait_for_event("client_a", "SessionRekeyed").await?;
    Ok(())
}

async fn run_byzantine_fault_with_orchestrator(orchestrator: &mut EventOrchestrator, client_type: ClientType) -> Result<()> {
    orchestrator.start_client_by_type(client_type, "honest_a".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "honest_b".to_string()).await?;
    orchestrator.start_client_by_type(client_type, "malicious".to_string()).await?;
    orchestrator.wait_for_all_ready().await?;
    
    orchestrator.wait_for_peer_event("honest_a", "PeerDiscovered", "honest_b").await?;
    orchestrator.send_command("honest_a", "/send Legitimate message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    
    orchestrator.send_command("malicious", "/inject-corrupted-packets 5").await?;
    orchestrator.send_command("honest_a", "/send Post-attack message").await?;
    orchestrator.wait_for_event("honest_b", "MessageReceived").await?;
    
    Ok(())
}


/// Run cross-implementation compatibility test (CLI ↔ WASM)
async fn run_cross_implementation_test(orchestrator: &mut EventOrchestrator) -> Result<()> {
    info!("Starting cross-implementation compatibility test (CLI ↔ WASM)");

    // Start one CLI client and one WASM client
    orchestrator.start_rust_cli_client("cli_alice".to_string()).await?;
    orchestrator.start_wasm_client("wasm_bob".to_string()).await?;

    // Wait for both clients to be ready
    orchestrator.wait_for_all_ready().await?;
    info!("Both CLI and WASM clients are ready");

    // Start discovery on both clients
    orchestrator.send_command("cli_alice", "discover").await?;
    orchestrator.send_command("wasm_bob", "discover").await?;

    // Wait for cross-discovery (CLI discovers WASM and vice versa)
    let _cli_discovers_wasm = orchestrator
        .wait_for_peer_event("cli_alice", "PeerDiscovered", "wasm_bob")
        .await?;
    info!("CLI client discovered WASM client");

    let _wasm_discovers_cli = orchestrator
        .wait_for_peer_event("wasm_bob", "PeerDiscovered", "cli_alice")
        .await?;
    info!("WASM client discovered CLI client");

    // Test bidirectional messaging
    // CLI → WASM
    orchestrator.send_command("cli_alice", "send Hello from CLI to WASM").await?;
    let _cli_sent = orchestrator.wait_for_event("cli_alice", "MessageSent").await?;
    let wasm_received = orchestrator.wait_for_event("wasm_bob", "MessageReceived").await?;
    info!("CLI → WASM message successful");

    // WASM → CLI  
    orchestrator.send_command("wasm_bob", "send Hello from WASM to CLI").await?;
    let _wasm_sent = orchestrator.wait_for_event("wasm_bob", "MessageSent").await?;
    let cli_received = orchestrator.wait_for_event("cli_alice", "MessageReceived").await?;
    info!("WASM → CLI message successful");

    // Verify message contents
    if let Some(content) = wasm_received.data.get("content").and_then(|v| v.as_str()) {
        if content != "Hello from CLI to WASM" {
            return Err(anyhow::anyhow!("Message content mismatch: expected 'Hello from CLI to WASM', got '{}'", content));
        }
    }

    if let Some(content) = cli_received.data.get("content").and_then(|v| v.as_str()) {
        if content != "Hello from WASM to CLI" {
            return Err(anyhow::anyhow!("Message content mismatch: expected 'Hello from WASM to CLI', got '{}'", content));
        }
    }

    info!("Cross-implementation compatibility test completed successfully");
    Ok(())
}

/// Run all client types compatibility test
async fn run_all_client_types_test(orchestrator: &mut EventOrchestrator) -> Result<()> {
    info!("Starting all client types compatibility test");

    // Start clients of each type
    orchestrator.start_client_by_type(ClientType::RustCli, "cli_peer".to_string()).await?;
    orchestrator.start_client_by_type(ClientType::Wasm, "wasm_peer".to_string()).await?;

    // Note: Swift and Kotlin clients would be started here if their implementations
    // support automation mode. For now, we focus on CLI and WASM.
    // orchestrator.start_client_by_type(ClientType::Swift, "swift_peer".to_string()).await?;
    // orchestrator.start_client_by_type(ClientType::Kotlin, "kotlin_peer".to_string()).await?;

    // Wait for all clients to be ready
    orchestrator.wait_for_all_ready().await?;
    info!("All clients are ready");

    // Get clients by type for verification
    let clients_by_type = orchestrator.get_clients_by_type();
    info!("Active client types: {:?}", clients_by_type);

    // Verify we have the expected client types
    assert!(clients_by_type.contains_key(&ClientType::RustCli), "CLI client should be running");
    assert!(clients_by_type.contains_key(&ClientType::Wasm), "WASM client should be running");

    // Start discovery on all clients
    for client_name in orchestrator.running_clients() {
        orchestrator.send_command(&client_name, "discover").await?;
    }

    // Wait for discovery between different client types
    orchestrator.wait_for_peer_event("cli_peer", "PeerDiscovered", "wasm_peer").await?;
    orchestrator.wait_for_peer_event("wasm_peer", "PeerDiscovered", "cli_peer").await?;
    info!("Cross-type peer discovery completed");

    // Test messaging between different implementation types
    orchestrator.send_command("cli_peer", "send Multi-client test message").await?;
    orchestrator.wait_for_event("wasm_peer", "MessageReceived").await?;
    info!("Multi-client messaging successful");

    info!("All client types compatibility test completed successfully");
    Ok(())
}

/// List available test scenarios
fn list_scenarios() {
    println!("Available test scenarios:");
    println!("  deterministic-messaging         - Event-driven messaging without sleep() calls");
    println!("  security-conformance            - Protocol security validation");
    println!("  all-scenarios                   - Run comprehensive deterministic test suite");
    println!("  cross-implementation-test       - CLI ↔ WASM compatibility test");
    println!("  all-client-types               - Test all available client implementations");
}