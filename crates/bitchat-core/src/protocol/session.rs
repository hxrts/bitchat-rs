//! Session management types for the BitChat protocol
//!
//! This module provides types for session management including session states
//! and individual session handling. The stateful NoiseSessionManager has been
//! moved to the bitchat-runtime crate.

use alloc::vec::Vec;
use core::time::Duration;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "std"))] {
        use alloc::string::ToString;
        use alloc::format;
    }
}

use crate::protocol::crypto::{NoiseHandshake, NoiseKeyPair, NoiseTransport};
use crate::types::{Fingerprint, PeerId, TimeSource, Timestamp};
use crate::{internal::SessionError, BitchatError, Result};

// ----------------------------------------------------------------------------
// Session State
// ----------------------------------------------------------------------------

/// Session states in the connection lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Performing Noise handshake
    Handshaking,
    /// Handshake complete, ready for encrypted communication
    Established,
    /// Session is being rekeyed
    Rekeying,
    /// Session failed or terminated
    Failed,
}

// ----------------------------------------------------------------------------
// Noise Session
// ----------------------------------------------------------------------------

/// A single Noise protocol session with a peer
#[derive(Debug)]
pub struct NoiseSession {
    /// Peer identifier
    peer_id: PeerId,
    /// Peer's static public key fingerprint
    peer_fingerprint: Option<Fingerprint>,
    /// Current session state
    state: SessionState,
    /// Noise handshake state (during handshaking)
    handshake: Option<NoiseHandshake>,
    /// Noise transport state (when established)
    transport: Option<NoiseTransport>,
    /// Session creation timestamp
    created_at: Timestamp,
    /// Last activity timestamp
    last_activity: Timestamp,
    /// Message count for this session
    message_count: u64,
    /// Rekey threshold (number of messages before automatic rekey)
    rekey_threshold: u64,
    /// Time-based rekey interval in seconds
    rekey_interval_secs: u64,
    /// Timestamp of last rekey operation
    last_rekey: Timestamp,
}

impl NoiseSession {
    /// Create a new outbound session (initiator)
    pub fn new_outbound<T: TimeSource>(
        peer_id: PeerId,
        local_key: &NoiseKeyPair,
        time_source: &T,
    ) -> Result<Self> {
        let handshake = NoiseHandshake::initiator(local_key)?;
        let now = time_source.now();

        Ok(Self {
            peer_id,
            peer_fingerprint: None,
            state: SessionState::Handshaking,
            handshake: Some(handshake),
            transport: None,
            created_at: now,
            last_activity: now,
            message_count: 0,
            rekey_threshold: 1_000_000_000, // Default: rekey after 1 billion messages (matches canonical spec)
            rekey_interval_secs: 86400, // Default: rekey after 24 hours (matches canonical spec)
            last_rekey: now,
        })
    }

    /// Create a new inbound session (responder)
    pub fn new_inbound<T: TimeSource>(
        peer_id: PeerId,
        local_key: &NoiseKeyPair,
        time_source: &T,
    ) -> Result<Self> {
        let handshake = NoiseHandshake::responder(local_key)?;
        let now = time_source.now();

        Ok(Self {
            peer_id,
            peer_fingerprint: None,
            state: SessionState::Handshaking,
            handshake: Some(handshake),
            transport: None,
            created_at: now,
            last_activity: now,
            message_count: 0,
            rekey_threshold: 1_000_000_000, // Default: rekey after 1 billion messages (matches canonical spec)
            rekey_interval_secs: 86400, // Default: rekey after 24 hours (matches canonical spec)
            last_rekey: now,
        })
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Get peer fingerprint (available after handshake)
    pub fn peer_fingerprint(&self) -> Option<&Fingerprint> {
        self.peer_fingerprint.as_ref()
    }

    /// Get current session state
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Check if session is established
    pub fn is_established(&self) -> bool {
        self.state == SessionState::Established
    }

    /// Check if session failed
    pub fn is_failed(&self) -> bool {
        self.state == SessionState::Failed
    }

    /// Get session creation timestamp
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Process handshake message
    pub fn process_handshake_message<T: TimeSource>(
        &mut self,
        input: &[u8],
        time_source: &T,
    ) -> Result<Option<Vec<u8>>> {
        if self.state != SessionState::Handshaking {
            return Err(BitchatError::InvalidPacket(
                "Not in handshaking state".into(),
            ));
        }

        let handshake = self.handshake.as_mut().ok_or_else(|| {
            BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Handshaking".to_string(),
                actual: "No handshake state".to_string(),
            })
        })?;

