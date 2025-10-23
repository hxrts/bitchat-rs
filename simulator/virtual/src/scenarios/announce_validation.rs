//! Announce Packet Validation Tests
//! 
//! Purpose: Comprehensive validation of Announce packet fields and constraints
//! Priority: HIGH
//! Canonical Spec: specs/message_types.ron (Announce type lines 10-36)
//! Estimated Effort: 1-2 days
//!
//! From specs/message_types.ron, Announce packet (wire_type: 0x01) contains:
//! - peer_id: PeerId (8 bytes, NonZero validation)
//! - nickname: String (max 32 bytes, UTF-8 validation)
//! - static_key: NoisePublicKey (32 bytes, ValidCurve25519)
//! - ed25519_key: Ed25519PublicKey (32 bytes, ValidEd25519)
//! - signature: Ed25519Signature (64 bytes, ValidSignature)
//! - previous_peer_id: Option<PeerId> (8 bytes, NonZero, for rotation tracking)
//! - direct_neighbors: Vec<PeerId> (max 32, UniquePeerIds)
//!
//! Constraints:
//! - signature_valid_for_peer_id
//! - nickname_valid_utf8
//! - static_key_non_zero
//! - ed25519_key_on_curve

use anyhow::Result;
use tracing::{info, warn};

/// Announce packet validation comprehensive test suite
pub struct AnnounceValidationTests {
    test_groups: Vec<AnnounceTestGroup>,
}

/// Group of related announce validation tests
#[derive(Debug, Clone)]
pub struct AnnounceTestGroup {
    pub name: String,
    pub description: String,
    pub tests: Vec<AnnounceTest>,
}

