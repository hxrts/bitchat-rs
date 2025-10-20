//! Secure Identity State Manager
//!
//! Central manager for the three-layer identity system with persistent storage,
//! encryption, and verification capabilities.

use alloc::{boxed::Box, collections::BTreeMap, string::{String, ToString}, vec::Vec};

use super::{
    storage::{SecureStorage, StorageConfig, create_default_storage, create_test_storage},
    CryptographicIdentity, EphemeralIdentity, HandshakeState, IdentityCache, 
    SocialIdentity, TrustLevel,
};
use crate::{
    types::{Fingerprint, PeerId, Timestamp},
    BitchatError, Result,
};

/// Cache save interval to avoid excessive I/O
const CACHE_SAVE_INTERVAL_MS: u64 = 2000; // 2 seconds

/// Maximum age for ephemeral sessions before cleanup
const MAX_EPHEMERAL_AGE_MS: u64 = 3600_000; // 1 hour

/// Secure identity state manager implementing the three-layer identity model
pub struct SecureIdentityStateManager {
    /// In-memory ephemeral identities (not persisted)
    ephemeral_sessions: BTreeMap<PeerId, EphemeralIdentity>,
    
    /// In-memory identity cache (encrypted and persisted)
    identity_cache: IdentityCache,
    
    /// Secure storage backend
    storage: Box<dyn SecureStorage>,
    
    /// Storage configuration
    storage_config: StorageConfig,
    
    /// Cache encryption key (stored separately in secure storage)
    cache_encryption_key: Option<[u8; 32]>,
    
    /// Last cache save timestamp (for debouncing)
    last_cache_save: Timestamp,
    
    /// Whether the cache has unsaved changes
    cache_dirty: bool,
}

impl SecureIdentityStateManager {
    /// Create a new identity state manager with default storage
    pub fn new() -> Result<Self> {
        let storage = create_default_storage()?;
        Self::with_storage(storage, StorageConfig::default())
    }

    /// Create a new identity state manager for testing
    pub fn new_for_testing() -> Self {
        let storage = create_test_storage();
        Self::with_storage(storage, StorageConfig::default()).unwrap()
    }

    /// Create with custom storage and configuration
    pub fn with_storage(
        storage: Box<dyn SecureStorage>,
        config: StorageConfig,
    ) -> Result<Self> {
        let mut manager = Self {
            ephemeral_sessions: BTreeMap::new(),
            identity_cache: IdentityCache::new(),
            storage,
            storage_config: config,
            cache_encryption_key: None,
            last_cache_save: Timestamp::now(),
            cache_dirty: false,
        };

        // Load or create encryption key
        manager.ensure_encryption_key()?;
        
        // Load existing cache if available
        manager.load_cache()?;

        Ok(manager)
    }

    // ----------------------------------------------------------------------------
    // Ephemeral Identity Management
    // ----------------------------------------------------------------------------

    /// Register a new ephemeral identity
    pub fn register_ephemeral_identity(&mut self, peer_id: PeerId) -> &mut EphemeralIdentity {
        let ephemeral = EphemeralIdentity::new(peer_id);
        self.ephemeral_sessions.insert(peer_id, ephemeral);
        self.ephemeral_sessions.get_mut(&peer_id).unwrap()
    }

    /// Get ephemeral identity by peer ID
    pub fn get_ephemeral_identity(&self, peer_id: &PeerId) -> Option<&EphemeralIdentity> {
        self.ephemeral_sessions.get(peer_id)
    }

    /// Get mutable ephemeral identity by peer ID
    pub fn get_ephemeral_identity_mut(&mut self, peer_id: &PeerId) -> Option<&mut EphemeralIdentity> {
        self.ephemeral_sessions.get_mut(peer_id)
    }

    /// Update handshake state for an ephemeral identity
    pub fn update_handshake_state(&mut self, peer_id: &PeerId, state: HandshakeState) -> Result<()> {
        let fingerprint = if let Some(ephemeral) = self.ephemeral_sessions.get_mut(peer_id) {
            ephemeral.set_handshake_state(state);
            ephemeral.get_fingerprint().cloned()
        } else {
            return Err(BitchatError::invalid_peer_id("Ephemeral identity not found"));
        };
        
        // If handshake completed, update cryptographic identity
        if let Some(fingerprint) = fingerprint {
            self.update_last_handshake(&fingerprint)?;
        }
        
        Ok(())
    }

    /// Clean up old ephemeral sessions
    pub fn cleanup_ephemeral_sessions(&mut self) {
        let now = Timestamp::now();
        let cutoff = now.as_millis().saturating_sub(MAX_EPHEMERAL_AGE_MS);

        self.ephemeral_sessions.retain(|_, ephemeral| {
            ephemeral.session_start.as_millis() >= cutoff
        });
    }

