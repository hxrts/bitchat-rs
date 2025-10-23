# BitChat Feature Roadmap

This document outlines the remaining feature implementation plan for the BitChat Rust library, designed to provide a lean, secure, cross-platform messaging protocol suitable for integration into mobile, desktop, and web applications.

## Feature Implementation Status Overview

| Feature | Tier | Status | Implementation Location | Notes |
|---------|------|--------|------------------------|-------|
| QR-based Peer Verification | 1 | COMPLETE | `verification.rs` | Cryptographic identity verification |
| Gossip Synchronization with GCS Filters | 2 | NOT_STARTED | - | Efficient mesh sync |
| Enhanced Session Management | 2 | PARTIAL | `session.rs` | Basic sessions exist, needs enhancement |
| Comprehensive Security Validation | 2 | NOT_STARTED | - | Crypto parameter validation |
| Basic Keychain Abstraction | 3 | NOT_STARTED | - | Platform-specific secure storage |
| Configurable Transport Parameters | 3 | PARTIAL | `config.rs` | Basic config exists, needs expansion |

## Feature Tiers

### Tier 1: Critical Core Protocol Features

#### 1. QR-based Peer Verification
**Purpose**: Cryptographic peer identity verification through QR code exchange.

**Implementation Criteria**:
- Challenge-response verification protocol
- Ed25519 signature-based proof of identity
- Time-limited verification tokens
- Anti-replay protection

**Key Data Structures**:
```rust
pub struct VerificationService {
    pending_challenges: HashMap<PeerId, PendingChallenge>,
    config: VerificationConfig,
}

pub struct VerificationQR {
    pub version: u8,
    pub noise_public_key: PublicKey,
    pub signing_public_key: VerifyingKey,  // PUBLIC key only - never expose private keys!
    pub nickname: String,
    pub timestamp: Timestamp,
    pub nonce: [u8; 32],
    pub signature: Signature,  // Self-signed proof of private key ownership
}

pub struct PendingChallenge {
    pub nonce_a: [u8; 32],
    pub nonce_b: [u8; 32],
    pub expires_at: Instant,
}
```

**Interoperability**: QR data format compatible with external QR libraries.

### Tier 2: Important Transport Features

#### 2. Gossip Synchronization with GCS Filters
**Purpose**: Efficient message synchronization between mesh peers.

**Implementation Criteria**:
- Golomb-Coded Sets (GCS) for compact message filters
- On-demand synchronization with configurable intervals
- Stale peer detection and cleanup
- Bandwidth-efficient sync protocol

**Key Data Structures**:
```rust
pub struct GossipSyncManager {
    seen_messages: BTreeMap<Timestamp, MessageId>,
    peer_filters: HashMap<PeerId, PeerSyncState>,
    config: GossipConfig,
}

pub struct GCSFilter {
    filter_data: Vec<u8>,
    false_positive_rate: f64,
    element_count: u32,
}

pub struct SyncRequest {
    pub peer_id: PeerId,
    pub filter: GCSFilter,
    pub timestamp_range: (Timestamp, Timestamp),
}
```

**Interoperability**: Standard GCS implementation for cross-platform compatibility.

#### 3. Enhanced Session Management
**Purpose**: Robust session lifecycle with automatic recovery and rekey.

**Implementation Criteria**:
- Automatic session rekey based on time/message count
- Session state persistence across restarts
- Dead session detection and cleanup
- Graceful session migration

**Key Data Structures**:
```rust
pub struct SessionManager {
    sessions: HashMap<PeerId, NoiseSession>,
    rekey_scheduler: RekeyScheduler,
    persistence: Box<dyn SessionPersistence>,
}

pub struct SessionMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
    pub rekey_count: u32,
}

pub trait SessionPersistence {
    fn save_session(&self, peer_id: &PeerId, session: &NoiseSession) -> Result<()>;
    fn load_session(&self, peer_id: &PeerId) -> Result<Option<NoiseSession>>;
    fn delete_session(&self, peer_id: &PeerId) -> Result<()>;
}
```

**Interoperability**: Pluggable persistence with platform-specific implementations.

#### 4. Comprehensive Security Validation
**Purpose**: Validate cryptographic parameters and detect security violations.

**Implementation Criteria**:
- Key strength validation
- Protocol parameter verification
- Anomaly detection for unusual patterns
- Security event logging

