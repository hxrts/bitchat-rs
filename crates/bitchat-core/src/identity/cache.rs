//! Identity cache for storing all identity data

use alloc::{collections::BTreeMap, vec::Vec};
use serde::{Deserialize, Serialize};

use super::{CryptographicIdentity, SocialIdentity};
use crate::types::{Fingerprint, Timestamp};

/// In-memory cache of all identity data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCache {
    /// Cryptographic identities by fingerprint
    pub cryptographic_identities: BTreeMap<Fingerprint, CryptographicIdentity>,
    /// Social identities by fingerprint
    pub social_identities: BTreeMap<Fingerprint, SocialIdentity>,
    /// Verified fingerprints
    pub verified_fingerprints: Vec<Fingerprint>,
}

impl IdentityCache {
    /// Create a new empty identity cache
    pub fn new() -> Self {
        Self {
            cryptographic_identities: BTreeMap::new(),
            social_identities: BTreeMap::new(),
            verified_fingerprints: Vec::new(),
        }
    }

    /// Upsert a cryptographic identity
    pub fn upsert_cryptographic_identity(&mut self, identity: CryptographicIdentity) {
        self.cryptographic_identities
            .insert(identity.fingerprint.clone(), identity);
    }

    /// Get cryptographic identity by fingerprint
    pub fn get_cryptographic_identity(
        &self,
        fingerprint: &Fingerprint,
    ) -> Option<&CryptographicIdentity> {
        self.cryptographic_identities.get(fingerprint)
    }

    /// Upsert a social identity
    pub fn upsert_social_identity(&mut self, identity: SocialIdentity) {
        self.social_identities
            .insert(identity.fingerprint.clone(), identity);
    }

    /// Get social identity by fingerprint
    pub fn get_social_identity(&self, fingerprint: &Fingerprint) -> Option<&SocialIdentity> {
        self.social_identities.get(fingerprint)
    }

    /// Set verified status
    pub fn set_verified(&mut self, fingerprint: &Fingerprint, verified: bool) {
        if verified {
            if !self.verified_fingerprints.contains(fingerprint) {
                self.verified_fingerprints.push(fingerprint.clone());
            }
        } else {
            self.verified_fingerprints.retain(|f| f != fingerprint);
        }
    }

    /// Check if a fingerprint is verified
    pub fn is_verified(&self, fingerprint: &Fingerprint) -> bool {
        self.verified_fingerprints.contains(fingerprint)
    }

    /// Get all verified fingerprints
    pub fn get_verified_fingerprints(&self) -> Vec<Fingerprint> {
        self.verified_fingerprints.clone()
    }

    /// Find fingerprint by nickname
    pub fn find_by_nickname(&self, nickname: &str) -> Option<&Fingerprint> {
        self.social_identities
            .iter()
            .find(|(_, social)| {
                social
                    .local_petname
                    .as_deref()
                    .or(social.claimed_nickname.as_deref())
                    == Some(nickname)
            })
            .map(|(fp, _)| fp)
    }

    /// Clean up old identities
    pub fn cleanup_old_identities(&mut self, max_age_ms: u64) {
        let now = Timestamp::now();
        let cutoff = now.as_millis().saturating_sub(max_age_ms);

        // Clone the verified set to avoid borrow checker issues
        let verified_set = self.verified_fingerprints.clone();

        // Remove cryptographic identities that haven't been seen recently
        // and are not verified
        self.cryptographic_identities.retain(|fp, crypto| {
            crypto.last_handshake.as_millis() >= cutoff || verified_set.contains(fp)
        });

        // Remove social identities without corresponding crypto identities
        self.social_identities
            .retain(|fp, _| self.cryptographic_identities.contains_key(fp));

        // Clean up verified list
        self.verified_fingerprints
            .retain(|fp| self.cryptographic_identities.contains_key(fp));
    }

    /// Remove an identity completely
    pub fn remove_identity(&mut self, fingerprint: &Fingerprint) {
        self.cryptographic_identities.remove(fingerprint);
        self.social_identities.remove(fingerprint);
        self.verified_fingerprints.retain(|f| f != fingerprint);
    }

    /// Get cache statistics
    pub fn stats(&self) -> IdentityCacheStats {
        IdentityCacheStats {
            total_cryptographic_identities: self.cryptographic_identities.len(),
            total_social_identities: self.social_identities.len(),
            total_verified: self.verified_fingerprints.len(),
            total_favorites: self
                .social_identities
                .values()
                .filter(|s| s.is_favorite)
                .count(),
            total_blocked: self
                .social_identities
                .values()
                .filter(|s| s.is_blocked)
                .count(),
        }
    }
}

impl Default for IdentityCache {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------------
// Statistics
// ----------------------------------------------------------------------------

/// Statistics about the identity cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCacheStats {
    /// Total cryptographic identities
    pub total_cryptographic_identities: usize,
    /// Total social identities
    pub total_social_identities: usize,
    /// Total verified identities
    pub total_verified: usize,
    /// Total favorites
    pub total_favorites: usize,
    /// Total blocked
    pub total_blocked: usize,
}