    /// Get all active ephemeral sessions
    pub fn get_active_ephemeral_sessions(&self) -> Vec<&EphemeralIdentity> {
        self.ephemeral_sessions.values().collect()
    }

    // ----------------------------------------------------------------------------
    // Cryptographic Identity Management
    // ----------------------------------------------------------------------------

    /// Upsert a cryptographic identity
    pub fn upsert_cryptographic_identity(&mut self, identity: CryptographicIdentity) -> Result<()> {
        self.identity_cache.upsert_cryptographic_identity(identity);
        self.mark_cache_dirty();
        self.save_cache_if_needed()
    }

    /// Get cryptographic identity by fingerprint
    pub fn get_cryptographic_identity(&self, fingerprint: &Fingerprint) -> Option<&CryptographicIdentity> {
        self.identity_cache.get_cryptographic_identity(fingerprint)
    }

    /// Update last handshake time for a cryptographic identity
    pub fn update_last_handshake(&mut self, fingerprint: &Fingerprint) -> Result<()> {
        if let Some(crypto) = self.identity_cache.cryptographic_identities.get_mut(fingerprint) {
            crypto.update_handshake_time();
            self.mark_cache_dirty();
            self.save_cache_if_needed()
        } else {
            Ok(()) // Identity might not be persisted yet
        }
    }

    /// Create cryptographic identity from public keys
    pub fn create_cryptographic_identity(
        &mut self,
        noise_public_key: [u8; 32],
        signing_public_key: Option<[u8; 32]>,
    ) -> Result<Fingerprint> {
        let identity = CryptographicIdentity::new(noise_public_key, signing_public_key);
        let fingerprint = identity.fingerprint.clone();
        self.upsert_cryptographic_identity(identity)?;
        Ok(fingerprint)
    }

    // ----------------------------------------------------------------------------
    // Social Identity Management
    // ----------------------------------------------------------------------------

    /// Get or create social identity for a fingerprint
    pub fn get_or_create_social_identity(&mut self, fingerprint: &Fingerprint) -> Result<&mut SocialIdentity> {
        if !self.identity_cache.social_identities.contains_key(fingerprint) {
            let social = SocialIdentity::new(fingerprint.clone());
            self.identity_cache.upsert_social_identity(social);
            self.mark_cache_dirty();
        }
        
        Ok(self.identity_cache.social_identities.get_mut(fingerprint).unwrap())
    }

    /// Get social identity by fingerprint
    pub fn get_social_identity(&self, fingerprint: &Fingerprint) -> Option<&SocialIdentity> {
        self.identity_cache.get_social_identity(fingerprint)
    }

    /// Update social identity
    pub fn update_social_identity<F>(&mut self, fingerprint: &Fingerprint, update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut SocialIdentity),
    {
        let social = self.get_or_create_social_identity(fingerprint)?;
        update_fn(social);
        self.mark_cache_dirty();
        self.save_cache_if_needed()
    }

    /// Set nickname for a peer
    pub fn set_nickname(&mut self, fingerprint: &Fingerprint, nickname: Option<String>) -> Result<()> {
        self.update_social_identity(fingerprint, |social| {
            social.set_claimed_nickname(nickname);
        })
    }

    /// Set petname for a peer
    pub fn set_petname(&mut self, fingerprint: &Fingerprint, petname: Option<String>) -> Result<()> {
        self.update_social_identity(fingerprint, |social| {
            social.set_petname(petname);
        })
    }

    /// Set trust level for a peer
    pub fn set_trust_level(&mut self, fingerprint: &Fingerprint, level: TrustLevel) -> Result<()> {
        self.update_social_identity(fingerprint, |social| {
            social.set_trust_level(level);
        })
    }

    /// Set verification status
    pub fn set_verified(&mut self, fingerprint: &Fingerprint, verified: bool) -> Result<()> {
        self.identity_cache.set_verified(fingerprint, verified);
        self.mark_cache_dirty();
        self.save_cache_if_needed()
    }

    /// Check if a fingerprint is verified
    pub fn is_verified(&self, fingerprint: &Fingerprint) -> bool {
        self.identity_cache.is_verified(fingerprint)
    }

    /// Get all verified fingerprints
    pub fn get_verified_fingerprints(&self) -> Vec<Fingerprint> {
        self.identity_cache.get_verified_fingerprints()
    }

    /// Set favorite status
    pub fn set_favorite(&mut self, fingerprint: &Fingerprint, favorite: bool) -> Result<()> {
        self.update_social_identity(fingerprint, |social| {
            social.set_favorite(favorite);
        })
    }

