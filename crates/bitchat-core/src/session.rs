//! Session management for the BitChat protocol
//!
//! This module provides stateful session management for peer-to-peer communication,
//! including Noise protocol handshakes, message encryption/decryption, and session lifecycle.

use alloc::vec::Vec;
use hashbrown::HashMap;
use core::time::Duration;

use crate::crypto::{NoiseHandshake, NoiseKeyPair, NoiseTransport};
use crate::types::{Fingerprint, PeerId, TimeSource, Timestamp};
use crate::{BitchatError, Result};

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
    /// Session failed or terminated
    Failed,
}

// ----------------------------------------------------------------------------
// Noise Session
// ----------------------------------------------------------------------------

/// A single Noise protocol session with a peer
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

        let handshake = self
            .handshake
            .as_mut()
            .ok_or_else(|| BitchatError::Session { message: "No handshake state".to_string() })?;

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
                use crate::crypto::generate_fingerprint;
                self.peer_fingerprint = Some(generate_fingerprint(&remote_static));
            }

            // Convert to transport mode
            let handshake = self.handshake.take()
                .ok_or_else(|| BitchatError::Session { message: "No handshake available for transport conversion".to_string() })?;
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

        let handshake = self
            .handshake
            .as_mut()
            .ok_or_else(|| BitchatError::Session { message: "No handshake state".to_string() })?;

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
                use crate::crypto::generate_fingerprint;
                self.peer_fingerprint = Some(generate_fingerprint(&remote_static));
            }

            // Convert to transport mode
            let handshake = self.handshake.take()
                .ok_or_else(|| BitchatError::Session { message: "No handshake available for transport conversion".to_string() })?;
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
            let transport = self
                .transport
                .as_mut()
                .ok_or_else(|| BitchatError::Session { message: "No transport state".to_string() })?;
            transport.encrypt(plaintext)
        };

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
            let transport = self
                .transport
                .as_mut()
                .ok_or_else(|| BitchatError::Session { message: "No transport state".to_string() })?;
            transport.decrypt(ciphertext)
        };

        self.update_activity(time_source);
        result
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
// Session Manager
// ----------------------------------------------------------------------------

/// Session timeout configuration
#[derive(Debug, Clone)]
pub struct SessionTimeouts {
    /// Maximum time for handshake completion
    pub handshake_timeout: Duration,
    /// Maximum idle time before session cleanup
    pub idle_timeout: Duration,
}

impl Default for SessionTimeouts {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Manages multiple Noise sessions with different peers
pub struct NoiseSessionManager<T: TimeSource> {
    /// Local Noise key pair
    local_key: NoiseKeyPair,
    /// Active sessions by peer ID
    sessions: HashMap<PeerId, NoiseSession>,
    /// Session timeout configuration
    timeouts: SessionTimeouts,
    /// Time source for generating timestamps
    time_source: T,
}

impl<T: TimeSource> NoiseSessionManager<T> {
    /// Create a new session manager
    pub fn new(local_key: NoiseKeyPair, time_source: T) -> Self {
        Self {
            local_key,
            sessions: HashMap::new(),
            timeouts: SessionTimeouts::default(),
            time_source,
        }
    }

    /// Create a new session manager with custom timeouts
    pub fn with_timeouts(
        local_key: NoiseKeyPair,
        timeouts: SessionTimeouts,
        time_source: T,
    ) -> Self {
        Self {
            local_key,
            sessions: HashMap::new(),
            timeouts,
            time_source,
        }
    }

    /// Get or create outbound session
    pub fn get_or_create_outbound(&mut self, peer_id: PeerId) -> Result<&mut NoiseSession> {
        if !self.sessions.contains_key(&peer_id) {
            let session = NoiseSession::new_outbound(peer_id, &self.local_key, &self.time_source)?;
            self.sessions.insert(peer_id, session);
        }

        self.sessions.get_mut(&peer_id)
            .ok_or_else(|| BitchatError::Session { message: "Session not found after creation".to_string() })
    }

