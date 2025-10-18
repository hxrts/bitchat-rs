//! Cross-client compatibility test scenario
//!
//! This scenario tests message exchange between different BitChat client implementations:
//! 1. Start clients of different types (Rust + Swift, Rust + Kotlin, Swift + Kotlin)  
//! 2. Test handshake establishment between different implementations
//! 3. Test message exchange between different implementations
//! 4. Verify protocol compatibility across all implementations

use crate::orchestrator::TestOrchestrator;
use anyhow::{Context, Result};
use tracing::info;

/// Test combinations of different client implementations
pub async fn run(orchestrator: &mut TestOrchestrator) -> Result<()> {
    info!("Starting cross-client compatibility test scenario");

    // Test Rust <-> Swift compatibility
    info!("Testing Rust <-> Swift compatibility...");
    test_client_pair(orchestrator, "rust", "swift")
        .await
        .context("Rust <-> Swift compatibility test failed")?;

    // Clean up before next test
    orchestrator.stop_all_clients().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Test Rust <-> Kotlin compatibility
    info!("Testing Rust <-> Kotlin compatibility...");
    test_client_pair(orchestrator, "rust", "kotlin")
        .await
        .context("Rust <-> Kotlin compatibility test failed")?;

    // Clean up before next test
    orchestrator.stop_all_clients().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Test Swift <-> Kotlin compatibility
    info!("Testing Swift <-> Kotlin compatibility...");
    test_client_pair(orchestrator, "swift", "kotlin")
        .await
        .context("Swift <-> Kotlin compatibility test failed")?;

    info!("Cross-client compatibility test completed successfully");
    Ok(())
}

/// Test compatibility between two specific client implementations
async fn test_client_pair(
    orchestrator: &mut TestOrchestrator,
    client1_type: &str,
    client2_type: &str,
) -> Result<()> {
    let client1_name = format!("{}-client", client1_type);
    let client2_name = format!("{}-client", client2_type);

    info!("Starting {} and {} clients", client1_type, client2_type);

    // Start first client
    start_client_by_type(orchestrator, client1_type, &client1_name)
        .await
        .with_context(|| format!("Failed to start {} client", client1_type))?;

    // Start second client
    start_client_by_type(orchestrator, client2_type, &client2_name)
        .await
        .with_context(|| format!("Failed to start {} client", client2_type))?;

    // Wait for both clients to initialize
    info!("Waiting for clients to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Test handshake between clients
    info!(
        "Testing handshake between {} and {}",
        client1_type, client2_type
    );
    orchestrator
        .send_command(&client1_name, &format!("connect {}", client2_name))
        .await
        .context("Failed to initiate handshake")?;

    // Wait for handshake completion (with relaxed pattern matching)
    let handshake_patterns = [
        "Handshake complete",
        "Connected to",
        "connection established",
    ];
    let mut handshake_success = false;

    for pattern in &handshake_patterns {
        if orchestrator.wait_for_output(&client1_name, pattern).await.is_ok() {
            handshake_success = true;
            break;
        }
        if orchestrator.wait_for_output(&client2_name, pattern).await.is_ok() {
            handshake_success = true;
            break;
        }
    }

    if !handshake_success {
        anyhow::bail!(
            "Handshake between {} and {} did not complete",
            client1_type,
            client2_type
        );
    }

    info!(
        "Handshake completed between {} and {}",
        client1_type, client2_type
    );

    // Test message exchange
    let test_message = format!("Hello from {} to {}!", client1_type, client2_type);
    info!("Testing message exchange: '{}'", test_message);

    orchestrator
        .send_command(
            &client1_name,
            &format!("send {} {}", client2_name, test_message),
        )
        .await
        .context("Failed to send test message")?;

    // Wait for message reception with relaxed pattern matching
    let message_patterns = [&test_message, "Message from", "received"];
    let mut message_success = false;

    for pattern in &message_patterns {
        if orchestrator.wait_for_output(&client2_name, pattern).await.is_ok() {
            message_success = true;
            break;
        }
    }

    if !message_success {
        anyhow::bail!(
            "Message not received by {} from {}",
            client2_type,
            client1_type
        );
    }

    info!(
        "Message successfully exchanged between {} and {}",
        client1_type, client2_type
    );

    // Test reverse message
    let response_message = format!("Response from {} to {}!", client2_type, client1_type);
    info!("Testing reverse message: '{}'", response_message);

    orchestrator
        .send_command(
            &client2_name,
            &format!("send {} {}", client1_name, response_message),
        )
        .await
        .context("Failed to send response message")?;

    // Wait for response reception
    let response_patterns = [&response_message, "Message from", "received"];
    let mut response_success = false;

    for pattern in &response_patterns {
        if orchestrator.wait_for_output(&client1_name, pattern).await.is_ok() {
            response_success = true;
            break;
        }
    }

    if !response_success {
        anyhow::bail!(
            "Response message not received by {} from {}",
            client1_type,
            client2_type
        );
    }

    info!(
        "Bidirectional messaging verified between {} and {}",
        client1_type, client2_type
    );

    // Stop both clients
    orchestrator
        .stop_client(&client1_name)
        .await
        .context("Failed to stop first client")?;
    orchestrator
        .stop_client(&client2_name)
        .await
        .context("Failed to stop second client")?;

    Ok(())
}

/// Start a client by type (rust, swift, or kotlin)
async fn start_client_by_type(
    orchestrator: &mut TestOrchestrator,
    client_type: &str,
    client_name: &str,
) -> Result<()> {
    match client_type {
        "rust" => orchestrator.start_rust_client(client_name).await,
        "swift" => orchestrator.start_swift_client(client_name).await,
        "kotlin" => orchestrator.start_kotlin_client(client_name).await,
        _ => anyhow::bail!("Unknown client type: {}", client_type),
    }
}
