//! Capability detection and version negotiation for BitChat interoperability
//!
//! This module implements capability announcement and version negotiation to ensure
//! graceful interoperability with different BitChat implementations that may support
//! different feature sets.

use alloc::{collections::BTreeSet, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::protocol::message::NoisePayloadType;
use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Current capability protocol version
pub const CAPABILITY_PROTOCOL_VERSION: u8 = 1;

/// Maximum number of capabilities to announce
pub const MAX_CAPABILITIES: usize = 64;

/// Capability negotiation timeout in milliseconds
pub const CAPABILITY_TIMEOUT: u64 = 30 * 1000; // 30 seconds

// ----------------------------------------------------------------------------
// Core Types
// ----------------------------------------------------------------------------

/// BitChat protocol version with feature support
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version number
    pub major: u8,
    /// Minor version number  
    pub minor: u8,
    /// Patch version number
    pub patch: u8,
}

impl ProtocolVersion {
    /// Create a new protocol version
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Current BitChat protocol version (1.0.0)
    pub fn current() -> Self {
        Self::new(1, 0, 0)
    }

    /// Check if this version is compatible with another version
    pub fn is_compatible_with(&self, other: &ProtocolVersion) -> bool {
        // Same major version is required for compatibility
        self.major == other.major
    }

    /// Check if this version is newer than another version
    pub fn is_newer_than(&self, other: &ProtocolVersion) -> bool {
        if self.major != other.major {
            return self.major > other.major;
        }
        if self.minor != other.minor {
            return self.minor > other.minor;
        }
        self.patch > other.patch
    }
}

impl core::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Capability identifier for optional features
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Create a new capability identifier
    pub fn new(id: String) -> Result<Self> {
        if id.is_empty() || id.len() > 64 {
            return Err(BitchatError::invalid_packet("Invalid capability ID length"));
        }
        Ok(Self(id))
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    // Standard capability identifiers for BitChat features
    
    /// Core messaging capability (always present)
    pub fn core_messaging() -> Self {
        Self("core.messaging.v1".to_string())
    }

    /// File transfer capability
    pub fn file_transfer() -> Self {
        Self("file_transfer.v1".to_string())
    }

    /// Group messaging capability
    pub fn group_messaging() -> Self {
        Self("group_messaging.v1".to_string())
    }

    /// Multi-device session synchronization capability
    pub fn multi_device_sync() -> Self {
        Self("multi_device_sync.v1".to_string())
    }

    /// Geohash location channels capability
    pub fn location_channels() -> Self {
        Self("location_channels.v1".to_string())
    }

    /// Message fragmentation capability
    pub fn fragmentation() -> Self {
        Self("fragmentation.v1".to_string())
    }

    /// Mesh synchronization capability
    pub fn mesh_sync() -> Self {
        Self("mesh_sync.v1".to_string())
    }

    /// Noise Protocol capability
    pub fn noise_protocol() -> Self {
        Self("noise_protocol.v1".to_string())
    }

    /// BLE transport capability
    pub fn ble_transport() -> Self {
        Self("transport.ble.v1".to_string())
    }

    /// Nostr transport capability
    pub fn nostr_transport() -> Self {
        Self("transport.nostr.v1".to_string())
    }
}

impl core::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Capability metadata with version and optional parameters
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Unique capability identifier
    pub id: CapabilityId,
    /// Capability version
    pub version: String,
    /// Optional parameters for this capability
    pub parameters: BTreeSet<String>,
}

impl Capability {
    /// Create a new capability
    pub fn new(id: CapabilityId, version: String) -> Self {
        Self {
            id,
            version,
            parameters: BTreeSet::new(),
        }
    }

    /// Add a parameter to this capability
    pub fn with_parameter(mut self, parameter: String) -> Self {
        self.parameters.insert(parameter);
        self
    }

    /// Check if this capability has a specific parameter
    pub fn has_parameter(&self, parameter: &str) -> bool {
        self.parameters.contains(parameter)
    }
}