    /// Create inbound session
    pub fn create_inbound(&mut self, peer_id: PeerId) -> Result<&mut NoiseSession> {
        let session = NoiseSession::new_inbound(peer_id, &self.local_key, &self.time_source)?;
        self.sessions.insert(peer_id, session);
        self.sessions.get_mut(&peer_id)
            .ok_or_else(|| BitchatError::Session { message: "Session not found after creation".to_string() })
    }

    /// Get existing session
    pub fn get_session(&self, peer_id: &PeerId) -> Option<&NoiseSession> {
        self.sessions.get(peer_id)
    }

    /// Get mutable session
    pub fn get_session_mut(&mut self, peer_id: &PeerId) -> Option<&mut NoiseSession> {
        self.sessions.get_mut(peer_id)
    }

    /// Remove session
    pub fn remove_session(&mut self, peer_id: &PeerId) -> Option<NoiseSession> {
        self.sessions.remove(peer_id)
    }

    /// Get all active sessions
    pub fn sessions(&self) -> impl Iterator<Item = (&PeerId, &NoiseSession)> {
        self.sessions.iter()
    }

    /// Get count of sessions in each state
    pub fn session_counts(&self) -> (usize, usize, usize) {
        let mut handshaking = 0;
        let mut established = 0;
        let mut failed = 0;

        for session in self.sessions.values() {
            match session.state() {
                SessionState::Handshaking => handshaking += 1,
                SessionState::Established => established += 1,
                SessionState::Failed => failed += 1,
            }
        }

        (handshaking, established, failed)
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&mut self) {
        let expired_peers: Vec<PeerId> = self
            .sessions
            .iter()
            .filter_map(|(peer_id, session)| {
                let timeout = match session.state() {
                    SessionState::Handshaking => self.timeouts.handshake_timeout,
                    SessionState::Established => self.timeouts.idle_timeout,
                    SessionState::Failed => Duration::from_secs(1), // Remove failed immediately
                };

                if session.time_since_activity(&self.time_source) > timeout {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect();

        for peer_id in expired_peers {
            self.sessions.remove(&peer_id);
        }
    }

    /// Get local key fingerprint
    pub fn local_fingerprint(&self) -> Fingerprint {
        self.local_key.fingerprint()
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::NoiseKeyPair;
    use crate::types::StdTimeSource;

    #[cfg(feature = "std")]
    #[test]
    fn test_session_creation() {
        let alice_key = NoiseKeyPair::generate();
        let bob_key = NoiseKeyPair::generate();
        let alice_id = PeerId::from_bytes(&alice_key.public_key_bytes());
        let bob_id = PeerId::from_bytes(&bob_key.public_key_bytes());
        let time_source = StdTimeSource;

        let alice_session = NoiseSession::new_outbound(bob_id, &alice_key, &time_source).unwrap();
        let bob_session = NoiseSession::new_inbound(alice_id, &bob_key, &time_source).unwrap();

        assert_eq!(alice_session.state(), SessionState::Handshaking);
        assert_eq!(bob_session.state(), SessionState::Handshaking);
        assert_eq!(alice_session.peer_id(), bob_id);
        assert_eq!(bob_session.peer_id(), alice_id);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_session_manager() {
        let key = NoiseKeyPair::generate();
        let time_source = StdTimeSource;
        let mut manager = NoiseSessionManager::new(key, time_source);

        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // Create outbound session
        let session = manager.get_or_create_outbound(peer_id).unwrap();
        assert_eq!(session.state(), SessionState::Handshaking);

        // Should return existing session
        let session2 = manager.get_or_create_outbound(peer_id).unwrap();
        assert_eq!(session2.peer_id(), peer_id);

        let (handshaking, established, failed) = manager.session_counts();
        assert_eq!(handshaking, 1);
        assert_eq!(established, 0);
        assert_eq!(failed, 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_full_handshake() {
        let alice_key = NoiseKeyPair::generate();
        let bob_key = NoiseKeyPair::generate();
        let alice_id = PeerId::from_bytes(&alice_key.public_key_bytes());
        let bob_id = PeerId::from_bytes(&bob_key.public_key_bytes());
        let time_source = StdTimeSource;

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
