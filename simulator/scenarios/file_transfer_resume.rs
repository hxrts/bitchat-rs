//! Large File Transfer Interruption and Resume Test Scenario
//!
//! Tests multi-GiB file transfer interrupted mid-way and resume capability
//! to verify v2 protocol fragmentation robustness.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::test_runner::{TestResult, TestScenario};
use bitchat_core::{BitchatApp, FileTransferRequest};

pub struct FileTransferResumeScenario;

#[async_trait::async_trait]
impl TestScenario for FileTransferResumeScenario {
    fn name(&self) -> &'static str {
        "file-transfer-interruption-resume"
    }

    async fn run(&self) -> TestResult {
        info!("Starting large file transfer interruption and resume test...");

        // Setup clients with v2 protocol for large file support
        let mut sender = BitchatApp::new_with_version(2).await?;
        let mut receiver = BitchatApp::new_with_version(2).await?;

        sender.start().await?;
        receiver.start().await?;
        sleep(Duration::from_secs(3)).await;

        // Phase 1: Create large test file (10MB for testing, scaled down from GiB)
        info!("Phase 1: Creating large test file");
        
        let test_file_size = 10 * 1024 * 1024; // 10MB
        let test_file_data = generate_test_file_data(test_file_size);
        let file_hash = sha256_hash(&test_file_data);

        let transfer_request = FileTransferRequest {
            filename: "large_test_file.bin".to_string(),
            mime_type: "application/octet-stream".to_string(),
            file_size: test_file_size as u64,
            content: test_file_data.clone(),
        };

        // Phase 2: Start file transfer
        info!("Phase 2: Starting large file transfer");
        
        let transfer_id = sender.start_file_transfer(
            receiver.peer_id(),
            transfer_request.clone()
        ).await?;

        // Monitor transfer progress
        let mut progress_events = sender.subscribe_transfer_progress(transfer_id).await?;
        let mut bytes_transferred = 0;
        let mut interruption_triggered = false;

        // Phase 3: Interrupt transfer at 40% completion
        info!("Phase 3: Monitoring transfer and triggering interruption");
        
        while let Some(progress) = progress_events.recv().await {
            bytes_transferred = progress.bytes_transferred;
            let progress_percent = (bytes_transferred as f64 / test_file_size as f64) * 100.0;
            
            if progress_percent >= 40.0 && !interruption_triggered {
                info!("Interrupting transfer at {:.1}% completion", progress_percent);
                
                // Simulate network interruption
                sender.simulate_network_interruption().await?;
                receiver.simulate_network_interruption().await?;
                
                interruption_triggered = true;
                break;
            }
        }

        assert!(interruption_triggered, "Transfer should have been interrupted");
        assert!(bytes_transferred > 0, "Some bytes should have been transferred");
        assert!(bytes_transferred < test_file_size, "Transfer should be incomplete");

        // Phase 4: Verify transfer is marked as interrupted
        info!("Phase 4: Verifying transfer interruption state");
        
        sleep(Duration::from_secs(2)).await;
        
        let transfer_status = sender.get_transfer_status(transfer_id).await?;
        assert!(transfer_status.is_interrupted(), "Transfer should be marked as interrupted");
        
        let partial_file = receiver.get_partial_transfer(transfer_id).await?;
        assert!(partial_file.is_some(), "Receiver should have partial file data");
        assert_eq!(
            partial_file.unwrap().len(),
            bytes_transferred,
            "Partial file size should match transferred bytes"
        );

        // Phase 5: Restore connectivity and resume transfer
        info!("Phase 5: Restoring connectivity and resuming transfer");
        
        sender.restore_network_connectivity().await?;
        receiver.restore_network_connectivity().await?;
        
        sleep(Duration::from_secs(3)).await; // Allow reconnection

        // Resume transfer
        let resume_result = sender.resume_file_transfer(transfer_id).await?;
        assert!(resume_result.is_ok(), "Transfer resume should succeed");

        // Phase 6: Monitor resumed transfer to completion
        info!("Phase 6: Monitoring resumed transfer to completion");
        
        let mut resumed_progress = sender.subscribe_transfer_progress(transfer_id).await?;
        let mut transfer_completed = false;
        
        while let Some(progress) = resumed_progress.recv().await {
            let progress_percent = (progress.bytes_transferred as f64 / test_file_size as f64) * 100.0;
            
            if progress.is_completed {
                info!("Transfer completed at 100% - {} bytes", progress.bytes_transferred);
                transfer_completed = true;
                break;
            }
            
            // Verify transfer is progressing from where it left off
            assert!(
                progress.bytes_transferred >= bytes_transferred,
                "Resumed transfer should continue from interruption point"
            );
        }

        assert!(transfer_completed, "Transfer should complete after resume");

        // Phase 7: Verify file integrity
        info!("Phase 7: Verifying received file integrity");
        
        let received_file = receiver.get_completed_transfer(transfer_id).await?;
        assert!(received_file.is_some(), "Receiver should have completed file");
        
        let received_data = received_file.unwrap();
        assert_eq!(received_data.len(), test_file_size, "File size should match original");
        
        let received_hash = sha256_hash(&received_data);
        assert_eq!(received_hash, file_hash, "File hash should match original - no corruption");

        // Phase 8: Test multiple interruption scenario
        info!("Phase 8: Testing multiple interruption scenario");
        
        let second_transfer = FileTransferRequest {
            filename: "multi_interrupt_test.bin".to_string(),
            mime_type: "application/octet-stream".to_string(),
            file_size: (test_file_size / 2) as u64,
            content: test_file_data[..test_file_size / 2].to_vec(),
        };

        let transfer_id_2 = sender.start_file_transfer(
            receiver.peer_id(),
            second_transfer
        ).await?;

        // Interrupt multiple times at different progress points
        let interrupt_points = vec![20.0, 60.0, 85.0];
        for interrupt_point in interrupt_points {
            // Monitor and interrupt
            let mut progress = sender.subscribe_transfer_progress(transfer_id_2).await?;
            while let Some(p) = progress.recv().await {
                let percent = (p.bytes_transferred as f64 / (test_file_size / 2) as f64) * 100.0;
                if percent >= interrupt_point {
                    sender.simulate_network_interruption().await?;
                    sleep(Duration::from_millis(500)).await;
                    sender.restore_network_connectivity().await?;
                    sender.resume_file_transfer(transfer_id_2).await?;
                    break;
                }
            }
        }

        info!("File transfer interruption and resume test completed successfully");
        TestResult::Success
    }
}

fn generate_test_file_data(size: usize) -> Vec<u8> {
    // Generate deterministic test data for integrity verification
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push((i % 256) as u8);
    }
    data
}

fn sha256_hash(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}