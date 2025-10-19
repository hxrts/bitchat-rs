//! Geohash-based location channels for BitChat
//!
//! This module implements the geohash channel system that allows location-based
//! messaging with privacy features. Messages are organized into geographic channels
//! based on geohash precision levels.

use crate::protocol::crypto::IdentityKeyPair;
use crate::types::{Fingerprint, PeerId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Geohash precision levels for location-based channels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeohashPrecision {
    /// Region level (2 characters) - ~2500km coverage
    Region = 2,
    /// State level (3 characters) - ~630km coverage  
    State = 3,
    /// City level (4 characters) - ~156km coverage
    City = 4,
    /// District level (5 characters) - ~39km coverage
    District = 5,
    /// Neighborhood level (6 characters) - ~4.9km coverage
    Neighborhood = 6,
    /// Block level (7 characters) - ~1.2km coverage
    Block = 7,
    /// Building level (8 characters) - ~153m coverage
    Building = 8,
}

impl GeohashPrecision {
    /// Get all available precision levels in order from largest to smallest coverage
    pub fn all() -> &'static [GeohashPrecision] {
        &[
            GeohashPrecision::Region,
            GeohashPrecision::State,
            GeohashPrecision::City,
            GeohashPrecision::District,
            GeohashPrecision::Neighborhood,
            GeohashPrecision::Block,
            GeohashPrecision::Building,
        ]
    }

    /// Get the approximate coverage radius in meters
    pub fn coverage_radius_meters(&self) -> f64 {
        match self {
            GeohashPrecision::Region => 2_500_000.0,
            GeohashPrecision::State => 630_000.0,
            GeohashPrecision::City => 156_000.0,
            GeohashPrecision::District => 39_000.0,
            GeohashPrecision::Neighborhood => 4_900.0,
            GeohashPrecision::Block => 1_200.0,
            GeohashPrecision::Building => 153.0,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            GeohashPrecision::Region => "Region (~2500km)",
            GeohashPrecision::State => "State (~630km)",
            GeohashPrecision::City => "City (~156km)",
            GeohashPrecision::District => "District (~39km)",
            GeohashPrecision::Neighborhood => "Neighborhood (~4.9km)",
            GeohashPrecision::Block => "Block (~1.2km)",
            GeohashPrecision::Building => "Building (~153m)",
        }
    }
}

impl fmt::Display for GeohashPrecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// A geographic location represented by latitude and longitude
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoLocation {
    /// Latitude in degrees (-90 to 90)
    pub latitude: f64,
    /// Longitude in degrees (-180 to 180)
    pub longitude: f64,
}

impl GeoLocation {
    /// Create a new geographic location
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, GeohashError> {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(GeohashError::InvalidLatitude(latitude));
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(GeohashError::InvalidLongitude(longitude));
        }

        Ok(Self {
            latitude,
            longitude,
        })
    }

    /// Encode this location as a geohash with the specified precision
    pub fn to_geohash(&self, precision: GeohashPrecision) -> String {
        geohash::encode(
            geohash::Coord {
                x: self.longitude,
                y: self.latitude,
            },
            precision as usize,
        )
        .unwrap_or_else(|_| {
            // Fallback to a default geohash if encoding fails
            "s".repeat(precision as usize)
        })
    }

    /// Get all geohash channels this location belongs to
    pub fn to_all_geohashes(&self) -> Vec<(GeohashPrecision, String)> {
        GeohashPrecision::all()
            .iter()
            .map(|&precision| (precision, self.to_geohash(precision)))
            .collect()
    }
}

/// A geohash-based location channel
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeohashChannel {
    /// The geohash string identifying this channel
    pub geohash: String,
    /// The precision level of this geohash
    pub precision: GeohashPrecision,
}

impl GeohashChannel {
    /// Create a new geohash channel
    pub fn new(geohash: String, precision: GeohashPrecision) -> Result<Self, GeohashError> {
        if geohash.len() != precision as usize {
            return Err(GeohashError::InvalidGeohashLength {
                expected: precision as usize,
                actual: geohash.len(),
            });
        }

        // Validate geohash characters (base32: 0-9, b-z except a,i,l,o)
        const VALID_CHARS: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";
        for byte in geohash.as_bytes() {
            if !VALID_CHARS.contains(byte) {
                return Err(GeohashError::InvalidGeohashCharacter(*byte as char));
            }
        }

        Ok(Self { geohash, precision })
    }

    /// Create a geohash channel from a geographic location
    pub fn from_location(location: GeoLocation, precision: GeohashPrecision) -> Self {
        let geohash = location.to_geohash(precision);
        Self { geohash, precision }
    }

