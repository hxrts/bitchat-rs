# BitChat Library Features Roadmap

This document outlines the feature implementation plan for the BitChat Rust library, designed to provide a lean, secure, cross-platform messaging protocol suitable for integration into mobile, desktop, and web applications.

## Design Philosophy

The BitChat library focuses on **protocol-level features** that provide security, reliability, and performance benefits while avoiding platform-specific implementations. Applications built on top of the library handle presentation, UI, platform integration, and user experience concerns.

## Feature Tiers

### Tier 1: Critical Core Protocol Features

#### 1. Message Deduplication
**Purpose**: Prevent duplicate message processing in mesh networking environments.

**Implementation Criteria**:
- Time-based and content-based duplicate detection
- Configurable retention window (default: 5 minutes)
- Memory-bounded storage with LRU eviction
- Hash-based message fingerprinting

**Key Data Structures**:
```rust
pub struct MessageDeduplicator {
    seen_messages: LruCache<MessageHash, Timestamp>,
    config: DeduplicationConfig,
}

pub struct DeduplicationConfig {
    pub max_age_seconds: u64,
    pub max_entries: usize,
    pub hash_algorithm: HashAlgorithm,
}

pub struct MessageHash([u8; 32]);
```

**Interoperability**: Trait-based interface allowing custom deduplication strategies.

#### 2. Read Receipts & Delivery Acknowledgments
**Purpose**: Provide reliable message delivery confirmation and read status tracking.

**Implementation Criteria**:
- Delivery acknowledgments for all private messages
- Read receipts with privacy controls
- Automatic retry with exponential backoff
- Rate limiting to prevent relay spam

**Key Data Structures**:
```rust
pub struct ReadReceipt {
    pub message_id: MessageId,
    pub peer_id: PeerId,
    pub timestamp: Timestamp,
    pub receipt_type: ReceiptType,
}

pub enum ReceiptType {
    Delivered,
    Read,
}

pub struct DeliveryTracker {
    pending_acks: HashMap<MessageId, PendingAck>,
    retry_queue: VecDeque<RetryEntry>,
}
```

**Interoperability**: Event-driven interface with configurable privacy settings.

#### 3. Rate Limiting (Noise Protocol)
**Purpose**: Prevent DoS attacks and ensure fair resource usage.

**Implementation Criteria**:
- Per-peer handshake rate limiting (max/minute)
- Global and per-peer message rate limiting (max/second)
- Token bucket algorithm with configurable refill rates
- Security logging for rate limit violations

**Key Data Structures**:
```rust
pub struct NoiseRateLimiter {
    handshake_buckets: HashMap<PeerId, TokenBucket>,
    message_buckets: HashMap<PeerId, TokenBucket>,
    global_limits: GlobalRateLimits,
}

pub struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_rate: f64,
}

pub struct RateLimitConfig {
    pub max_handshakes_per_minute: u32,
    pub max_messages_per_second: u32,
    pub global_handshake_limit: u32,
    pub global_message_limit: u32,
}
```

**Interoperability**: Pluggable rate limiting with custom policy support.

#### 4. Fragment Reassembly with TTL
**Purpose**: Reliable transmission of large messages over MTU-limited transports.

**Implementation Criteria**:
- Automatic fragmentation for messages exceeding transport MTU
- Out-of-order fragment reassembly
- TTL-based fragment expiration
- Memory-bounded fragment storage

**Key Data Structures**:
```rust
pub struct FragmentAssembler {
    assemblies: HashMap<FragmentKey, FragmentAssembly>,
    config: FragmentConfig,
}

pub struct FragmentAssembly {
    fragments: BTreeMap<u32, Fragment>,
    total_fragments: u32,
    expires_at: Instant,
    bytes_received: usize,
}

pub struct FragmentConfig {
    pub max_message_size: usize,
    pub fragment_ttl_seconds: u64,
    pub max_concurrent_assemblies: usize,
}
```

**Interoperability**: Transport-agnostic with configurable MTU sizes.

#### 5. QR-based Peer Verification
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

#### 6. Basic Transport Failover
**Purpose**: Essential hybrid transport capability - prefer BLE, fallback to Nostr.