/// Implementation information for capability negotiation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationInfo {
    /// Implementation name (e.g., "bitchat-rust", "bitchat-swift")
    pub name: String,
    /// Implementation version
    pub version: String,
    /// Platform information (optional)
    pub platform: Option<String>,
}

impl ImplementationInfo {
    /// Create new implementation info
    pub fn new(name: String, version: String) -> Self {
        Self {
            name,
            version,
            platform: None,
        }
    }

    /// Add platform information
    pub fn with_platform(mut self, platform: String) -> Self {
        self.platform = Some(platform);
        self
    }

    /// Create info for our Rust implementation
    pub fn bitchat_rust() -> Self {
        #[cfg(feature = "std")]
        {
            Self::new(
                "bitchat-rust".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )
            .with_platform(format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
        }
        #[cfg(not(feature = "std"))]
        {
            Self::new(
                "bitchat-rust".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )
        }
    }
}

// ----------------------------------------------------------------------------
// Capability Messages
// ----------------------------------------------------------------------------

/// Version hello message with capability announcement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionHello {
    /// Sending peer ID
    pub peer_id: PeerId,
    /// Supported protocol versions (in preference order)
    pub supported_versions: Vec<ProtocolVersion>,
    /// Announced capabilities
    pub capabilities: Vec<Capability>,
    /// Implementation information
    pub implementation: ImplementationInfo,
    /// Hello timestamp
    pub timestamp: Timestamp,
}

impl VersionHello {
    /// Create a new version hello message
    pub fn new(
        peer_id: PeerId,
        supported_versions: Vec<ProtocolVersion>,
        capabilities: Vec<Capability>,
        implementation: ImplementationInfo,
    ) -> Result<Self> {
        if capabilities.len() > MAX_CAPABILITIES {
            return Err(BitchatError::invalid_packet("Too many capabilities"));
        }

        Ok(Self {
            peer_id,
            supported_versions,
            capabilities,
            implementation,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a hello message with standard capabilities for our implementation
    pub fn standard(peer_id: PeerId) -> Result<Self> {
        let capabilities = vec![
            Capability::new(CapabilityId::core_messaging(), "1.0".to_string()),
            Capability::new(CapabilityId::noise_protocol(), "1.0".to_string()),
            Capability::new(CapabilityId::fragmentation(), "1.0".to_string()),
            Capability::new(CapabilityId::mesh_sync(), "1.0".to_string()),
            Capability::new(CapabilityId::location_channels(), "1.0".to_string()),
            Capability::new(CapabilityId::file_transfer(), "1.0".to_string()),
            Capability::new(CapabilityId::group_messaging(), "1.0".to_string()),
            Capability::new(CapabilityId::multi_device_sync(), "1.0".to_string()),
            Capability::new(CapabilityId::ble_transport(), "1.0".to_string()),
            Capability::new(CapabilityId::nostr_transport(), "1.0".to_string()),
        ];

        Self::new(
            peer_id,
            vec![ProtocolVersion::current()],
            capabilities,
            ImplementationInfo::bitchat_rust(),
        )
    }

    /// Get capabilities by ID
    pub fn get_capability(&self, id: &CapabilityId) -> Option<&Capability> {
        self.capabilities.iter().find(|cap| &cap.id == id)
    }

    /// Check if a capability is supported
    pub fn supports_capability(&self, id: &CapabilityId) -> bool {
        self.get_capability(id).is_some()
    }
}

/// Version acknowledgment with negotiated capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionAck {
    /// Responding peer ID
    pub peer_id: PeerId,
    /// Negotiated protocol version
    pub negotiated_version: ProtocolVersion,
    /// Mutually supported capabilities
    pub mutual_capabilities: Vec<Capability>,
    /// Implementation information
    pub implementation: ImplementationInfo,
    /// Ack timestamp
    pub timestamp: Timestamp,
}

impl VersionAck {
    /// Create a new version acknowledgment
    pub fn new(
        peer_id: PeerId,
        negotiated_version: ProtocolVersion,
        mutual_capabilities: Vec<Capability>,
        implementation: ImplementationInfo,
    ) -> Self {
        Self {
            peer_id,
            negotiated_version,
            mutual_capabilities,
            implementation,
            timestamp: Timestamp::now(),
        }
    }

    /// Check if a capability was negotiated
    pub fn supports_capability(&self, id: &CapabilityId) -> bool {
        self.mutual_capabilities.iter().any(|cap| &cap.id == id)
    }
}

/// Capability negotiation rejection with reason
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRejection {
    /// Rejecting peer ID
    pub peer_id: PeerId,
    /// Rejection reason
    pub reason: RejectionReason,
    /// Optional description
    pub description: Option<String>,
    /// Rejection timestamp
    pub timestamp: Timestamp,
}

impl CapabilityRejection {
    /// Create a new capability rejection
    pub fn new(peer_id: PeerId, reason: RejectionReason, description: Option<String>) -> Self {
        Self {
            peer_id,
            reason,
            description,
            timestamp: Timestamp::now(),
        }
    }
}

/// Reasons for capability negotiation rejection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    /// No compatible protocol version
    IncompatibleVersion,
    /// No common capabilities
    IncompatibleCapabilities,
    /// Too many capabilities requested
    TooManyCapabilities,
    /// Invalid capability format
    InvalidCapabilities,
    /// Negotiation timeout
    Timeout,
    /// Unknown error
    Unknown,
}

/// Status of capability negotiation with a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationStatus {
    /// Negotiation not yet started
    Unknown,
    /// Version hello sent, waiting for response
    Pending,
    /// Capabilities successfully negotiated
    Negotiated,
    /// Peer is legacy (canonical implementation) - no capability negotiation support
    Legacy,
}

