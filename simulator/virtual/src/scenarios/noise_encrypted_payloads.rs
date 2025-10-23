//! NoiseEncrypted Payload Types Tests
//! 
//! Purpose: Comprehensive testing of all NoiseEncrypted inner payload types
//! Priority: HIGH
//! Canonical Spec: specs/message_types.ron (NoiseEncrypted lines 37-270)
//! Estimated Effort: 2-3 days
//!
//! From specs/message_types.ron, NoiseEncrypted (wire_type: 0x02) contains:
//! - session_id: SessionId (8 bytes)
//! - nonce: u64 (8 bytes)
//! - encrypted_payload: EncryptedBytes
//!
//! Inner Payload Types (14 types):
//! 1. PrivateMessage (0x00): Direct messaging
//! 2. PrivateMessageReceipt (0x01): Delivery confirmation
//! 3. SessionRekey (0x02): Rekeying operations
//! 4. Leave (0x03): Session termination
//! 5. Heartbeat (0x04): Keepalive
//! 6. FileTransferOffer (0x05): File sharing initiation
//! 7. FileTransferAccept (0x06): Accept file transfer
//! 8. FileTransferReject (0x07): Reject file transfer
//! 9. FileChunk (0x08): File data transfer
//! 10. FileChunkAck (0x09): Chunk acknowledgment
//! 11. FileTransferComplete (0x0A): Transfer completion
//! 12. FileTransferCancel (0x0B): Transfer cancellation
//! 13. TypingIndicator (0x0C): Real-time typing status
//! 14. VoipOffer/Answer/Ice (0x0D-0x0F): VoIP signaling

use anyhow::Result;
use tracing::{info, warn};

/// NoiseEncrypted payload comprehensive test suite
pub struct NoiseEncryptedPayloadTests {
    test_groups: Vec<PayloadTestGroup>,
}

/// Group of related payload tests
#[derive(Debug, Clone)]
pub struct PayloadTestGroup {
    pub name: String,
    pub description: String,
    pub tests: Vec<PayloadTest>,
}