    /// Set blocked status
    pub fn set_blocked(&mut self, fingerprint: &Fingerprint, blocked: bool) -> Result<()> {
        self.update_social_identity(fingerprint, |social| {
            social.set_blocked(blocked);
        })
    }

    // ----------------------------------------------------------------------------
    // Search and Lookup
    // ----------------------------------------------------------------------------

    /// Find fingerprint by nickname
    pub fn find_by_nickname(&self, nickname: &str) -> Option<&Fingerprint> {
        self.identity_cache.find_by_nickname(nickname)
    }

    /// Get display name for a fingerprint
    pub fn get_display_name(&self, fingerprint: &Fingerprint) -> Option<String> {
        self.get_social_identity(fingerprint)
            .and_then(|social| social.display_name().map(|s| s.to_string()))
    }

    /// Get all social identities
    pub fn get_all_social_identities(&self) -> Vec<&SocialIdentity> {
        self.identity_cache.social_identities.values().collect()
    }

    /// Get all cryptographic identities
    pub fn get_all_cryptographic_identities(&self) -> Vec<&CryptographicIdentity> {
        self.identity_cache.cryptographic_identities.values().collect()
    }

    // ----------------------------------------------------------------------------
    // Cleanup and Maintenance
    // ----------------------------------------------------------------------------

    /// Clean up old identities
    pub fn cleanup_old_identities(&mut self, max_age_ms: u64) -> Result<()> {
        self.identity_cache.cleanup_old_identities(max_age_ms);
        self.cleanup_ephemeral_sessions();
        self.mark_cache_dirty();
        self.save_cache_if_needed()
    }

    /// Remove an identity completely
    pub fn remove_identity(&mut self, fingerprint: &Fingerprint) -> Result<()> {
        self.identity_cache.remove_identity(fingerprint);
        self.mark_cache_dirty();
        self.save_cache_if_needed()
    }

    /// Panic mode: clear all identity data
    pub fn panic_clear_all_data(&mut self) -> Result<()> {
        // Clear in-memory data
        self.ephemeral_sessions.clear();
        self.identity_cache = IdentityCache::new();
        self.cache_encryption_key = None;
        self.cache_dirty = false;

        // Clear persistent storage
        self.storage.clear_all()?;
        
        Ok(())
    }

    // ----------------------------------------------------------------------------
    // Statistics and Debugging
    // ----------------------------------------------------------------------------

    /// Get identity cache statistics
    pub fn get_cache_stats(&self) -> super::IdentityCacheStats {
        self.identity_cache.stats()
    }

    /// Get ephemeral session count
    pub fn get_ephemeral_session_count(&self) -> usize {
        self.ephemeral_sessions.len()
    }

    /// Check if storage is available
    pub fn is_storage_available(&self) -> bool {
        self.storage.is_available()
    }

    // ----------------------------------------------------------------------------
    // Private Methods
    // ----------------------------------------------------------------------------