        let output = handshake.read_message(input)?;

        // Check if handshake is complete
        let is_finished = handshake.is_handshake_finished();
        let remote_static = if is_finished {
            handshake.get_remote_static()
        } else {
            None
        };

        if is_finished {
            // Extract peer's static key and generate fingerprint
            if let Some(remote_static) = remote_static {
                use crate::protocol::crypto::generate_fingerprint;
                self.peer_fingerprint = Some(generate_fingerprint(remote_static));
            }

            // Convert to transport mode
            let handshake = self.handshake.take().ok_or_else(|| {
                BitchatError::Session(SessionError::InvalidState {
                    peer_id: self.peer_id.to_string(),
                    expected: "Handshaking with available handshake".to_string(),
                    actual: "No handshake available for transport conversion".to_string(),
                })
            })?;
            self.transport = Some(handshake.into_transport_mode()?);
            self.state = SessionState::Established;
        }

        self.update_activity(time_source);

        Ok(if output.is_empty() {
            None
        } else {
            Some(output)
        })
    }

    /// Create handshake message
    pub fn create_handshake_message<T: TimeSource>(
        &mut self,
        payload: &[u8],
        time_source: &T,
    ) -> Result<Vec<u8>> {
        if self.state != SessionState::Handshaking {
            return Err(BitchatError::InvalidPacket(
                "Not in handshaking state".into(),
            ));
        }

        let handshake = self.handshake.as_mut().ok_or_else(|| {
            BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Handshaking".to_string(),
                actual: "No handshake state".to_string(),
            })
        })?;

        let output = handshake.write_message(payload)?;

        // Check if handshake is complete after writing
        let is_finished = handshake.is_handshake_finished();
        let remote_static = if is_finished {
            handshake.get_remote_static()
        } else {
            None
        };

        if is_finished {
            // Extract peer's static key and generate fingerprint
            if let Some(remote_static) = remote_static {
                use crate::protocol::crypto::generate_fingerprint;
                self.peer_fingerprint = Some(generate_fingerprint(remote_static));
            }

            // Convert to transport mode
            let handshake = self.handshake.take().ok_or_else(|| {
                BitchatError::Session(SessionError::InvalidState {
                    peer_id: self.peer_id.to_string(),
                    expected: "Handshaking with available handshake".to_string(),
                    actual: "No handshake available for transport conversion".to_string(),
                })
            })?;
            self.transport = Some(handshake.into_transport_mode()?);
            self.state = SessionState::Established;
        }

        self.update_activity(time_source);

        Ok(output)
    }

    /// Encrypt a message (only when established)
    pub fn encrypt<T: TimeSource>(&mut self, plaintext: &[u8], time_source: &T) -> Result<Vec<u8>> {
        if self.state != SessionState::Established {
            return Err(BitchatError::InvalidPacket(
                "Session not established".into(),
            ));
        }

        let result = {
            let transport = self.transport.as_mut().ok_or_else(|| {
                BitchatError::Session(SessionError::InvalidState {
                    peer_id: self.peer_id.to_string(),
                    expected: "Established".to_string(),
                    actual: "No transport state".to_string(),
                })
            })?;
            transport.encrypt(plaintext)
        };

        // Increment message count and update activity
        self.message_count += 1;
        self.update_activity(time_source);

        result
    }

    /// Decrypt a message (only when established)
    pub fn decrypt<T: TimeSource>(
        &mut self,
        ciphertext: &[u8],
        time_source: &T,
    ) -> Result<Vec<u8>> {
        if self.state != SessionState::Established {
            return Err(BitchatError::InvalidPacket(
                "Session not established".into(),
            ));
        }

        let result = {
            let transport = self.transport.as_mut().ok_or_else(|| {
                BitchatError::Session(SessionError::InvalidState {
                    peer_id: self.peer_id.to_string(),
                    expected: "Established".to_string(),
                    actual: "No transport state".to_string(),
                })
            })?;
            transport.decrypt(ciphertext)
        };

        // Increment message count and update activity
        self.message_count += 1;
        self.update_activity(time_source);

        result
    }

    /// Check if session needs rekeying based on message count or time
    /// Uses 90% threshold for message count as per canonical implementation
    pub fn needs_rekey<T: TimeSource>(&self, time_source: &T) -> bool {
        if self.state != SessionState::Established {
            return false;
        }

        // Check message count threshold (90% of max as per canonical spec)
        let rekey_message_threshold = (self.rekey_threshold * 90) / 100;
        if self.message_count >= rekey_message_threshold {
            return true;
        }

        // Check time-based threshold (session timeout from last activity)
        let now = time_source.now();
        let time_since_activity = now
            .as_millis()
            .saturating_sub(self.last_activity.as_millis());
        let session_timeout_ms = self.rekey_interval_secs * 1000;

        time_since_activity >= session_timeout_ms
    }

    /// Initialize rekey process - returns to handshaking state with new keys
    pub fn start_rekey<T: TimeSource>(
        &mut self,
        local_key: &NoiseKeyPair,
        time_source: &T,
    ) -> Result<()> {
        if self.state != SessionState::Established {
            return Err(BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Established".to_string(),
                actual: format!("{:?}", self.state),
            }));
        }

        // Transition to rekeying state
        self.state = SessionState::Rekeying;

        // Create new handshake for rekey
        let handshake = NoiseHandshake::initiator(local_key)?;
        self.handshake = Some(handshake);
        self.transport = None; // Clear old transport

        // Reset counters
        self.message_count = 0;
        self.last_rekey = time_source.now();
        self.update_activity(time_source);

        Ok(())
    }

    /// Complete rekey process and return to established state
    pub fn complete_rekey<T: TimeSource>(&mut self, time_source: &T) -> Result<()> {
        if self.state != SessionState::Rekeying {
            return Err(BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Rekeying".to_string(),
                actual: format!("{:?}", self.state),
            }));
        }

        // Verify we have a completed handshake
        let handshake = self.handshake.as_ref().ok_or_else(|| {
            BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Rekeying with handshake".to_string(),
                actual: "No handshake state".to_string(),
            })
        })?;

        if !handshake.is_handshake_finished() {
            return Err(BitchatError::Session(SessionError::InvalidState {
                peer_id: self.peer_id.to_string(),
                expected: "Completed handshake".to_string(),
                actual: "Handshake not finished".to_string(),
            }));
        }

        // Convert to transport mode
        let handshake = self.handshake.take().unwrap();
        self.transport = Some(handshake.into_transport_mode()?);
        self.state = SessionState::Established;
        self.update_activity(time_source);

        Ok(())
    }

    /// Get current message count
    pub fn message_count(&self) -> u64 {
        self.message_count
    }

    /// Get rekey threshold
    pub fn rekey_threshold(&self) -> u64 {
        self.rekey_threshold
    }

    /// Set rekey threshold
    pub fn set_rekey_threshold(&mut self, threshold: u64) {
        self.rekey_threshold = threshold;
    }

    /// Get rekey interval in seconds
    pub fn rekey_interval_secs(&self) -> u64 {
        self.rekey_interval_secs
    }

    /// Set rekey interval in seconds
    pub fn set_rekey_interval_secs(&mut self, interval_secs: u64) {
        self.rekey_interval_secs = interval_secs;
    }

    /// Check if session is rekeying
    pub fn is_rekeying(&self) -> bool {
        self.state == SessionState::Rekeying
    }

    /// Mark session as failed
    pub fn mark_failed(&mut self) {
        self.state = SessionState::Failed;
        self.handshake = None;
        self.transport = None;
    }

    /// Update last activity timestamp
    fn update_activity<T: TimeSource>(&mut self, time_source: &T) {
        self.last_activity = time_source.now();
    }

    /// Get time since last activity
    pub fn time_since_activity<T: TimeSource>(&self, time_source: &T) -> Duration {
        let now = time_source.now();
        let diff = now
            .as_millis()
            .saturating_sub(self.last_activity.as_millis());
        Duration::from_millis(diff)
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::crypto::NoiseKeyPair;
    use crate::types::SystemTimeSource;

    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            #[test]
            fn test_session_creation() {
                let alice_key = NoiseKeyPair::generate();
                let bob_key = NoiseKeyPair::generate();
                let alice_id = PeerId::from_bytes(&alice_key.public_key_bytes());
                let bob_id = PeerId::from_bytes(&bob_key.public_key_bytes());
                let time_source = SystemTimeSource;

                let alice_session = NoiseSession::new_outbound(bob_id, &alice_key, &time_source).unwrap();
                let bob_session = NoiseSession::new_inbound(alice_id, &bob_key, &time_source).unwrap();

                assert_eq!(alice_session.state(), SessionState::Handshaking);
                assert_eq!(bob_session.state(), SessionState::Handshaking);
                assert_eq!(alice_session.peer_id(), bob_id);
                assert_eq!(bob_session.peer_id(), alice_id);
            }

            #[test]
            fn test_full_handshake() {
        let alice_key = NoiseKeyPair::generate();
        let bob_key = NoiseKeyPair::generate();
        let alice_id = PeerId::from_bytes(&alice_key.public_key_bytes());
        let bob_id = PeerId::from_bytes(&bob_key.public_key_bytes());
        let time_source = SystemTimeSource;

        let mut alice_session =
            NoiseSession::new_outbound(bob_id, &alice_key, &time_source).unwrap();
        let mut bob_session = NoiseSession::new_inbound(alice_id, &bob_key, &time_source).unwrap();

        // Step 1: Alice initiates
        let msg1 = alice_session
            .create_handshake_message(b"", &time_source)
            .unwrap();
        let response1 = bob_session
            .process_handshake_message(&msg1, &time_source)
            .unwrap();

        // Step 2: Bob responds
        let msg2 = response1.unwrap_or_else(|| {
            bob_session
                .create_handshake_message(b"", &time_source)
                .unwrap()
        });
        let response2 = alice_session
            .process_handshake_message(&msg2, &time_source)
            .unwrap();

        // Step 3: Alice finalizes
        let msg3 = response2.unwrap_or_else(|| {
            alice_session
                .create_handshake_message(b"", &time_source)
                .unwrap()
        });
        bob_session
            .process_handshake_message(&msg3, &time_source)
            .unwrap();

        assert!(alice_session.is_established());
        assert!(bob_session.is_established());
        assert!(alice_session.peer_fingerprint().is_some());
        assert!(bob_session.peer_fingerprint().is_some());

                // Test encrypted communication
                let plaintext = b"Hello, Bob!";
                let ciphertext = alice_session.encrypt(plaintext, &time_source).unwrap();
                let decrypted = bob_session.decrypt(&ciphertext, &time_source).unwrap();
                assert_eq!(plaintext.as_slice(), decrypted.as_slice());
            }
        }
    }
}