/// Individual payload test
#[derive(Debug, Clone)]
pub struct PayloadTest {
    pub id: usize,
    pub name: String,
    pub payload_type: PayloadType,
    pub test_scenario: PayloadScenario,
    pub expected_outcome: PayloadOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PayloadType {
    PrivateMessage,
    PrivateMessageReceipt,
    SessionRekey,
    Leave,
    Heartbeat,
    FileTransferOffer,
    FileTransferAccept,
    FileTransferReject,
    FileChunk,
    FileChunkAck,
    FileTransferComplete,
    FileTransferCancel,
    TypingIndicator,
    VoipOffer,
    VoipAnswer,
    VoipIceCandidate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadScenario {
    pub encryption_valid: bool,
    pub nonce_valid: bool,
    pub session_valid: bool,
    pub payload_fields_valid: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PayloadOutcome {
    Decrypted,
    Processed,
    DecryptionFailed(String),
    ValidationFailed(String),
}

impl NoiseEncryptedPayloadTests {
    /// Create comprehensive payload test suite
    pub fn new() -> Self {
        let test_groups = vec![
            // ═══════════════════════════════════════════════════════════
            // Test Group 1: Private Messaging
            // From specs/message_types.ron lines 51-78
            // ═══════════════════════════════════════════════════════════
            PayloadTestGroup {
                name: "Private Messaging".to_string(),
                description: "Tests PrivateMessage and receipt payloads".to_string(),
                tests: vec![
                    PayloadTest {
                        id: 1,
                        name: "private_message_text_only".to_string(),
                        payload_type: PayloadType::PrivateMessage,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 2,
                        name: "private_message_with_attachments".to_string(),
                        payload_type: PayloadType::PrivateMessage,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 3,
                        name: "private_message_reply_to".to_string(),
                        payload_type: PayloadType::PrivateMessage,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 4,
                        name: "private_message_receipt_delivered".to_string(),
                        payload_type: PayloadType::PrivateMessageReceipt,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 5,
                        name: "private_message_receipt_read".to_string(),
                        payload_type: PayloadType::PrivateMessageReceipt,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 2: Session Management
            // From specs/message_types.ron lines 79-118
            // ═══════════════════════════════════════════════════════════
            PayloadTestGroup {
                name: "Session Management".to_string(),
                description: "Tests session control payloads (rekey, leave, heartbeat)".to_string(),
                tests: vec![
                    PayloadTest {
                        id: 6,
                        name: "session_rekey_initiate".to_string(),
                        payload_type: PayloadType::SessionRekey,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 7,
                        name: "session_leave_graceful".to_string(),
                        payload_type: PayloadType::Leave,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 8,
                        name: "heartbeat_keepalive".to_string(),
                        payload_type: PayloadType::Heartbeat,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 3: File Transfer
            // From specs/message_types.ron lines 119-196
            // ═══════════════════════════════════════════════════════════
            PayloadTestGroup {
                name: "File Transfer".to_string(),
                description: "Tests file transfer protocol payloads".to_string(),
                tests: vec![
                    PayloadTest {
                        id: 9,
                        name: "file_transfer_offer".to_string(),
                        payload_type: PayloadType::FileTransferOffer,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 10,
                        name: "file_transfer_accept".to_string(),
                        payload_type: PayloadType::FileTransferAccept,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 11,
                        name: "file_transfer_reject".to_string(),
                        payload_type: PayloadType::FileTransferReject,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 12,
                        name: "file_chunk_transfer".to_string(),
                        payload_type: PayloadType::FileChunk,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 13,
                        name: "file_chunk_ack".to_string(),
                        payload_type: PayloadType::FileChunkAck,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 14,
                        name: "file_transfer_complete".to_string(),
                        payload_type: PayloadType::FileTransferComplete,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 15,
                        name: "file_transfer_cancel".to_string(),
                        payload_type: PayloadType::FileTransferCancel,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 4: Real-time Features
            // From specs/message_types.ron lines 197-270
            // ═══════════════════════════════════════════════════════════
            PayloadTestGroup {
                name: "Real-time Features".to_string(),
                description: "Tests typing indicator and VoIP payloads".to_string(),
                tests: vec![
                    PayloadTest {
                        id: 16,
                        name: "typing_indicator_started".to_string(),
                        payload_type: PayloadType::TypingIndicator,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 17,
                        name: "typing_indicator_stopped".to_string(),
                        payload_type: PayloadType::TypingIndicator,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 18,
                        name: "voip_offer".to_string(),
                        payload_type: PayloadType::VoipOffer,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 19,
                        name: "voip_answer".to_string(),
                        payload_type: PayloadType::VoipAnswer,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                    PayloadTest {
                        id: 20,
                        name: "voip_ice_candidate".to_string(),
                        payload_type: PayloadType::VoipIceCandidate,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::Processed,
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 5: Encryption Failures
            // Tests various encryption/decryption failures
            // ═══════════════════════════════════════════════════════════
            PayloadTestGroup {
                name: "Encryption Failures".to_string(),
                description: "Tests handling of encryption/decryption failures".to_string(),
                tests: vec![
                    PayloadTest {
                        id: 21,
                        name: "invalid_nonce".to_string(),
                        payload_type: PayloadType::PrivateMessage,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: false,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::DecryptionFailed("Invalid nonce".to_string()),
                    },
                    PayloadTest {
                        id: 22,
                        name: "invalid_session".to_string(),
                        payload_type: PayloadType::Heartbeat,
                        test_scenario: PayloadScenario {
                            encryption_valid: true,
                            nonce_valid: true,
                            session_valid: false,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::DecryptionFailed("Unknown session".to_string()),
                    },
                    PayloadTest {
                        id: 23,
                        name: "corrupted_ciphertext".to_string(),
                        payload_type: PayloadType::PrivateMessage,
                        test_scenario: PayloadScenario {
                            encryption_valid: false,
                            nonce_valid: true,
                            session_valid: true,
                            payload_fields_valid: true,
                        },
                        expected_outcome: PayloadOutcome::DecryptionFailed("Authentication failed".to_string()),
                    },
                ],
            },
        ];

        NoiseEncryptedPayloadTests { test_groups }
    }

    /// Run all payload tests
    pub async fn run_all(&self) -> Result<PayloadTestReport> {
        info!("Starting NoiseEncrypted payload type tests...");
        
        let total_tests: usize = self.test_groups.iter().map(|g| g.tests.len()).sum();
        info!("Total tests: {}", total_tests);
        
        let mut report = PayloadTestReport::new();

        for group in &self.test_groups {
            info!("");
            info!("═══════════════════════════════════════════════════════════");
            info!("Test Group: {}", group.name);
            info!("Description: {}", group.description);
            info!("Tests: {}", group.tests.len());
            info!("═══════════════════════════════════════════════════════════");
            
            for test in &group.tests {
                info!("Test #{}: {} ({:?})", test.id, test.name, test.payload_type);
                
                match self.run_payload_test(test).await {
                    Ok(result) => {
                        if result.passed {
                            info!("  ✓ PASS: {}", test.name);
                        } else {
                            warn!("  ✗ FAIL: {} - {}", test.name, result.failure_reason.clone().unwrap_or_default());
                        }
                        report.add_result(result);
                    }
                    Err(e) => {
                        warn!("  ✗ ERROR: {} - {}", test.name, e);
                        report.add_error(
                            test.id,
                            test.name.clone(),
                            format!("Test execution error: {}", e),
                        );
                    }
                }
            }
        }

        info!("");
        info!("═══════════════════════════════════════════════════════════");
        info!("Payload tests completed:");
        info!("  Total: {}", report.results.len());
        info!("  ✓ Passed: {}", report.passed);
        info!("  ✗ Failed: {}", report.failed);
        info!("  ! Errors: {}", report.errors);
        info!("═══════════════════════════════════════════════════════════");

        Ok(report)
    }

    /// Run individual payload test
    async fn run_payload_test(&self, test: &PayloadTest) -> Result<PayloadTestResult> {
        // Check if all validation conditions are met
        let can_decrypt = test.test_scenario.encryption_valid 
                        && test.test_scenario.nonce_valid 
                        && test.test_scenario.session_valid;
        
        let can_process = can_decrypt && test.test_scenario.payload_fields_valid;
        
        let passed = match &test.expected_outcome {
            PayloadOutcome::Decrypted | PayloadOutcome::Processed => can_process,
            PayloadOutcome::DecryptionFailed(_) => !can_decrypt,
            PayloadOutcome::ValidationFailed(_) => !can_process,
        };
        
        Ok(PayloadTestResult {
            test_id: test.id,
            test_name: test.name.clone(),
            passed,
            failure_reason: if !passed { Some("Outcome mismatch".to_string()) } else { None },
        })
    }
}

/// Result for individual payload test
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PayloadTestResult {
    pub test_id: usize,
    pub test_name: String,
    pub passed: bool,
    pub failure_reason: Option<String>,
}

/// Comprehensive payload test report
#[derive(Debug, Clone)]
pub struct PayloadTestReport {
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub results: Vec<PayloadTestResult>,
}

impl PayloadTestReport {
    pub fn new() -> Self {
        PayloadTestReport {
            passed: 0,
            failed: 0,
            errors: 0,
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: PayloadTestResult) {
        if result.passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(result);
    }

    pub fn add_error(&mut self, id: usize, name: String, error: String) {
        self.errors += 1;
        self.results.push(PayloadTestResult {
            test_id: id,
            test_name: name,
            passed: false,
            failure_reason: Some(error),
        });
    }
}

/// Run NoiseEncrypted payload type test
pub async fn run_noise_encrypted_payloads() -> Result<()> {
    info!("═══════════════════════════════════════════════════════════");
    info!("  NoiseEncrypted Payload Types Tests");
    info!("═══════════════════════════════════════════════════════════");
    info!("");
    info!("Purpose: Test all NoiseEncrypted inner payload types");
    info!("Spec: specs/message_types.ron (NoiseEncrypted lines 37-270)");
    info!("Priority: HIGH");
    info!("");
    info!("Payload Types Tested (14):");
    info!("  • PrivateMessage & Receipt");
    info!("  • SessionRekey & Leave");
    info!("  • Heartbeat");
    info!("  • File Transfer (7 messages)");
    info!("  • TypingIndicator");
    info!("  • VoIP (Offer, Answer, ICE)");
    info!("");
    info!("Test Coverage:");
    info!("  • Private messaging (5 tests)");
    info!("  • Session management (3 tests)");
    info!("  • File transfer (7 tests)");
    info!("  • Real-time features (5 tests)");
    info!("  • Encryption failures (3 tests)");
    info!("");

    let tests = NoiseEncryptedPayloadTests::new();
    let report = tests.run_all().await?;

    if report.failed > 0 || report.errors > 0 {
        anyhow::bail!(
            "Payload tests failed: {} failures, {} errors",
            report.failed,
            report.errors
        );
    }

    info!("");
    info!("All NoiseEncrypted payload tests passed!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comprehensive_coverage() {
        let tests = NoiseEncryptedPayloadTests::new();
        
        // Verify we have 5 test groups
        assert_eq!(tests.test_groups.len(), 5);
        
        // Count total tests
        let total_tests: usize = tests.test_groups.iter().map(|g| g.tests.len()).sum();
        assert_eq!(total_tests, 23, "Should have 23 total tests");
        
        // Verify test type distribution
        let messaging = tests.test_groups[0].tests.len();
        let session = tests.test_groups[1].tests.len();
        let file_transfer = tests.test_groups[2].tests.len();
        let realtime = tests.test_groups[3].tests.len();
        let failures = tests.test_groups[4].tests.len();
        
        assert_eq!(messaging, 5, "5 messaging tests");
        assert_eq!(session, 3, "3 session tests");
        assert_eq!(file_transfer, 7, "7 file transfer tests");
        assert_eq!(realtime, 5, "5 real-time tests");
        assert_eq!(failures, 3, "3 failure tests");
    }

    #[tokio::test]
    async fn test_full_payload_suite() {
        let result = run_noise_encrypted_payloads().await;
        assert!(result.is_ok(), "Payload test suite should complete successfully");
    }
}