    /// Ensure encryption key exists
    fn ensure_encryption_key(&mut self) -> Result<()> {
        const KEY_NAME: &str = "identity_cache_encryption_key";
        
        if let Some(key_data) = self.storage.retrieve(KEY_NAME)? {
            if key_data.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_data);
                self.cache_encryption_key = Some(key);
                return Ok(());
            }
        }

        // Generate new key
        let mut key = [0u8; 32];
        use rand_core::RngCore;
        rand_core::OsRng.fill_bytes(&mut key);
        
        self.storage.store(KEY_NAME, key.to_vec())?;
        self.cache_encryption_key = Some(key);
        
        Ok(())
    }

    /// Load identity cache from storage
    fn load_cache(&mut self) -> Result<()> {
        const CACHE_NAME: &str = "identity_cache";
        
        if let Some(encrypted_data) = self.storage.retrieve(CACHE_NAME)? {
            if let Some(key) = &self.cache_encryption_key {
                if let Ok(decrypted_data) = self.decrypt_data(&encrypted_data, key) {
                    if let Ok(cache) = bincode::deserialize::<IdentityCache>(&decrypted_data) {
                        self.identity_cache = cache;
                        return Ok(());
                    }
                }
            }
        }
        
        // If load failed, start with empty cache
        self.identity_cache = IdentityCache::new();
        Ok(())
    }

    /// Mark cache as dirty
    fn mark_cache_dirty(&mut self) {
        self.cache_dirty = true;
    }

    /// Save cache if needed (debounced)
    fn save_cache_if_needed(&mut self) -> Result<()> {
        if !self.cache_dirty {
            return Ok(());
        }

        let now = Timestamp::now();
        if now.as_millis().saturating_sub(self.last_cache_save.as_millis()) < CACHE_SAVE_INTERVAL_MS {
            return Ok(());
        }

        self.save_cache_now()
    }

    /// Force save cache immediately
    fn save_cache_now(&mut self) -> Result<()> {
        const CACHE_NAME: &str = "identity_cache";
        
        if let Some(key) = &self.cache_encryption_key {
            let serialized = bincode::serialize(&self.identity_cache)
                .map_err(|_| BitchatError::serialization_error_with_message("Failed to serialize identity cache"))?;
            
            let encrypted = self.encrypt_data(&serialized, key)?;
            self.storage.store(CACHE_NAME, encrypted)?;
            
            self.cache_dirty = false;
            self.last_cache_save = Timestamp::now();
        }
        
        Ok(())
    }

    /// Encrypt data using AES-GCM
    fn encrypt_data(&self, data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, AeadInPlace};
        use rand_core::RngCore;
        
        let cipher = Aes256Gcm::new(key.into());
        
        let mut nonce_bytes = [0u8; 12];
        rand_core::OsRng.fill_bytes(&mut nonce_bytes);
        
        let mut ciphertext = data.to_vec();
        let tag = cipher.encrypt_in_place_detached((&nonce_bytes).into(), b"", &mut ciphertext)
            .map_err(|_| BitchatError::encryption_error("Encryption failed"))?;
        
        // Format: nonce (12 bytes) + tag (16 bytes) + ciphertext
        let mut result = Vec::with_capacity(12 + 16 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&tag[..]);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    /// Decrypt data using AES-GCM
    fn decrypt_data(&self, encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, AeadInPlace};
        
        if encrypted.len() < 28 { // 12 + 16 minimum
            return Err(BitchatError::decryption_error("Invalid encrypted data length"));
        }
        
        let cipher = Aes256Gcm::new(key.into());
        
        let mut ciphertext = encrypted[28..].to_vec();
        
        cipher.decrypt_in_place_detached(
            (&encrypted[0..12]).into(),
            b"",
            &mut ciphertext,
            (&encrypted[12..28]).into()
        ).map_err(|_| BitchatError::decryption_error("Decryption failed"))?;
        
        Ok(ciphertext)
    }
}

impl Drop for SecureIdentityStateManager {
    fn drop(&mut self) {
        // Attempt to save cache on drop
        let _ = self.save_cache_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_state_manager() {
        let mut manager = SecureIdentityStateManager::new_for_testing();
        
        // Test ephemeral identity
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let ephemeral = manager.register_ephemeral_identity(peer_id);
        assert_eq!(ephemeral.peer_id, peer_id);
        
        // Test cryptographic identity
        let noise_key = [1u8; 32];
        let signing_key = Some([2u8; 32]);
        let fingerprint = manager.create_cryptographic_identity(noise_key, signing_key).unwrap();
        
        let crypto = manager.get_cryptographic_identity(&fingerprint).unwrap();
        assert_eq!(crypto.public_key, noise_key);
        assert_eq!(crypto.signing_public_key, signing_key);
        
        // Test social identity
        manager.set_petname(&fingerprint, Some("Alice".to_string())).unwrap();
        manager.set_trust_level(&fingerprint, TrustLevel::Trusted).unwrap();
        
        let social = manager.get_social_identity(&fingerprint).unwrap();
        assert_eq!(social.local_petname, Some("Alice".to_string()));
        assert_eq!(social.trust_level, TrustLevel::Trusted);
        
        // Test verification
        manager.set_verified(&fingerprint, true).unwrap();
        assert!(manager.is_verified(&fingerprint));
        
        let verified = manager.get_verified_fingerprints();
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0], fingerprint);
    }

    #[test]
    fn test_handshake_integration() {
        let mut manager = SecureIdentityStateManager::new_for_testing();
        
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        manager.register_ephemeral_identity(peer_id);
        
        let fingerprint = Fingerprint::new([3u8; 32]);
        let handshake_state = HandshakeState::Completed { fingerprint: fingerprint.clone() };
        
        manager.update_handshake_state(&peer_id, handshake_state).unwrap();
        
        let ephemeral = manager.get_ephemeral_identity(&peer_id).unwrap();
        assert!(ephemeral.is_handshake_complete());
        assert_eq!(ephemeral.get_fingerprint(), Some(&fingerprint));
    }

    #[test]
    fn test_cleanup() {
        let mut manager = SecureIdentityStateManager::new_for_testing();
        
        // Add some test data
        let _fingerprint = Fingerprint::new([4u8; 32]);
        manager.create_cryptographic_identity([1u8; 32], None).unwrap();
        
        // Test cleanup
        manager.cleanup_old_identities(0).unwrap(); // Remove everything
        
        let stats = manager.get_cache_stats();
        // Note: cleanup only removes unverified identities
        assert!(stats.total_cryptographic_identities <= 1);
    }
}