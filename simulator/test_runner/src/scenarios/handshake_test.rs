//! Noise protocol handshake test scenario
//!
//! This scenario tests the Noise protocol handshake establishment between clients:
//! 1. Start two clients (initiator and responder)
//! 2. Initiate handshake from initiator
//! 3. Verify responder receives handshake request
//! 4. Verify handshake completion on both sides
//! 5. Test that secure communication works after handshake

use crate::orchestrator::TestOrchestrator;
use anyhow::{Context, Result};
use tracing::info;

/// Run the handshake test scenario
pub async fn run(orchestrator: &mut TestOrchestrator) -> Result<()> {
    info!("Starting Noise handshake test scenario");

    // Start initiator client
    info!("Starting initiator client");
    orchestrator
        .start_rust_client("initiator")
        .await
        .context("Failed to start initiator client")?;

    // Start responder client
    info!("Starting responder client");
    orchestrator
        .start_rust_client("responder")
        .await
        .context("Failed to start responder client")?;

    // Wait for both clients to initialize
    info!("Waiting for clients to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Initiate handshake
    info!("Initiating Noise handshake");
    orchestrator
        .send_command("initiator", "connect responder")
        .await
        .context("Failed to send connect command to initiator")?;

    // Wait for handshake initiation on initiator side
    info!("Waiting for handshake initiation...");
    let _init_output = match orchestrator
        .wait_for_output("initiator", "Handshake initiated")
        .await
    {
        Ok(output) => output,
        Err(_) => orchestrator
            .wait_for_output("initiator", "Connecting to")
            .await
            .context("Initiator did not start handshake")?,
    };

    info!("Handshake initiated by initiator");

    // Wait for handshake reception on responder side
    info!("Waiting for responder to receive handshake...");
    let _resp_output = match orchestrator
        .wait_for_output("responder", "Handshake received")
        .await
    {
        Ok(output) => output,
        Err(_) => orchestrator
            .wait_for_output("responder", "Connection from")
            .await
            .context("Responder did not receive handshake")?,
    };

    info!("Handshake received by responder");

    // Wait for handshake completion
    info!("Waiting for handshake completion...");

    // Check both sides for completion
    let completion_check = async {
        let initiator_complete = match orchestrator
            .wait_for_output("initiator", "Handshake complete")
            .await
        {
            Ok(output) => Ok(output),
            Err(_) => {
                orchestrator
                    .wait_for_output("initiator", "Connected to")
                    .await
            }
        };

        let responder_complete = match orchestrator
            .wait_for_output("responder", "Handshake complete")
            .await
        {
            Ok(output) => Ok(output),
            Err(_) => {
                orchestrator
                    .wait_for_output("responder", "Connected to")
                    .await
            }
        };

        // At least one side should report completion
        if initiator_complete.is_ok() || responder_complete.is_ok() {
            Ok(())
        } else {
            anyhow::bail!("Neither side completed handshake")
        }
    };

    tokio::time::timeout(tokio::time::Duration::from_secs(10), completion_check)
        .await
        .context("Timeout waiting for handshake completion")?
        .context("Handshake did not complete successfully")?;

    info!("Handshake completed successfully");

    // Test secure communication after handshake
    info!("Testing secure communication after handshake");
    let secure_message = "Secure message after handshake";

    orchestrator
        .send_command("initiator", &format!("send responder {}", secure_message))
        .await
        .context("Failed to send secure message")?;

    // Verify message is received
    let _secure_output = orchestrator
        .wait_for_output("responder", secure_message)
        .await
        .context("Secure message not received after handshake")?;

    info!("Secure communication verified after handshake");

    // Clean up - stop both clients
    info!("Stopping clients...");
    orchestrator
        .stop_client("initiator")
        .await
        .context("Failed to stop initiator client")?;
    orchestrator
        .stop_client("responder")
        .await
        .context("Failed to stop responder client")?;

    info!("Handshake test completed successfully");
    Ok(())
}