// ----------------------------------------------------------------------------
// Capability Manager
// ----------------------------------------------------------------------------

/// Manages capability negotiation and peer compatibility
#[derive(Debug, Clone)]
pub struct CapabilityManager {
    /// Local peer ID
    local_peer_id: PeerId,
    /// Our supported capabilities
    local_capabilities: Vec<Capability>,
    /// Our implementation info
    implementation: ImplementationInfo,
    /// Negotiated capabilities with each peer
    peer_capabilities: std::collections::HashMap<PeerId, Vec<Capability>>,
    /// Negotiated protocol versions with each peer
    peer_versions: std::collections::HashMap<PeerId, ProtocolVersion>,
    /// Peers that don't support capability negotiation (canonical implementation)
    legacy_peers: std::collections::HashSet<PeerId>,
    /// Timeout tracking for version hello messages
    hello_timeouts: std::collections::HashMap<PeerId, Timestamp>,
}

impl CapabilityManager {
    /// Create a new capability manager
    pub fn new(local_peer_id: PeerId) -> Result<Self> {
        let hello = VersionHello::standard(local_peer_id)?;
        
        Ok(Self {
            local_peer_id,
            local_capabilities: hello.capabilities,
            implementation: hello.implementation,
            peer_capabilities: std::collections::HashMap::new(),
            peer_versions: std::collections::HashMap::new(),
            legacy_peers: std::collections::HashSet::new(),
            hello_timeouts: std::collections::HashMap::new(),
        })
    }

    /// Create a version hello message for this peer
    pub fn create_hello(&self) -> Result<VersionHello> {
        VersionHello::new(
            self.local_peer_id,
            vec![ProtocolVersion::current()],
            self.local_capabilities.clone(),
            self.implementation.clone(),
        )
    }

    /// Process an incoming version hello and create appropriate response
    pub fn process_hello(&mut self, hello: &VersionHello) -> Result<VersionAck> {
        // Find compatible protocol version
        let negotiated_version = self.negotiate_version(&hello.supported_versions)?;

        // Find mutual capabilities
        let mutual_capabilities = self.find_mutual_capabilities(&hello.capabilities);

        // Store negotiated state
        self.peer_capabilities.insert(hello.peer_id, mutual_capabilities.clone());
        self.peer_versions.insert(hello.peer_id, negotiated_version.clone());

        Ok(VersionAck::new(
            self.local_peer_id,
            negotiated_version,
            mutual_capabilities,
            self.implementation.clone(),
        ))
    }