**Implementation Criteria**:
- Simple transport priority ordering (BLE primary, Nostr fallback)
- Basic transport health detection (connection success/failure)
- Message routing to available transports
- Essential for MVP hybrid P2P/relay functionality

**Key Data Structures**:
```rust
pub struct BasicTransportManager {
    transports: Vec<Box<dyn TransportTask>>,
    primary_transport: TransportType,
    fallback_transport: TransportType,
    routing_strategy: BasicRoutingStrategy,
}

pub enum BasicRoutingStrategy {
    PreferPrimary,        // Always try primary first
    LoadBalance,          // Round-robin between available
    BroadcastAll,         // Send via all available transports
}

pub struct TransportStatus {
    pub transport_type: TransportType,
    pub is_available: bool,
    pub last_success: Option<Instant>,
    pub consecutive_failures: u32,
}
```

**Interoperability**: Foundation for advanced transport management, upgradeable to full health monitoring.

#### 7. BitChat Tunneling Through Nostr
**Purpose**: Enable BitChat protocol operation over Nostr relays for cross-platform reach and transport resilience.

**Implementation Criteria**:
- Multiple embedding strategies (private DMs, public events, custom kinds)
- Privacy-conscious design with configurable tradeoffs
- Relay health monitoring and automatic selection
- Integration with transport failover logic
- Store-and-forward capability for offline peers

**Key Data Structures**:
```rust
pub struct NostrTunneledTransport {
    relay_client: NostrClient,
    embedding_strategy: EmbeddingStrategy,
    privacy_config: PrivacyConfig,
    relay_selector: RelaySelector,
}

pub enum EmbeddingStrategy {
    PrivateMessage,   // Embed in NIP-04/NIP-44 encrypted DMs
    PublicEvent,      // Embed in public events (less private, more reach)
    CustomKind(u16),  // Use custom Nostr event kind
}

pub struct PrivacyConfig {
    pub use_tor: bool,
    pub relay_rotation_interval: Duration,
    pub message_padding: bool,
    pub timing_randomization: bool,
}

pub struct RelaySelector {
    relays: Vec<RelayInfo>,
    health_monitor: RelayHealthMonitor,
    selection_strategy: SelectionStrategy,
}

pub struct RelayInfo {
    pub url: String,
    pub health: RelayHealth,
    pub capabilities: RelayCapabilities,
    pub privacy_score: f64,
}
```

**Interoperability**: 
- Standard Nostr protocol compliance (NIPs 01, 04, 44)
- Cross-platform relay connectivity
- Pluggable embedding strategies for different privacy/reach tradeoffs

**Privacy Considerations**:
- Metadata leakage (timing, message sizes, IP addresses)
- Traffic analysis resistance through padding and timing randomization
- Relay logging and surveillance mitigation
- Optional Tor integration for enhanced anonymity

**Benefits**:
- Web browser compatibility (no BLE peripheral mode required)
- Corporate firewall traversal
- Offline message delivery via store-and-forward
- Global peer discovery and bootstrapping
- Transport redundancy and automatic failover

### Tier 2: Important Transport Features

#### 8. Gossip Synchronization with GCS Filters
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

#### 9. Message Padding & Anti-Fingerprinting
**Purpose**: Protect against traffic analysis and timing attacks.

**Implementation Criteria**:
- Variable-length message padding
- Configurable padding strategies
- Timing randomization for message transmission
- Traffic pattern obfuscation

**Key Data Structures**:
```rust
pub struct MessagePadding {
    strategy: PaddingStrategy,
    config: PaddingConfig,
}

pub enum PaddingStrategy {
    Fixed(usize),
    Random { min: usize, max: usize },
    PowerOfTwo,
    Custom(Box<dyn PaddingFunction>),
}

pub struct PaddingConfig {
    pub enabled: bool,
    pub max_padding_bytes: usize,
    pub timing_jitter_ms: Range<u64>,
}
```

**Interoperability**: Pluggable padding strategies for different threat models.

#### 10. Enhanced Session Management
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

#### 11. Advanced Transport Failover Logic
**Purpose**: Intelligent routing between multiple transport protocols.

**Implementation Criteria**:
- Transport health monitoring and scoring
- Automatic failover based on performance metrics
- Message queuing during transport transitions
- Load balancing across available transports

