//! Session manager for the BitChat runtime
//!
//! This module contains the stateful NoiseSessionManager that manages multiple
//! sessions with different peers.

use std::collections::HashMap;
use core::time::Duration;

use bitchat_core::{
    BitchatError, BitchatResult, PeerId, 
    internal::{SessionError, NoiseKeyPair, NoiseSession, SessionState, TimeSource, Fingerprint}
};

// ----------------------------------------------------------------------------
// Session Timeout Configuration
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

// ----------------------------------------------------------------------------
// Session Manager
// ----------------------------------------------------------------------------

/// Manages multiple Noise sessions with different peers
#[derive(Debug)]
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
    pub fn new(local_key: NoiseKeyPair, time_source: T, timeouts: SessionTimeouts) -> Self {
        Self {
            local_key,
            sessions: HashMap::new(),
            timeouts,
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
    pub fn get_or_create_outbound(&mut self, peer_id: PeerId) -> BitchatResult<&mut NoiseSession> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.sessions.entry(peer_id) {
            let session = NoiseSession::new_outbound(peer_id, &self.local_key, &self.time_source)?;
            e.insert(session);
        }

        self.sessions
            .get_mut(&peer_id)
            .ok_or_else(|| BitchatError::Session(SessionError::SessionNotFound {
                peer_id: peer_id.to_string(),
            }))
    }

    /// Create inbound session
    pub fn create_inbound(&mut self, peer_id: PeerId) -> BitchatResult<&mut NoiseSession> {
        let session = NoiseSession::new_inbound(peer_id, &self.local_key, &self.time_source)?;
        self.sessions.insert(peer_id, session);
        self.sessions
            .get_mut(&peer_id)
            .ok_or_else(|| BitchatError::Session(SessionError::SessionNotFound {
                peer_id: peer_id.to_string(),
            }))
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
    use bitchat_core::internal::NoiseKeyPair;
    use bitchat_core::SystemTimeSource;

    #[cfg(feature = "std")]
    #[test]
    fn test_session_manager() {
        let key = NoiseKeyPair::generate();
        let time_source = SystemTimeSource;
        let mut manager = NoiseSessionManager::new(key, time_source, SessionTimeouts::default());

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
}