    /// Process an incoming version ack
    pub fn process_ack(&mut self, ack: &VersionAck) -> Result<()> {
        // Store negotiated capabilities and version
        self.peer_capabilities.insert(ack.peer_id, ack.mutual_capabilities.clone());
        self.peer_versions.insert(ack.peer_id, ack.negotiated_version.clone());
        Ok(())
    }

    /// Check if a peer supports a specific capability
    pub fn peer_supports_capability(&self, peer_id: &PeerId, capability: &CapabilityId) -> bool {
        self.peer_capabilities
            .get(peer_id)
            .map(|caps| caps.iter().any(|cap| &cap.id == capability))
            .unwrap_or(false)
    }

    /// Get negotiated protocol version with a peer
    pub fn peer_protocol_version(&self, peer_id: &PeerId) -> Option<&ProtocolVersion> {
        self.peer_versions.get(peer_id)
    }

    /// Get all negotiated capabilities with a peer
    pub fn peer_capabilities(&self, peer_id: &PeerId) -> Option<&Vec<Capability>> {
        self.peer_capabilities.get(peer_id)
    }

    /// Check if we should enable a feature for communication with a peer
    pub fn should_use_feature(&self, peer_id: &PeerId, feature: &CapabilityId) -> bool {
        self.peer_supports_capability(peer_id, feature)
    }

    /// Remove peer from capability tracking
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peer_capabilities.remove(peer_id);
        self.peer_versions.remove(peer_id);
        self.legacy_peers.remove(peer_id);
        self.hello_timeouts.remove(peer_id);
    }

    /// Record that we sent a VersionHello to a peer and start timeout tracking
    pub fn track_hello_sent(&mut self, peer_id: PeerId) {
        let timeout = Timestamp::now() + CAPABILITY_TIMEOUT;
        self.hello_timeouts.insert(peer_id, timeout);
    }

    /// Check for peers that haven't responded to VersionHello and mark them as legacy
    pub fn check_hello_timeouts(&mut self) -> Vec<PeerId> {
        let now = Timestamp::now();
        let mut timed_out_peers = Vec::new();

        // Find peers that have timed out
        let expired_peers: Vec<PeerId> = self.hello_timeouts
            .iter()
            .filter(|(_, &timeout)| now > timeout)
            .map(|(&peer_id, _)| peer_id)
            .collect();

        for peer_id in expired_peers {
            // Move to legacy peers with core capabilities only
            self.mark_as_legacy_peer(peer_id);
            self.hello_timeouts.remove(&peer_id);
            timed_out_peers.push(peer_id);
        }

        timed_out_peers
    }

    /// Mark a peer as legacy (doesn't support capability negotiation)
    pub fn mark_as_legacy_peer(&mut self, peer_id: PeerId) {
        self.legacy_peers.insert(peer_id);
        
        // Assign core capabilities only (what canonical implementation supports)
        let core_capabilities = vec![
            Capability::new(CapabilityId::core_messaging(), "1.0".to_string()),
            Capability::new(CapabilityId::noise_protocol(), "1.0".to_string()),
            Capability::new(CapabilityId::fragmentation(), "1.0".to_string()),
            Capability::new(CapabilityId::location_channels(), "1.0".to_string()),
            Capability::new(CapabilityId::mesh_sync(), "1.0".to_string()),
            Capability::new(CapabilityId::ble_transport(), "1.0".to_string()),
            Capability::new(CapabilityId::nostr_transport(), "1.0".to_string()),
        ];
        
        self.peer_capabilities.insert(peer_id, core_capabilities);
        self.peer_versions.insert(peer_id, ProtocolVersion::current());
    }

    /// Check if a peer is marked as legacy (canonical implementation)
    pub fn is_legacy_peer(&self, peer_id: &PeerId) -> bool {
        self.legacy_peers.contains(peer_id)
    }

    /// Get the negotiation status for a peer
    pub fn get_negotiation_status(&self, peer_id: &PeerId) -> NegotiationStatus {
        if self.is_legacy_peer(peer_id) {
            NegotiationStatus::Legacy
        } else if self.peer_capabilities.contains_key(peer_id) {
            NegotiationStatus::Negotiated
        } else if self.hello_timeouts.contains_key(peer_id) {
            NegotiationStatus::Pending
        } else {
            NegotiationStatus::Unknown
        }
    }

    // Private helper methods

    fn negotiate_version(&self, peer_versions: &[ProtocolVersion]) -> Result<ProtocolVersion> {
        let our_version = ProtocolVersion::current();
        
        // Find the highest compatible version
        for peer_version in peer_versions {
            if our_version.is_compatible_with(peer_version) {
                return Ok(peer_version.clone());
            }
        }

        Err(BitchatError::invalid_packet("No compatible protocol version"))
    }

    fn find_mutual_capabilities(&self, peer_capabilities: &[Capability]) -> Vec<Capability> {
        let mut mutual = Vec::new();

        for peer_cap in peer_capabilities {
            // Find matching capability in our list
            if let Some(our_cap) = self.local_capabilities.iter().find(|cap| cap.id == peer_cap.id) {
                // Use the intersection of parameters
                let mutual_params: BTreeSet<String> = our_cap
                    .parameters
                    .intersection(&peer_cap.parameters)
                    .cloned()
                    .collect();

                let mut mutual_cap = Capability::new(peer_cap.id.clone(), peer_cap.version.clone());
                mutual_cap.parameters = mutual_params;
                mutual.push(mutual_cap);
            }
        }

        mutual
    }
}