    /// Get the parent channel (one precision level less specific)
    pub fn parent(&self) -> Option<GeohashChannel> {
        if self.geohash.len() <= 2 {
            return None; // Region is the top level
        }

        let parent_geohash = self.geohash[..self.geohash.len() - 1].to_string();
        let parent_precision = match self.precision {
            GeohashPrecision::State => GeohashPrecision::Region,
            GeohashPrecision::City => GeohashPrecision::State,
            GeohashPrecision::District => GeohashPrecision::City,
            GeohashPrecision::Neighborhood => GeohashPrecision::District,
            GeohashPrecision::Block => GeohashPrecision::Neighborhood,
            GeohashPrecision::Building => GeohashPrecision::Block,
            GeohashPrecision::Region => return None,
        };

        Some(GeohashChannel {
            geohash: parent_geohash,
            precision: parent_precision,
        })
    }

    /// Get all parent channels up to the region level
    pub fn all_parents(&self) -> Vec<GeohashChannel> {
        let mut parents = Vec::new();
        let mut current = self.parent();
        while let Some(parent) = current {
            parents.push(parent.clone());
            current = parent.parent();
        }
        parents
    }

    /// Generate a privacy-preserving identity for this geohash channel
    /// This creates a deterministic but unlinkable identity per geohash
    pub fn derive_channel_identity(
        &self,
        base_identity: &IdentityKeyPair,
    ) -> Result<ChannelIdentity, GeohashError> {
        // Create a deterministic seed from base identity and geohash
        let mut hasher = Sha256::new();
        hasher.update(b"bitchat-geohash-channel-v1");
        hasher.update(base_identity.public_key_bytes());
        hasher.update(self.geohash.as_bytes());
        hasher.update([self.precision as u8]);
        let seed = hasher.finalize();

        // Generate ephemeral keys from the seed
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&seed[..32]);
        let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);

        // Generate fingerprint from the public key
        let mut fingerprint_hasher = Sha256::new();
        fingerprint_hasher.update(keypair.verifying_key().to_bytes());
        let fingerprint_bytes = fingerprint_hasher.finalize();
        let mut fingerprint_array = [0u8; 32];
        fingerprint_array.copy_from_slice(&fingerprint_bytes);
        let fingerprint = Fingerprint::new(fingerprint_array);

        // Create PeerId from fingerprint
        let peer_id = fingerprint.to_peer_id();

        Ok(ChannelIdentity {
            channel: self.clone(),
            peer_id,
            fingerprint,
            signing_key: keypair,
        })
    }

    /// Get the channel identifier for Nostr topic/tag
    pub fn channel_id(&self) -> String {
        format!("bitchat-geo-{}-{}", self.precision as u8, self.geohash)
    }
}

impl fmt::Display for GeohashChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.geohash, self.precision.description())
    }
}

/// A privacy-preserving identity for a specific geohash channel
#[derive(Debug, Clone)]
pub struct ChannelIdentity {
    /// The geohash channel this identity is for
    pub channel: GeohashChannel,
    /// The peer ID for this channel
    pub peer_id: PeerId,
    /// The fingerprint for this channel
    pub fingerprint: Fingerprint,
    /// The signing key for this channel (private)
    signing_key: ed25519_dalek::SigningKey,
}

impl ChannelIdentity {
    /// Get the signing key (for internal use)
    pub fn signing_key(&self) -> &ed25519_dalek::SigningKey {
        &self.signing_key
    }

    /// Get the Ed25519 signing key bytes for external conversion (e.g., to Nostr keys)
    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

/// Location privacy manager for geohash channels
#[derive(Debug, Clone)]
pub struct LocationPrivacyManager {
    /// Base identity for deriving channel identities
    base_identity: IdentityKeyPair,
    /// Currently active channel (if any)
    active_channel: Option<GeohashChannel>,
    /// Cached channel identities
    channel_identities: std::collections::HashMap<GeohashChannel, ChannelIdentity>,
}

impl LocationPrivacyManager {
    /// Create a new location privacy manager
    pub fn new(base_identity: IdentityKeyPair) -> Self {
        Self {
            base_identity,
            active_channel: None,
            channel_identities: std::collections::HashMap::new(),
        }
    }

    /// Set the active location and channel precision
    pub fn set_location(
        &mut self,
        location: GeoLocation,
        precision: GeohashPrecision,
    ) -> Result<&ChannelIdentity, GeohashError> {
        let channel = GeohashChannel::from_location(location, precision);
        self.set_channel(channel)
    }

    /// Set the active channel directly (for "teleporting" to arbitrary locations)
    pub fn set_channel(
        &mut self,
        channel: GeohashChannel,
    ) -> Result<&ChannelIdentity, GeohashError> {
        // Cache the channel identity if not already cached
        if !self.channel_identities.contains_key(&channel) {
            let identity = channel.derive_channel_identity(&self.base_identity)?;
            self.channel_identities.insert(channel.clone(), identity);
        }

        self.active_channel = Some(channel.clone());
        Ok(self.channel_identities.get(&channel).unwrap())
    }

