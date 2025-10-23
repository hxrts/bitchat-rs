//! Social identity with user-assigned metadata

use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::types::TrustLevel;
use crate::types::{Fingerprint, Timestamp};

/// Social identity with user-assigned metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialIdentity {
    /// Associated fingerprint
    pub fingerprint: Fingerprint,
    /// Peer's claimed nickname
    pub claimed_nickname: Option<String>,
    /// User-assigned local petname
    pub local_petname: Option<String>,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Is this peer a favorite?
    pub is_favorite: bool,
    /// Is this peer blocked?
    pub is_blocked: bool,
    /// Last interaction timestamp
    pub last_interaction: Timestamp,
    /// Notes about this peer
    pub notes: Option<String>,
}

impl SocialIdentity {
    /// Create a new social identity
    pub fn new(fingerprint: Fingerprint) -> Self {
        Self {
            fingerprint,
            claimed_nickname: None,
            local_petname: None,
            trust_level: TrustLevel::Unknown,
            is_favorite: false,
            is_blocked: false,
            last_interaction: Timestamp::now(),
            notes: None,
        }
    }

    /// Get the display name (petname if set, otherwise claimed nickname)
    pub fn display_name(&self) -> Option<&str> {
        self.local_petname
            .as_deref()
            .or(self.claimed_nickname.as_deref())
    }

    /// Set claimed nickname
    pub fn set_claimed_nickname(&mut self, nickname: Option<String>) {
        self.claimed_nickname = nickname;
        self.last_interaction = Timestamp::now();
    }

    /// Set local petname
    pub fn set_petname(&mut self, petname: Option<String>) {
        self.local_petname = petname;
        self.last_interaction = Timestamp::now();
    }

    /// Set trust level
    pub fn set_trust_level(&mut self, level: TrustLevel) {
        self.trust_level = level;
        self.last_interaction = Timestamp::now();
    }

    /// Set favorite status
    pub fn set_favorite(&mut self, favorite: bool) {
        self.is_favorite = favorite;
        self.last_interaction = Timestamp::now();
    }

    /// Set blocked status
    pub fn set_blocked(&mut self, blocked: bool) {
        self.is_blocked = blocked;
        self.last_interaction = Timestamp::now();
    }
}