// ----------------------------------------------------------------------------
// Capability Message Wrapper
// ----------------------------------------------------------------------------

/// All capability negotiation message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityMessage {
    /// Version hello with capabilities
    Hello(VersionHello),
    /// Version acknowledgment
    Ack(VersionAck),
    /// Capability rejection
    Rejection(CapabilityRejection),
}

impl CapabilityMessage {
    /// Get the peer ID for this message
    pub fn peer_id(&self) -> PeerId {
        match self {
            CapabilityMessage::Hello(hello) => hello.peer_id,
            CapabilityMessage::Ack(ack) => ack.peer_id,
            CapabilityMessage::Rejection(rejection) => rejection.peer_id,
        }
    }

    /// Get the corresponding NoisePayloadType for this message
    pub fn payload_type(&self) -> NoisePayloadType {
        match self {
            CapabilityMessage::Hello(_) => NoisePayloadType::VersionHello,
            CapabilityMessage::Ack(_) => NoisePayloadType::VersionAck,
            CapabilityMessage::Rejection(_) => NoisePayloadType::CapabilityRejection,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_compatibility() {
        let v1_0_0 = ProtocolVersion::new(1, 0, 0);
        let v1_1_0 = ProtocolVersion::new(1, 1, 0);
        let v2_0_0 = ProtocolVersion::new(2, 0, 0);

        assert!(v1_0_0.is_compatible_with(&v1_1_0));
        assert!(v1_1_0.is_compatible_with(&v1_0_0));
        assert!(!v1_0_0.is_compatible_with(&v2_0_0));
        assert!(!v2_0_0.is_compatible_with(&v1_0_0));

        assert!(v1_1_0.is_newer_than(&v1_0_0));
        assert!(!v1_0_0.is_newer_than(&v1_1_0));
        assert!(v2_0_0.is_newer_than(&v1_1_0));
    }

    #[test]
    fn test_capability_creation() {
        let cap = Capability::new(CapabilityId::file_transfer(), "1.0".to_string())
            .with_parameter("chunked".to_string())
            .with_parameter("sha256".to_string());

        assert_eq!(cap.id, CapabilityId::file_transfer());
        assert_eq!(cap.version, "1.0");
        assert!(cap.has_parameter("chunked"));
        assert!(cap.has_parameter("sha256"));
        assert!(!cap.has_parameter("md5"));
    }

    #[test]
    fn test_version_hello_creation() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let hello = VersionHello::standard(peer_id).unwrap();

        assert_eq!(hello.peer_id, peer_id);
        assert_eq!(hello.supported_versions.len(), 1);
        assert!(hello.supports_capability(&CapabilityId::core_messaging()));
        assert!(hello.supports_capability(&CapabilityId::file_transfer()));
        assert!(hello.supports_capability(&CapabilityId::group_messaging()));
    }

    #[test]
    fn test_capability_manager() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let remote_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Create a hello from remote peer with limited capabilities
        let remote_hello = VersionHello::new(
            remote_peer,
            vec![ProtocolVersion::current()],
            vec![
                Capability::new(CapabilityId::core_messaging(), "1.0".to_string()),
                Capability::new(CapabilityId::noise_protocol(), "1.0".to_string()),
            ],
            ImplementationInfo::new("test-impl".to_string(), "1.0".to_string()),
        ).unwrap();

        // Process the hello
        let ack = manager.process_hello(&remote_hello).unwrap();

        // Should have negotiated only common capabilities
        assert_eq!(ack.mutual_capabilities.len(), 2);
        assert!(manager.peer_supports_capability(&remote_peer, &CapabilityId::core_messaging()));
        assert!(!manager.peer_supports_capability(&remote_peer, &CapabilityId::file_transfer()));
    }