    /// Get the current active channel identity
    pub fn active_identity(&self) -> Option<&ChannelIdentity> {
        self.active_channel
            .as_ref()
            .and_then(|channel| self.channel_identities.get(channel))
    }

    /// Get the current active channel
    pub fn active_channel(&self) -> Option<&GeohashChannel> {
        self.active_channel.as_ref()
    }

    /// Get all channels this location belongs to (for listening to messages)
    pub fn location_channels(&self, location: GeoLocation) -> Vec<GeohashChannel> {
        location
            .to_all_geohashes()
            .into_iter()
            .map(|(precision, geohash)| GeohashChannel { geohash, precision })
            .collect()
    }

    /// Clear the active location (go offline from location channels)
    pub fn clear_location(&mut self) {
        self.active_channel = None;
    }

    /// Get or create a channel identity for a specific channel
    pub fn get_channel_identity(
        &mut self,
        channel: &GeohashChannel,
    ) -> Result<&ChannelIdentity, GeohashError> {
        if !self.channel_identities.contains_key(channel) {
            let identity = channel.derive_channel_identity(&self.base_identity)?;
            self.channel_identities.insert(channel.clone(), identity);
        }
        Ok(self.channel_identities.get(channel).unwrap())
    }
}

/// Errors that can occur in geohash operations
#[derive(Debug, thiserror::Error)]
pub enum GeohashError {
    #[error("Invalid latitude: {0} (must be between -90 and 90 degrees)")]
    InvalidLatitude(f64),

    #[error("Invalid longitude: {0} (must be between -180 and 180 degrees)")]
    InvalidLongitude(f64),

    #[error("Invalid geohash length: expected {expected}, got {actual}")]
    InvalidGeohashLength { expected: usize, actual: usize },

    #[error("Invalid geohash character: '{0}' (must be base32)")]
    InvalidGeohashCharacter(char),

    #[error("Failed to generate cryptographic keys")]
    KeyGenerationFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geolocation_creation() {
        let loc = GeoLocation::new(37.7749, -122.4194).unwrap(); // San Francisco
        assert_eq!(loc.latitude, 37.7749);
        assert_eq!(loc.longitude, -122.4194);

        // Test invalid coordinates
        assert!(GeoLocation::new(91.0, 0.0).is_err()); // Invalid latitude
        assert!(GeoLocation::new(0.0, 181.0).is_err()); // Invalid longitude
    }

    #[test]
    fn test_geohash_generation() {
        let sf = GeoLocation::new(37.7749, -122.4194).unwrap(); // San Francisco

        let region = sf.to_geohash(GeohashPrecision::Region);
        assert_eq!(region.len(), 2);

        let building = sf.to_geohash(GeohashPrecision::Building);
        assert_eq!(building.len(), 8);

        // More precise geohashes should start with less precise ones
        assert!(building.starts_with(&region));
    }

    #[test]
    fn test_geohash_channel_creation() {
        let channel = GeohashChannel::new("9q8y".to_string(), GeohashPrecision::City).unwrap();
        assert_eq!(channel.geohash, "9q8y");
        assert_eq!(channel.precision, GeohashPrecision::City);

        // Test invalid length
        assert!(GeohashChannel::new("9q".to_string(), GeohashPrecision::City).is_err());
    }

    #[test]
    fn test_channel_hierarchy() {
        let channel =
            GeohashChannel::new("9q8yyk8z".to_string(), GeohashPrecision::Building).unwrap();

        let parent = channel.parent().unwrap();
        assert_eq!(parent.geohash, "9q8yyk8");
        assert_eq!(parent.precision, GeohashPrecision::Block);

        let all_parents = channel.all_parents();
        assert_eq!(all_parents.len(), 6); // Building -> Region
    }

    #[test]
    fn test_channel_identity_derivation() {
        let identity = IdentityKeyPair::generate().unwrap();
        let channel = GeohashChannel::new("9q8y".to_string(), GeohashPrecision::City).unwrap();

        let channel_id1 = channel.derive_channel_identity(&identity).unwrap();
        let channel_id2 = channel.derive_channel_identity(&identity).unwrap();

        // Same inputs should produce same identity
        assert_eq!(channel_id1.peer_id, channel_id2.peer_id);
        assert_eq!(channel_id1.fingerprint, channel_id2.fingerprint);
    }

    #[test]
    fn test_location_privacy_manager() {
        let identity = IdentityKeyPair::generate().unwrap();
        let mut manager = LocationPrivacyManager::new(identity);

        let sf = GeoLocation::new(37.7749, -122.4194).unwrap();
        {
            let channel_id = manager.set_location(sf, GeohashPrecision::City).unwrap();
            assert_eq!(channel_id.channel.precision, GeohashPrecision::City);
        }

        assert!(manager.active_identity().is_some());
        assert!(manager.active_channel().is_some());

        manager.clear_location();
        assert!(manager.active_identity().is_none());
        assert!(manager.active_channel().is_none());
    }
}