**Key Data Structures**:
```rust
pub struct SecurityValidator {
    key_validator: KeyValidator,
    protocol_validator: ProtocolValidator,
    anomaly_detector: AnomalyDetector,
}

pub struct SecurityEvent {
    pub severity: SecuritySeverity,
    pub event_type: SecurityEventType,
    pub peer_id: Option<PeerId>,
    pub timestamp: Timestamp,
    pub details: String,
}

pub enum SecuritySeverity {
    Info,
    Warning,
    Critical,
}
```

**Interoperability**: Event-driven security monitoring with configurable thresholds.

### Tier 3: Nice-to-Have Library Features

#### 5. Basic Keychain Abstraction
**Purpose**: Secure key storage interface for platform-specific implementations.

**Implementation Criteria**:
- Cross-platform key storage trait
- Encryption key derivation
- Biometric authentication support (platform-dependent)
- Key rotation and backup

**Key Data Structures**:
```rust
pub trait SecureStorage {
    fn store_key(&self, key_id: &str, key_data: &[u8]) -> Result<()>;
    fn retrieve_key(&self, key_id: &str) -> Result<Option<Vec<u8>>>;
    fn delete_key(&self, key_id: &str) -> Result<()>;
    fn list_keys(&self) -> Result<Vec<String>>;
}

pub struct KeychainManager {
    storage: Box<dyn SecureStorage>,
    encryption_key: Option<[u8; 32]>,
}
```

**Interoperability**: Platform-specific implementations for iOS Keychain, Android Keystore, etc.

#### 6. Configurable Transport Parameters
**Purpose**: Extend existing configuration system with canonical parameter compatibility and advanced configuration features.

**Current Status**: The Rust implementation already has a robust configuration system using `CliAppConfig` with figment-based loading from TOML files, environment variables, and command line arguments. This feature extends it with canonical compatibility and additional parameters.

**Implementation Criteria**:
- Extend existing `CliAppConfig` with canonical TransportConfig parameters
- Add configuration validation against canonical ranges
- Implement configuration presets (development, production, battery-optimized)
- Add runtime configuration updates for select parameters
- Maintain backward compatibility with existing configuration system

**Enhanced Configuration Structure**:
```rust
// Extend existing config.rs with canonical parameters
pub struct BitchatConfig {
    pub core: CoreConfig,
    pub ble: BleTransportConfig,
    pub nostr: NostrConfig,
    pub runtime: RuntimeConfig,
    pub identity: IdentityConfig,
    // New canonical-compatible sections
    pub limits: LimitsConfig,
    pub timing: TimingConfig,
    pub ui: UiConfig,
}

// Canonical BLE parameters from canonical TransportConfig
pub struct BleTransportConfig {
    // Existing parameters
    pub max_packet_size: usize,
    pub connection_timeout: Duration,
    pub scan_timeout: Duration,
    pub device_name_prefix: String,
    
    // New canonical parameters
    pub fragment_size: usize,                    // bleDefaultFragmentSize: 469
    pub max_central_links: usize,                // bleMaxCentralLinks: 6
    pub connect_rate_limit_interval: Duration,   // bleConnectRateLimitInterval: 0.5s
    pub duty_on_duration: Duration,              // bleDutyOnDuration: 5.0s
    pub duty_off_duration: Duration,             // bleDutyOffDuration: 10.0s
    pub announce_min_interval: Duration,         // bleAnnounceMinInterval: 1.0s
    pub dynamic_rssi_threshold: i32,             // bleDynamicRSSIThresholdDefault: -90
    pub connection_candidates_max: usize,        // bleConnectionCandidatesMax: 100
    pub pending_write_buffer_cap: usize,         // blePendingWriteBufferCapBytes: 1M
    pub pending_notifications_cap: usize,        // blePendingNotificationsCapCount: 20
    pub maintenance_interval: Duration,          // bleMaintenanceInterval: 5.0s
    pub peer_inactivity_timeout: Duration,       // blePeerInactivityTimeoutSeconds: 8.0s
    pub max_in_flight_assemblies: usize,         // bleMaxInFlightAssemblies: 128
    pub high_degree_threshold: usize,            // bleHighDegreeThreshold: 6
}

// Configuration presets for different deployment scenarios
pub enum ConfigPreset {
    Development,    // Faster timeouts, more verbose logging
    Production,     // Canonical timing values, optimal performance
    BatteryOptimized, // Longer intervals, reduced scanning
    Testing,        // Short timeouts, deterministic behavior
}
```