    #[test]
    fn test_feature_compatibility_check() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let remote_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Simulate negotiation with canonical implementation (no advanced features)
        let canonical_hello = VersionHello::new(
            remote_peer,
            vec![ProtocolVersion::current()],
            vec![
                Capability::new(CapabilityId::core_messaging(), "1.0".to_string()),
                Capability::new(CapabilityId::noise_protocol(), "1.0".to_string()),
                Capability::new(CapabilityId::fragmentation(), "1.0".to_string()),
                Capability::new(CapabilityId::location_channels(), "1.0".to_string()),
            ],
            ImplementationInfo::new("bitchat-swift".to_string(), "1.0".to_string()),
        ).unwrap();

        manager.process_hello(&canonical_hello).unwrap();

        // Check feature availability
        assert!(manager.should_use_feature(&remote_peer, &CapabilityId::core_messaging()));
        assert!(manager.should_use_feature(&remote_peer, &CapabilityId::location_channels()));
        assert!(!manager.should_use_feature(&remote_peer, &CapabilityId::file_transfer()));
        assert!(!manager.should_use_feature(&remote_peer, &CapabilityId::group_messaging()));
        assert!(!manager.should_use_feature(&remote_peer, &CapabilityId::multi_device_sync()));
    }

    #[test]
    fn test_legacy_peer_detection() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let legacy_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Initially unknown
        assert_eq!(manager.get_negotiation_status(&legacy_peer), NegotiationStatus::Unknown);

        // Send hello and track timeout
        manager.track_hello_sent(legacy_peer);
        assert_eq!(manager.get_negotiation_status(&legacy_peer), NegotiationStatus::Pending);

        // Mark as legacy
        manager.mark_as_legacy_peer(legacy_peer);
        assert_eq!(manager.get_negotiation_status(&legacy_peer), NegotiationStatus::Legacy);
        assert!(manager.is_legacy_peer(&legacy_peer));

        // Should have core capabilities only
        let caps = manager.peer_capabilities(&legacy_peer).unwrap();
        assert_eq!(caps.len(), 7); // Core capabilities
        assert!(manager.peer_supports_capability(&legacy_peer, &CapabilityId::core_messaging()));
        assert!(manager.peer_supports_capability(&legacy_peer, &CapabilityId::location_channels()));
        assert!(!manager.peer_supports_capability(&legacy_peer, &CapabilityId::file_transfer()));
        assert!(!manager.peer_supports_capability(&legacy_peer, &CapabilityId::group_messaging()));
        assert!(!manager.peer_supports_capability(&legacy_peer, &CapabilityId::multi_device_sync()));
    }

    #[test]
    fn test_hello_timeout_handling() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let timeout_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Track hello sent
        manager.track_hello_sent(timeout_peer);
        assert_eq!(manager.get_negotiation_status(&timeout_peer), NegotiationStatus::Pending);

        // Simulate timeout by marking as legacy directly (in real use, check_hello_timeouts would do this)
        manager.mark_as_legacy_peer(timeout_peer);
        
        // Should now be legacy with core capabilities
        assert_eq!(manager.get_negotiation_status(&timeout_peer), NegotiationStatus::Legacy);
        assert!(manager.peer_supports_capability(&timeout_peer, &CapabilityId::core_messaging()));
        assert!(!manager.peer_supports_capability(&timeout_peer, &CapabilityId::file_transfer()));
    }

    #[test]
    fn test_graceful_degradation_with_canonical_implementation() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let canonical_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Simulate canonical implementation that doesn't respond to VersionHello
        manager.track_hello_sent(canonical_peer);
        manager.mark_as_legacy_peer(canonical_peer); // This would happen after timeout

        // Verify graceful degradation
        assert!(manager.is_legacy_peer(&canonical_peer));
        assert_eq!(manager.get_negotiation_status(&canonical_peer), NegotiationStatus::Legacy);

        // Can use core features
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::core_messaging()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::noise_protocol()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::fragmentation()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::location_channels()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::mesh_sync()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::ble_transport()));
        assert!(manager.should_use_feature(&canonical_peer, &CapabilityId::nostr_transport()));

        // Cannot use advanced features
        assert!(!manager.should_use_feature(&canonical_peer, &CapabilityId::file_transfer()));
        assert!(!manager.should_use_feature(&canonical_peer, &CapabilityId::group_messaging()));
        assert!(!manager.should_use_feature(&canonical_peer, &CapabilityId::multi_device_sync()));
    }

    #[test]
    fn test_negotiated_vs_legacy_peer_comparison() {
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let enhanced_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
        let legacy_peer = PeerId::new([17, 18, 19, 20, 21, 22, 23, 24]);
        
        let mut manager = CapabilityManager::new(peer_id).unwrap();

        // Enhanced peer with full capabilities
        let enhanced_hello = VersionHello::standard(enhanced_peer).unwrap();
        manager.process_hello(&enhanced_hello).unwrap();

        // Legacy peer with no capability negotiation
        manager.mark_as_legacy_peer(legacy_peer);

        // Compare capabilities
        assert_eq!(manager.get_negotiation_status(&enhanced_peer), NegotiationStatus::Negotiated);
        assert_eq!(manager.get_negotiation_status(&legacy_peer), NegotiationStatus::Legacy);

        // Enhanced peer has all features
        assert!(manager.should_use_feature(&enhanced_peer, &CapabilityId::file_transfer()));
        assert!(manager.should_use_feature(&enhanced_peer, &CapabilityId::group_messaging()));
        assert!(manager.should_use_feature(&enhanced_peer, &CapabilityId::multi_device_sync()));

        // Legacy peer has only core features
        assert!(!manager.should_use_feature(&legacy_peer, &CapabilityId::file_transfer()));
        assert!(!manager.should_use_feature(&legacy_peer, &CapabilityId::group_messaging()));
        assert!(!manager.should_use_feature(&legacy_peer, &CapabilityId::multi_device_sync()));

        // Both have core features
        assert!(manager.should_use_feature(&enhanced_peer, &CapabilityId::core_messaging()));
        assert!(manager.should_use_feature(&legacy_peer, &CapabilityId::core_messaging()));
    }
}