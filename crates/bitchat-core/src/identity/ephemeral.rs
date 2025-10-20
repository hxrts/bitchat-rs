//! Ephemeral identity for temporary sessions

use serde::{Deserialize, Serialize};

use super::types::HandshakeState;
use crate::types::{Fingerprint, PeerId, Timestamp};

/// Ephemeral identity for a temporary session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralIdentity {
    /// Temporary peer ID for this session
    pub peer_id: PeerId,
    /// Current handshake state
    pub handshake_state: HandshakeState,
    /// Session start time
    pub session_start: Timestamp,
    /// Last activity timestamp
    pub last_activity: Timestamp,
}

impl EphemeralIdentity {
    /// Create a new ephemeral identity
    pub fn new(peer_id: PeerId) -> Self {
        let now = Timestamp::now();
        Self {
            peer_id,
            handshake_state: HandshakeState::None,
            session_start: now,
            last_activity: now,
        }
    }

    /// Update handshake state
    pub fn set_handshake_state(&mut self, state: HandshakeState) {
        self.handshake_state = state;
        self.last_activity = Timestamp::now();
    }

    /// Get fingerprint if handshake is complete
    pub fn get_fingerprint(&self) -> Option<&Fingerprint> {
        self.handshake_state.fingerprint()
    }

    /// Check if handshake is complete
    pub fn is_handshake_complete(&self) -> bool {
        self.handshake_state.is_complete()
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Timestamp::now();
    }
}

