//! Test scenarios for BitChat protocol validation
//!
//! This module contains comprehensive test suites that validate specific protocol 
//! components against the canonical BitChat specification. These are kept separate
//! from the unified TOML scenarios as they provide detailed, spec-based validation.

// Specification-based protocol validation test suites
pub mod wire_format_canonical;
pub mod noise_handshake_faults;
pub mod session_state_machine;
pub mod message_fragmentation;
pub mod rekey_trigger_conditions;
pub mod announce_validation;
pub mod session_cleanup;
pub mod noise_encrypted_payloads;
pub mod geohash_identity;
pub mod nostr_nip17;

// Re-export scenario functions for protocol validation
pub use wire_format_canonical::run_wire_format_canonical;
pub use noise_handshake_faults::run_noise_handshake_faults;
pub use session_state_machine::run_session_state_machine;
pub use message_fragmentation::run_message_fragmentation;
pub use rekey_trigger_conditions::run_rekey_trigger_conditions;
pub use announce_validation::run_announce_validation;
pub use session_cleanup::run_session_cleanup;
pub use noise_encrypted_payloads::run_noise_encrypted_payloads;
pub use geohash_identity::run_geohash_identity;
pub use nostr_nip17::run_nostr_nip17;