**Key Data Structures**:
```rust
pub struct TransportManager {
    transports: HashMap<TransportType, Box<dyn Transport>>,
    routing_table: RoutingTable,
    health_monitor: TransportHealthMonitor,
}

pub struct TransportHealth {
    pub latency_ms: Option<u64>,
    pub success_rate: f64,
    pub last_failure: Option<Instant>,
    pub consecutive_failures: u32,
}

pub struct RoutingRule {
    pub peer_id: Option<PeerId>,
    pub message_type: Option<MessageType>,
    pub preferred_transport: TransportType,
    pub fallback_transports: Vec<TransportType>,
}
```

**Interoperability**: Transport-agnostic interface with pluggable transport implementations.

#### 12. Comprehensive Security Validation
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

#### 13. Basic Keychain Abstraction
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

#### 14. Configurable Transport Parameters
**Purpose**: Centralized configuration for all transport and protocol parameters.

**Implementation Criteria**:
- Compile-time and runtime configuration
- Environment variable override support
- Validation of parameter ranges
- Performance tuning presets

**Key Data Structures**:
```rust
pub struct BitchatConfig {
    pub transport: TransportConfig,
    pub security: SecurityConfig,
    pub performance: PerformanceConfig,
    pub features: FeatureConfig,
}

pub struct TransportConfig {
    pub ble_fragment_size: usize,
    pub message_ttl: u8,
    pub max_concurrent_connections: usize,
    pub announce_interval: Duration,
}

pub trait ConfigSource {
    fn load_config(&self) -> Result<BitchatConfig>;
    fn save_config(&self, config: &BitchatConfig) -> Result<()>;
}
```

**Interoperability**: Multiple configuration sources (files, environment, command line).

## Feature Flags Architecture

```toml
[features]
default = ["core", "dedup", "receipts", "rate-limiting", "basic-transport-failover", "nostr-tunneling"]

# Core protocol (always included)
core = []

# Tier 1 features (critical for MVP)
dedup = ["core"]
receipts = ["core"] 
rate-limiting = ["core"]
fragmentation = ["core"]
qr-verification = ["core", "qr"]
basic-transport-failover = ["core"]  # Essential hybrid capability
nostr-tunneling = ["core", "nostr"]  # Critical for cross-platform reach

# Tier 2 features
gossip-sync = ["core", "gcs"]
anti-fingerprinting = ["core"]
session-recovery = ["core"]
advanced-transport-failover = ["core", "basic-transport-failover"]
security-validation = ["core"]

# Tier 3 features
keychain-abstraction = ["core"]
configurable-params = ["core", "serde"]

# Convenience bundles
full = [
    "dedup", "receipts", "rate-limiting", "fragmentation", "qr-verification", "basic-transport-failover", "nostr-tunneling",
    "gossip-sync", "anti-fingerprinting", "session-recovery", "advanced-transport-failover",
    "security-validation", "keychain-abstraction", "configurable-params"
]
minimal = ["core", "dedup", "receipts", "basic-transport-failover", "nostr-tunneling"]
security-focused = ["core", "rate-limiting", "qr-verification", "anti-fingerprinting", "security-validation"]
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

1. **Config System**: Feature #13 (Configurable Transport Parameters) overlaps significantly with the existing `config.rs` module and `BitchatConfig` struct. Update roadmap to build upon existing implementation rather than replacing it.

2. **Delivery Tracking**: Feature #2 (Read Receipts & Delivery Acknowledgments) should integrate with the existing `delivery.rs` module rather than creating a separate `DeliveryTracker`.

3. **Transport Architecture**: Ensure all transport features align with the existing CSP-based `TransportTask` trait and channel architecture.

4. **Session Management**: Coordinate with existing session handling in the current codebase.

### Security Audit Required
- **QR Verification**: The corrected `VerificationQR` structure now properly uses `VerifyingKey` (public key) instead of `SigningKey` (private key)
- **All cryptographic features**: Must undergo security review before implementation

### Application Boundary Enforcement
- **Command System**: Moved to bitchat-cli scope - keep bitchat-core UI-agnostic
- **Platform Integration**: All OS-specific features belong in consuming applications

This roadmap ensures the BitChat library remains focused, secure, and suitable for cross-platform integration while providing the essential features needed for robust peer-to-peer messaging applications.