//! Storage abstraction for identity management
//!
//! Provides cross-platform secure storage interfaces for identity data.
//! Supports native keychain integration, WASM browser storage, and testing mocks.

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};

use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Storage Trait
// ----------------------------------------------------------------------------

/// Key-value storage abstraction for identity data
pub trait SecureStorage: Send + Sync {
    /// Store encrypted data with a key
    fn store(&mut self, key: &str, data: Vec<u8>) -> Result<()>;

    /// Retrieve encrypted data by key
    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Delete data by key
    fn delete(&mut self, key: &str) -> Result<()>;

    /// List all keys (for debugging/cleanup)
    fn list_keys(&self) -> Result<Vec<String>>;

    /// Clear all stored data (panic mode)
    fn clear_all(&mut self) -> Result<()>;

    /// Check if storage is available and accessible
    fn is_available(&self) -> bool;
}

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

/// Storage configuration for different platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Service identifier for keychain/storage
    pub service_id: String,
    /// Access control level
    pub access_level: AccessLevel,
    /// Encryption settings
    pub encryption: EncryptionConfig,
}

/// Access control levels for secure storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessLevel {
    /// Data accessible when device is unlocked
    WhenUnlocked,
    /// Data accessible after first unlock (survives device lock)
    AfterFirstUnlock,
    /// Data always accessible (least secure)
    Always,
}

/// Encryption configuration for stored data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Whether to encrypt data before storage
    pub enabled: bool,
    /// Key derivation method
    pub key_derivation: KeyDerivation,
}

/// Key derivation methods for encryption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyDerivation {
    /// Use device-specific key material
    DeviceBound,
    /// Use user-provided passphrase
    Passphrase,
    /// Use randomly generated key stored separately
    Random,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            service_id: "chat.bitchat.identity".to_string(),
            access_level: AccessLevel::WhenUnlocked,
            encryption: EncryptionConfig {
                enabled: true,
                key_derivation: KeyDerivation::DeviceBound,
            },
        }
    }
}

// ----------------------------------------------------------------------------
// Memory Storage Implementation
// ----------------------------------------------------------------------------

/// In-memory storage implementation for testing and fallback
#[derive(Debug, Default)]
pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
    available: bool,
}

impl MemoryStorage {
    /// Create a new memory storage instance
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            available: true,
        }
    }

    /// Create with specific configuration (config is ignored for memory storage)
    #[allow(unused_variables)]
    pub fn with_config(config: StorageConfig) -> Self {
        Self::new()
    }
}

impl SecureStorage for MemoryStorage {
    fn store(&mut self, key: &str, data: Vec<u8>) -> Result<()> {
        if !self.available {
            return Err(BitchatError::storage_error("Storage not available"));
        }
        self.data.insert(key.to_string(), data);
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if !self.available {
            return Err(BitchatError::storage_error("Storage not available"));
        }
        Ok(self.data.get(key).cloned())
    }

    fn delete(&mut self, key: &str) -> Result<()> {
        if !self.available {
            return Err(BitchatError::storage_error("Storage not available"));
        }
        self.data.remove(key);
        Ok(())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        if !self.available {
            return Err(BitchatError::storage_error("Storage not available"));
        }
        Ok(self.data.keys().cloned().collect())
    }

    fn clear_all(&mut self) -> Result<()> {
        if !self.available {
            return Err(BitchatError::storage_error("Storage not available"));
        }
        self.data.clear();
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

// ----------------------------------------------------------------------------
// Factory Functions
// ----------------------------------------------------------------------------

/// Create a default secure storage implementation for the current platform
pub fn create_default_storage() -> Result<Box<dyn SecureStorage>> {
    // For now, use memory storage for all platforms
    // Platform-specific implementations will be added later:
    // - KeychainStorage for macOS/iOS
    // - BrowserStorage for WASM
    // - FileSystemStorage for Linux/Windows
    Ok(Box::new(MemoryStorage::new()))
}

/// Create a storage implementation for testing
pub fn create_test_storage() -> Box<dyn SecureStorage> {
    Box::new(MemoryStorage::new())
}

// ----------------------------------------------------------------------------
// Errors
// ----------------------------------------------------------------------------

/// Errors related to secure storage operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageError {
    /// Storage not available on this platform
    NotAvailable,
    /// Access denied (device locked, permissions, etc.)
    AccessDenied,
    /// Encryption/decryption failed
    EncryptionFailed,
    /// Key not found
    KeyNotFound(String),
    /// Storage quota exceeded
    QuotaExceeded,
    /// Generic storage error
    StorageError(String),
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "Secure storage not available on this platform"),
            Self::AccessDenied => write!(f, "Access denied to secure storage"),
            Self::EncryptionFailed => write!(f, "Encryption or decryption failed"),
            Self::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            Self::QuotaExceeded => write!(f, "Storage quota exceeded"),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl From<StorageError> for BitchatError {
    fn from(err: StorageError) -> Self {
        BitchatError::storage_error(alloc::format!("{}", err))
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryStorage::new();

        assert!(storage.is_available());

        let key = "test_key";
        let data = vec![1, 2, 3, 4];

        // Store data
        storage.store(key, data.clone()).unwrap();

        // Retrieve data
        let retrieved = storage.retrieve(key).unwrap().unwrap();
        assert_eq!(retrieved, data);

        // List keys
        let keys = storage.list_keys().unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], key);

        // Delete data
        storage.delete(key).unwrap();
        assert!(storage.retrieve(key).unwrap().is_none());

        // Clear all
        storage.store("key1", vec![1]).unwrap();
        storage.store("key2", vec![2]).unwrap();
        storage.clear_all().unwrap();
        assert!(storage.list_keys().unwrap().is_empty());
    }
}
