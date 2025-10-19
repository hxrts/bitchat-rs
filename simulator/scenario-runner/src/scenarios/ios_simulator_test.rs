//! iOS Simulator compatibility test scenario
//! 
//! Tests BitChat on real iOS apps running in iOS Simulator
//! 
//! This module delegates to the emulator-rig framework which provides
//! comprehensive iOS simulator testing with proper orchestration.

use anyhow::Result;
use std::process::Command;
use tracing::info;

/// Run iOS simulator to iOS simulator communication test
/// 
/// This function calls the emulator-rig framework which implements
/// comprehensive iOS simulator testing with proper error handling,
/// structured logging, and integration with the broader testing infrastructure.
pub async fn run_ios_simulator_test() -> Result<()> {
    info!("Starting iOS Simulator ↔ iOS Simulator communication test");
    info!("Delegating to emulator-rig framework...");

    // Execute the emulator-rig iOS test through nix development environment
    let output = Command::new("nix")
        .args(&["develop", "..", "--command", "cargo", "run", "--", "ios-to-ios"])
        .current_dir("../emulator-rig")
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("iOS simulator test completed successfully");
        
        // Print the output from the test
        if !stdout.is_empty() {
            println!("{}", stdout);
        }
        
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        eprintln!("iOS simulator test failed:");
        if !stdout.is_empty() {
            eprintln!("STDOUT: {}", stdout);
        }
        if !stderr.is_empty() {
            eprintln!("STDERR: {}", stderr);
        }
        
        Err(anyhow::anyhow!("emulator-rig iOS test failed with exit code: {}", 
            output.status.code().unwrap_or(-1)))
    }
}