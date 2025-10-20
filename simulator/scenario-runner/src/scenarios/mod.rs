//! Test scenarios for BitChat cross-client compatibility testing
//!
//! Each scenario is implemented as a separate module to maintain organization
//! and enable easy addition of new test scenarios.

pub mod deterministic_messaging;
pub mod transport_failover;
pub mod transport_commands_test;
pub mod session_management;
pub mod session_rekey;
pub mod byzantine_fault;
pub mod byzantine_validation;
pub mod security_conformance;
pub mod cross_implementation_test;
pub mod all_client_types;
pub mod ios_simulator_test;

// Re-export scenario functions for easy access
pub use deterministic_messaging::run_deterministic_messaging;
pub use transport_failover::run_transport_failover;
pub use transport_commands_test::run_transport_commands_test;
pub use session_management::run_session_management;
pub use session_rekey::run_session_rekey;
pub use byzantine_fault::run_byzantine_fault;
pub use byzantine_validation::run_byzantine_validation;
pub use security_conformance::run_security_conformance;
pub use cross_implementation_test::run_cross_implementation_test;
pub use all_client_types::run_all_client_types_test;
pub use ios_simulator_test::run_ios_simulator_test;