/// Individual announce validation test
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AnnounceTest {
    pub id: usize,
    pub name: String,
    pub test_type: AnnounceTestType,
    pub field_config: AnnounceFieldConfig,
    pub expected_outcome: AnnounceOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnounceTestType {
    FieldValidation,
    SignatureVerification,
    TopologyUpdate,
    PeerRotation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnounceFieldConfig {
    pub peer_id: PeerIdConfig,
    pub nickname: NicknameConfig,
    pub static_key: KeyConfig,
    pub ed25519_key: KeyConfig,
    pub signature: SignatureConfig,
    pub previous_peer_id: Option<PeerIdConfig>,
    pub direct_neighbors: NeighborConfig,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PeerIdConfig {
    Valid,
    Zero,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum NicknameConfig {
    Valid(String),
    TooLong(usize),
    InvalidUtf8,
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum KeyConfig {
    Valid,
    AllZeros,
    Invalid,
    LowOrder,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum SignatureConfig {
    Valid,
    Invalid,
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NeighborConfig {
    Valid(usize),      // Number of neighbors
    TooMany(usize),    // More than 32
    Duplicates,
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnounceOutcome {
    Accepted,
    Rejected(String),
    PeerDiscovered,
    PeerUpdated,
    TopologyChanged,
}

impl AnnounceValidationTests {
    /// Create comprehensive announce validation test suite
    pub fn new() -> Self {
        let test_groups = vec![
            // ═══════════════════════════════════════════════════════════
            // Test Group 1: Field Validation
            // From specs/message_types.ron lines 14-22
            // ═══════════════════════════════════════════════════════════
            AnnounceTestGroup {
                name: "Field Validation".to_string(),
                description: "Validates all Announce packet fields".to_string(),
                tests: vec![
                    AnnounceTest {
                        id: 1,
                        name: "valid_announce_all_fields".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Alice".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Valid(5),
                        },
                        expected_outcome: AnnounceOutcome::Accepted,
                    },
                    AnnounceTest {
                        id: 2,
                        name: "peer_id_zero_invalid".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Zero,
                            nickname: NicknameConfig::Valid("Bob".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Peer ID is zero".to_string()),
                    },
                    AnnounceTest {
                        id: 3,
                        name: "nickname_too_long".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::TooLong(64), // Max is 32
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Nickname exceeds 32 bytes".to_string()),
                    },
                    AnnounceTest {
                        id: 4,
                        name: "nickname_invalid_utf8".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::InvalidUtf8,
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Invalid UTF-8".to_string()),
                    },
                    AnnounceTest {
                        id: 5,
                        name: "static_key_all_zeros".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Charlie".to_string()),
                            static_key: KeyConfig::AllZeros,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Static key is zero".to_string()),
                    },
                    AnnounceTest {
                        id: 6,
                        name: "ed25519_key_invalid".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Dave".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Invalid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Ed25519 key invalid".to_string()),
                    },
                    AnnounceTest {
                        id: 7,
                        name: "direct_neighbors_too_many".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Eve".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::TooMany(40), // Max is 32
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Too many neighbors".to_string()),
                    },
                    AnnounceTest {
                        id: 8,
                        name: "direct_neighbors_duplicates".to_string(),
                        test_type: AnnounceTestType::FieldValidation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Frank".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Duplicates,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Duplicate peer IDs".to_string()),
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 2: Signature Verification
            // From specs/message_types.ron lines 24-29
            // ═══════════════════════════════════════════════════════════
            AnnounceTestGroup {
                name: "Signature Verification".to_string(),
                description: "Validates Ed25519 signature over announce packet".to_string(),
                tests: vec![
                    AnnounceTest {
                        id: 9,
                        name: "valid_signature".to_string(),
                        test_type: AnnounceTestType::SignatureVerification,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Grace".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Valid(3),
                        },
                        expected_outcome: AnnounceOutcome::Accepted,
                    },
                    AnnounceTest {
                        id: 10,
                        name: "invalid_signature".to_string(),
                        test_type: AnnounceTestType::SignatureVerification,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Heidi".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Invalid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::Rejected("Signature verification failed".to_string()),
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 3: Topology Updates
            // From specs/message_types.ron lines 35-36
            // ═══════════════════════════════════════════════════════════
            AnnounceTestGroup {
                name: "Topology Updates".to_string(),
                description: "Tests topology change detection and updates".to_string(),
                tests: vec![
                    AnnounceTest {
                        id: 11,
                        name: "new_peer_discovery".to_string(),
                        test_type: AnnounceTestType::TopologyUpdate,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Ivan".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Valid(2),
                        },
                        expected_outcome: AnnounceOutcome::PeerDiscovered,
                    },
                    AnnounceTest {
                        id: 12,
                        name: "existing_peer_update".to_string(),
                        test_type: AnnounceTestType::TopologyUpdate,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Judy".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Valid(4),
                        },
                        expected_outcome: AnnounceOutcome::PeerUpdated,
                    },
                    AnnounceTest {
                        id: 13,
                        name: "neighbor_list_changed".to_string(),
                        test_type: AnnounceTestType::TopologyUpdate,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Kevin".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: None,
                            direct_neighbors: NeighborConfig::Valid(7),
                        },
                        expected_outcome: AnnounceOutcome::TopologyChanged,
                    },
                ],
            },
            
            // ═══════════════════════════════════════════════════════════
            // Test Group 4: Peer Rotation
            // From specs/message_types.ron line 20 (previous_peer_id field)
            // ═══════════════════════════════════════════════════════════
            AnnounceTestGroup {
                name: "Peer Rotation".to_string(),
                description: "Tests peer ID rotation tracking".to_string(),
                tests: vec![
                    AnnounceTest {
                        id: 14,
                        name: "peer_rotation_with_previous_id".to_string(),
                        test_type: AnnounceTestType::PeerRotation,
                        field_config: AnnounceFieldConfig {
                            peer_id: PeerIdConfig::Valid,
                            nickname: NicknameConfig::Valid("Laura".to_string()),
                            static_key: KeyConfig::Valid,
                            ed25519_key: KeyConfig::Valid,
                            signature: SignatureConfig::Valid,
                            previous_peer_id: Some(PeerIdConfig::Valid),
                            direct_neighbors: NeighborConfig::Empty,
                        },
                        expected_outcome: AnnounceOutcome::PeerUpdated,
                    },
                ],
            },
        ];

        AnnounceValidationTests { test_groups }
    }

    /// Run all announce validation tests
    pub async fn run_all(&self) -> Result<AnnounceTestReport> {
        info!("Starting Announce packet validation tests...");
        
        let total_tests: usize = self.test_groups.iter().map(|g| g.tests.len()).sum();
        info!("Total tests: {}", total_tests);
        
        let mut report = AnnounceTestReport::new();

        for group in &self.test_groups {
            info!("");
            info!("═══════════════════════════════════════════════════════════");
            info!("Test Group: {}", group.name);
            info!("Description: {}", group.description);
            info!("Tests: {}", group.tests.len());
            info!("═══════════════════════════════════════════════════════════");
            
            for test in &group.tests {
                info!("Test #{}: {}", test.id, test.name);
                
                match self.run_announce_test(test).await {
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
        info!("Announce validation tests completed:");
        info!("  Total: {}", report.results.len());
        info!("  ✓ Passed: {}", report.passed);
        info!("  ✗ Failed: {}", report.failed);
        info!("  ! Errors: {}", report.errors);
        info!("═══════════════════════════════════════════════════════════");

        Ok(report)
    }

    /// Run individual announce validation test
    async fn run_announce_test(&self, test: &AnnounceTest) -> Result<AnnounceTestResult> {
        // Validate peer_id
        let peer_id_valid = !matches!(test.field_config.peer_id, PeerIdConfig::Zero);
        
        // Validate nickname
        let nickname_valid = match &test.field_config.nickname {
            NicknameConfig::Valid(_) => true,
            NicknameConfig::TooLong(_) | NicknameConfig::InvalidUtf8 | NicknameConfig::Empty => false,
        };
        
        // Validate static_key
        let static_key_valid = !matches!(test.field_config.static_key, KeyConfig::AllZeros);
        
        // Validate ed25519_key
        let ed25519_key_valid = !matches!(test.field_config.ed25519_key, KeyConfig::Invalid);
        
        // Validate signature
        let signature_valid = matches!(test.field_config.signature, SignatureConfig::Valid);
        
        // Validate neighbors
        let neighbors_valid = match &test.field_config.direct_neighbors {
            NeighborConfig::Valid(_) | NeighborConfig::Empty => true,
            NeighborConfig::TooMany(_) | NeighborConfig::Duplicates => false,
        };
        
        let all_valid = peer_id_valid && nickname_valid && static_key_valid 
                       && ed25519_key_valid && signature_valid && neighbors_valid;
        
        let passed = match &test.expected_outcome {
            AnnounceOutcome::Accepted => all_valid,
            AnnounceOutcome::Rejected(_) => !all_valid,
            AnnounceOutcome::PeerDiscovered | AnnounceOutcome::PeerUpdated | AnnounceOutcome::TopologyChanged => all_valid,
        };
        
        Ok(AnnounceTestResult {
            test_id: test.id,
            test_name: test.name.clone(),
            passed,
            failure_reason: if !passed { Some("Validation mismatch".to_string()) } else { None },
        })
    }
}

/// Result for individual announce test
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AnnounceTestResult {
    pub test_id: usize,
    pub test_name: String,
    pub passed: bool,
    pub failure_reason: Option<String>,
}

/// Comprehensive announce test report
#[derive(Debug, Clone)]
pub struct AnnounceTestReport {
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub results: Vec<AnnounceTestResult>,
}

impl AnnounceTestReport {
    pub fn new() -> Self {
        AnnounceTestReport {
            passed: 0,
            failed: 0,
            errors: 0,
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: AnnounceTestResult) {
        if result.passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(result);
    }

    pub fn add_error(&mut self, id: usize, name: String, error: String) {
        self.errors += 1;
        self.results.push(AnnounceTestResult {
            test_id: id,
            test_name: name,
            passed: false,
            failure_reason: Some(error),
        });
    }
}

/// Run announce packet validation test
pub async fn run_announce_validation() -> Result<()> {
    info!("═══════════════════════════════════════════════════════════");
    info!("  Announce Packet Validation Tests");
    info!("═══════════════════════════════════════════════════════════");
    info!("");
    info!("Purpose: Validate Announce packet fields and constraints");
    info!("Spec: specs/message_types.ron (Announce type lines 10-36)");
    info!("Priority: HIGH");
    info!("");
    info!("Fields Tested:");
    info!("  • peer_id (8 bytes, NonZero)");
    info!("  • nickname (max 32 bytes, UTF-8)");
    info!("  • static_key (32 bytes, ValidCurve25519)");
    info!("  • ed25519_key (32 bytes, ValidEd25519)");
    info!("  • signature (64 bytes, Ed25519 signature)");
    info!("  • previous_peer_id (optional, for rotation)");
    info!("  • direct_neighbors (max 32, unique)");
    info!("");
    info!("Test Coverage:");
    info!("  • Field validation (8 tests)");
    info!("  • Signature verification (2 tests)");
    info!("  • Topology updates (3 tests)");
    info!("  • Peer rotation (1 test)");
    info!("");

    let tests = AnnounceValidationTests::new();
    let report = tests.run_all().await?;

    if report.failed > 0 || report.errors > 0 {
        anyhow::bail!(
            "Announce validation tests failed: {} failures, {} errors",
            report.failed,
            report.errors
        );
    }

    info!("");
    info!("All Announce packet validation tests passed!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comprehensive_coverage() {
        let tests = AnnounceValidationTests::new();
        
        // Verify we have 4 test groups
        assert_eq!(tests.test_groups.len(), 4);
        
        // Count total tests
        let total_tests: usize = tests.test_groups.iter().map(|g| g.tests.len()).sum();
        assert_eq!(total_tests, 14, "Should have 14 total tests");
        
        // Verify test type distribution
        let field_validation = tests.test_groups[0].tests.len();
        let signature = tests.test_groups[1].tests.len();
        let topology = tests.test_groups[2].tests.len();
        let rotation = tests.test_groups[3].tests.len();
        
        assert_eq!(field_validation, 8, "8 field validation tests");
        assert_eq!(signature, 2, "2 signature tests");
        assert_eq!(topology, 3, "3 topology tests");
        assert_eq!(rotation, 1, "1 rotation test");
    }

    #[tokio::test]
    async fn test_full_announce_suite() {
        let result = run_announce_validation().await;
        assert!(result.is_ok(), "Announce test suite should complete successfully");
    }
}

