//! Critical Test Scenarios Module
//!
//! Contains implementations of critical protocol test scenarios
//! that were identified as missing from the original test suite.

pub mod transport_failover;
pub mod session_rekey;
pub mod byzantine_fault;
pub mod panic_recovery;
pub mod mesh_partition;
pub mod file_transfer_resume;
pub mod version_compatibility;
pub mod peer_scaling;

pub use transport_failover::TransportFailoverScenario;
pub use session_rekey::SessionRekeyScenario;
pub use byzantine_fault::ByzantineFaultScenario;
pub use panic_recovery::PanicRecoveryScenario;
pub use mesh_partition::MeshPartitionScenario;
pub use file_transfer_resume::FileTransferResumeScenario;
pub use version_compatibility::VersionCompatibilityScenario;
pub use peer_scaling::PeerScalingScenario;

/// All critical test scenarios
pub fn all_critical_scenarios() -> Vec<Box<dyn crate::test_runner::TestScenario>> {
    vec![
        Box::new(TransportFailoverScenario),
        Box::new(SessionRekeyScenario), 
        Box::new(ByzantineFaultScenario),
        Box::new(PanicRecoveryScenario),
        Box::new(MeshPartitionScenario),
        Box::new(FileTransferResumeScenario),
        Box::new(VersionCompatibilityScenario),
        Box::new(PeerScalingScenario),
    ]
}

/// High priority security-critical scenarios
pub fn security_critical_scenarios() -> Vec<Box<dyn crate::test_runner::TestScenario>> {
    vec![
        Box::new(ByzantineFaultScenario),
        Box::new(PanicRecoveryScenario),
        Box::new(TransportFailoverScenario),
        Box::new(SessionRekeyScenario),
    ]
}

/// Medium priority robustness scenarios  
pub fn robustness_scenarios() -> Vec<Box<dyn crate::test_runner::TestScenario>> {
    vec![
        Box::new(MeshPartitionScenario),
        Box::new(FileTransferResumeScenario),
        Box::new(VersionCompatibilityScenario),
        Box::new(PeerScalingScenario),
    ]
}