**Implementation Integration**:
- Build upon existing `crates/bitchat-core/src/config.rs` 
- Extend `CliAppConfig` struct with new canonical sections
- Maintain existing figment-based loading with TOML/env/CLI priority
- Add preset loading: `BitchatConfig::load_with_preset(ConfigPreset::Production)`
- Integrate with existing validation in `CliAppConfig::validate()`

**Canonical Compatibility**: All parameter names and default values match the canonical Swift implementation's `TransportConfig` enum, ensuring wire-level compatibility and consistent behavior across implementations.

**Migration Path**: Existing `bitchat.toml` files remain valid; new canonical parameters use sensible defaults if not specified.

## Feature Flags Architecture

```toml
[features]
default = ["core", "gossip-sync"]

# Core protocol (always included)
core = []

# Tier 1 features (critical for MVP)
qr-verification = ["core", "qr"]

# Tier 2 features
gossip-sync = ["core", "gcs"]
session-recovery = ["core"]
security-validation = ["core"]

# Tier 3 features
keychain-abstraction = ["core"]
configurable-params = ["core", "serde"]

# Convenience bundles
full = [
    "qr-verification", "gossip-sync", "session-recovery", 
    "security-validation", "keychain-abstraction", "configurable-params"
]
minimal = ["core", "gossip-sync"]
security-focused = ["core", "qr-verification", "security-validation"]
```

## Implementation Priority

1. **Phase 1**: Implement Tier 1 features (critical core functionality)
2. **Phase 2**: Add Tier 2 features (transport enhancements)
3. **Phase 3**: Implement Tier 3 features (convenience and configurability)
4. **Phase 4**: Optimization and real-world testing

## Testing Strategy

Each feature must include:
- Unit tests for core functionality
- Integration tests with other features
- Performance benchmarks
- Security test vectors (where applicable)
- Cross-platform compatibility tests

## Documentation Requirements

Each feature requires:
- API documentation with examples
- Security considerations
- Performance characteristics
- Platform-specific integration notes
- Migration guides for breaking changes

## Critical Implementation Notes

### Code Reconciliation Required
Before implementing features from this roadmap, perform a detailed reconciliation with the existing refactored bitchat-core codebase:

1. **Config System**: Feature #6 (Configurable Transport Parameters) overlaps significantly with the existing `config.rs` module and `BitchatConfig` struct. Update roadmap to build upon existing implementation rather than replacing it.

2. **Transport Architecture**: Ensure all transport features align with the existing CSP-based `TransportTask` trait and channel architecture.

3. **Session Management**: Coordinate with existing session handling in the current codebase.

### Security Audit Required
- **QR Verification**: The corrected `VerificationQR` structure now properly uses `VerifyingKey` (public key) instead of `SigningKey` (private key)
- **All cryptographic features**: Must undergo security review before implementation

### Application Boundary Enforcement
- **Command System**: Moved to bitchat-cli scope - keep bitchat-core UI-agnostic
- **Platform Integration**: All OS-specific features belong in consuming applications

This roadmap ensures the BitChat library remains focused, secure, and suitable for cross-platform integration while providing the essential features needed for robust peer-to-peer messaging applications.

## Completed Features

The following features have been successfully implemented and are production-ready:

### ✅ Basic Transport Failover (Tier 1) 
- **Location**: `transport/failover.rs`
- **Status**: Complete canonical-compatible implementation
- **Features**: BLE-first routing, transport health monitoring, peer reachability tracking

### ✅ BitChat Tunneling Through Nostr (Tier 1)
- **Location**: `bitchat-nostr` crate  
- **Status**: Complete canonical-compatible implementation
- **Features**: `bitchat1:` embedding, relay selection, NIP-17 gift wrapping

### ✅ Message Padding & Anti-Fingerprinting (Tier 2)
- **Location**: `bitchat-nostr/embedding.rs`
- **Status**: Complete implementation integrated with Nostr tunneling
- **Features**: Variable padding, timing jitter, traffic analysis resistance

### ✅ Advanced Transport Failover Logic (Tier 2)
- **Location**: `transport/advanced_failover.rs`, `transport/integration.rs`
- **Status**: Complete implementation with canonical health monitoring
- **Features**: Performance-based routing, message queuing, health scoring, CSP integration

### ✅ QR-based Peer Verification (Tier 1)
- **Location**: `verification.rs`
- **Status**: Complete canonical-compatible implementation
- **Features**: Ed25519 signature-based verification, challenge-response protocol, QR code generation, anti-replay protection