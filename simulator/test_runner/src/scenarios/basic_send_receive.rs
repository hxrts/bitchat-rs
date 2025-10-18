//! Basic send/receive test scenario
//!
//! This scenario tests basic message exchange between two BitChat clients:
//! 1. Start two clients (sender and receiver)
//! 2. Have sender send a message to receiver
//! 3. Verify receiver gets the message
//! 4. Have receiver send acknowledgment back
//! 5. Verify sender gets the acknowledgment

use crate::orchestrator::TestOrchestrator;
use anyhow::{Context, Result};
use tracing::{debug, info};

/// Run the basic send/receive test scenario
pub async fn run(orchestrator: &mut TestOrchestrator) -> Result<()> {
    info!("Starting basic send/receive test scenario");

    // Start sender client
    info!("Starting sender client");
    orchestrator
        .start_rust_client("sender")
        .await
        .context("Failed to start sender client")?;

    // Start receiver client
    info!("Starting receiver client");
    orchestrator
        .start_rust_client("receiver")
        .await
        .context("Failed to start receiver client")?;

    // Wait for both clients to initialize and discover each other
    info!("Waiting for clients to initialize and discover peers...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Test message sending from sender to receiver (using any available peer)
    let test_message = "Hello from sender!";
    info!("Sending test message: '{}'", test_message);

    orchestrator
        .send_command("sender", &format!("send any {}", test_message))
        .await
        .context("Failed to send message command to sender")?;

    // Wait for receiver to get the message
    info!("Waiting for receiver to get message...");
    let output = orchestrator
        .wait_for_output("receiver", "Message received from")
        .await
        .context("Receiver did not receive expected message")?;

    debug!("Receiver output: {}", output.line);
    info!("Receiver successfully received message");

    // Test acknowledgment from receiver to sender
    let ack_message = "Message received!";
    info!("Sending acknowledgment: '{}'", ack_message);

    orchestrator
        .send_command("receiver", &format!("send any {}", ack_message))
        .await
        .context("Failed to send acknowledgment command to receiver")?;

    // Wait for sender to get the acknowledgment
    info!("Waiting for sender to get acknowledgment...");
    let ack_output = orchestrator
        .wait_for_output("sender", "Message received from")
        .await
        .context("Sender did not receive expected acknowledgment")?;

    debug!("Sender output: {}", ack_output.line);
    info!("Sender successfully received acknowledgment");

    // Clean up - stop both clients
    info!("Stopping clients...");
    orchestrator
        .stop_client("sender")
        .await
        .context("Failed to stop sender client")?;
    orchestrator
        .stop_client("receiver")
        .await
        .context("Failed to stop receiver client")?;

    info!("Basic send/receive test completed successfully");
    Ok(())
}
