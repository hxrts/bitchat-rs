//! BitChat Core Protocol Implementation
//!
//! This crate provides the foundational types, cryptographic primitives, and serialization
//! for the BitChat peer-to-peer messaging protocol. It is designed to be `no_std` compatible
//! and work across both native and WebAssembly targets.
//!
//! ## Feature Flags
//!
//! ### Core Features (Always Available)
//! The default build includes only features that are compatible with the canonical Swift implementation:
//! - Core messaging and encryption (Noise Protocol XX)
//! - Message fragmentation and deduplication
//! - Location-based channels and mesh networking
//! - BLE and Nostr transport abstractions
//!
//! ### Experimental Features (`experimental` flag)
//! Advanced features that exceed the canonical implementation can be enabled with the `experimental` feature flag:
//! - **File Transfer**: Secure chunked file transfer with integrity verification
//! - **Group Messaging**: Role-based group chat management
//! - **Multi-Device Sync**: Cross-device session synchronization
//! - **Capability Negotiation**: Automatic feature detection and graceful degradation
//!
//! ```bash
//! # Build with canonical compatibility only
//! cargo build
//!
//! # Build with all experimental features
//! cargo build --features experimental
//! ```
//!
//! ## Architecture Overview
//!
//! BitChat follows a clean architecture with clear separation of concerns across multiple crates:
//!
//! ### bitchat-core: The Headless Engine
//!
//! This crate is the "brain" of the application - a completely self-contained library with no
//! knowledge of any user interface. It can be run from a CLI, GUI, web server, or test harness
//! without any changes.
//!
//! **Responsibilities:**
//! - **Public API**: The [`channel`] module defines the communication protocol (Command, Event, Effect, AppEvent)
//! - **Core Logic**: The [`logic`] module owns and manages all application state and business logic
//! - **Protocol Implementation**: The [`protocol`] module handles cryptography, sessions, and message delivery
//! - **Network Abstraction**: The [`transport_task`] module defines interfaces for network transports
//! - **Runtime Orchestration**: The [`runtime`] module wires all components together generically
//!
//! ### Transport Crates: The Network Connectors
//!
//! Separate crates like `bitchat-ble` and `bitchat-nostr` implement the [`TransportTask`] trait
//! to handle specific network protocols.
//!
//! ### Frontend Crates: The User Interfaces
//!
//! Crates like `bitchat-cli` provide user interfaces by:
//! - Translating user input into [`Command`]s
//! - Receiving [`AppEvent`]s and updating the UI
//! - Managing UI-specific state and rendering
//!
//! ## Usage Examples
//!
//! ### Basic Types and Configuration
//!
//! BitChat Core provides ergonomic APIs that accept flexible input types:
//!
//! ```rust
//! use bitchat_core::{PeerId, config::BitchatConfig};
//!
//! // Create PeerId from hex string (flexible input)
//! let peer_id: PeerId = "abcdef1234567890".parse().unwrap();
//!
//! // Or from 0x-prefixed hex
//! let peer_id2: PeerId = "0x1234567890abcdef".parse().unwrap();
//!
//! // Access underlying bytes via Deref
//! println!("Peer ID bytes: {:?}", &peer_id[..]);
//!
//! // Efficient configuration sharing with Arc
//! let shared_config = BitchatConfig::shared_browser_optimized();
//!
//! // Clone the Arc (cheap operation) to share across tasks
//! let config_for_task = shared_config.clone();
//! ```
//!
//! ### Runtime Integration
//!
//! ```rust,no_run
//! use bitchat_core::{PeerId, Command, AppEvent};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a peer ID from various formats
//! let peer_id: PeerId = "1234567890abcdef".parse()?;
//!
//! // Use bitchat-runtime crate for runtime functionality
//! // Example:
//! // use bitchat_runtime::BitchatRuntime;
//! // let mut runtime = BitchatRuntime::new(peer_id, config);
//! // runtime.add_transport(ble_transport)?;
//! // runtime.add_transport(nostr_transport)?;
//!
//! // Start the core engine
//! // runtime.start().await?;
//!
//! // Get channels to communicate with the core (commented for doc example)
//! // let command_sender = runtime.command_sender().unwrap();
//! // let mut app_event_receiver = runtime.take_app_event_receiver().unwrap();
//!
//! // Send commands and receive events (commented for doc example)
//! // command_sender.send(Command::StartDiscovery).await?;
//!
//! // Example event loop (commented for doc example)
//! // while let Some(app_event) = app_event_receiver.recv().await {
//! //     match app_event {
//! //         AppEvent::PeerStatusChanged { peer_id, status, .. } => {
//! //             println!("Peer {} status: {:?}", peer_id, status);
//! //         }
//! //         _ => {}
//! //     }
//! // }
//! # Ok(())
//! # }
//! ```
//!
//! ### Cryptographic Operations
//!
//! The cryptographic APIs accept flexible input types for convenience:
//!
//! ```rust,no_run
//! use bitchat_core::internal::{IdentityKeyPair, generate_fingerprint};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = IdentityKeyPair::generate()?;
//!
//! // Sign with different input types
//! let data1 = b"hello world";           // &[u8]
//! let data2 = vec![1, 2, 3, 4];         // Vec<u8>
//! let data3 = "hello".to_string();      // String
//!
//! let sig1 = keypair.sign(data1);       // AsRef<[u8]> for &[u8]
//! let sig2 = keypair.sign(&data2);      // AsRef<[u8]> for &Vec<u8>
//! let sig3 = keypair.sign(data3.as_bytes()); // AsRef<[u8]> for &[u8]
//!
//! // Generate fingerprint from public key (flexible input)
//! let public_key = keypair.public_key_bytes();
//! let fingerprint = generate_fingerprint(&public_key);
//! # Ok(())
//! # }
//! ```
//!
//! ## Module Organization
//!
//! - [`channel`]: CSP communication infrastructure and protocol definitions
//! - [`protocol`]: BitChat messaging protocol implementation (crypto, sessions, delivery)
//! - [`logic`]: Core application logic and state management
//! - [`network`]: Low-level networking utilities (fragmentation, reliability)
//! - [`testing`]: Mock implementations and test utilities
//! - [`runtime`]: Generic runtime for orchestrating all components

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// ----------------------------------------------------------------------------
// Compile-time Feature Guards
// ----------------------------------------------------------------------------
// Ensures that only one of the three mutually exclusive features
// (std, wasm, testing) is enabled at build time.

// Ensure mutually exclusive features are not enabled together
#[cfg(all(feature = "std", feature = "wasm"))]
compile_error!("Features 'std' and 'wasm' are mutually exclusive. Enable only one.");

// Note: 'testing' feature includes 'std' automatically, so they can coexist

#[cfg(all(feature = "wasm", feature = "testing"))]
compile_error!("Features 'wasm' and 'testing' are mutually exclusive. Enable only one.");

// Ensure at least one feature is enabled (since testing includes std)
#[cfg(not(any(feature = "std", feature = "wasm", feature = "testing")))]
compile_error!("At least one of 'std', 'wasm', or 'testing' features must be enabled.");

// Feature documentation for users
#[cfg(feature = "std")]
const _FEATURE_DOCS_STD: &str = "
BitChat Core compiled with 'std' feature:
- Full standard library support
- Tokio async runtime  
- Tracing logging
- All monitoring capabilities
- Optimized for CLI/server environments
";

#[cfg(feature = "wasm")]
const _FEATURE_DOCS_WASM: &str = "
BitChat Core compiled with 'wasm' feature:
- WebAssembly compatible (no_std)
- Browser async support via wasm-bindgen-futures
- Reduced feature set for smaller bundle size
- JavaScript interop capabilities
- Optimized for browser environments
";

#[cfg(feature = "testing")]
const _FEATURE_DOCS_TESTING: &str = "
BitChat Core compiled with 'testing' feature:
- All 'std' features included
- Additional test utilities
- Mock implementations
- Enhanced debugging capabilities
- Optimized for test environments
";

// ----------------------------------------------------------------------------
// Module Declarations
// ----------------------------------------------------------------------------

pub mod errors;
pub mod types;

#[cfg(feature = "task-logging")]
pub mod task_logging;

pub mod transport_task;

#[cfg(feature = "monitoring")]
pub mod monitoring;

pub mod config;
// Submodules
pub mod channel;
pub mod protocol;

#[cfg(feature = "std")]
pub mod geohash;

// ----------------------------------------------------------------------------
// Public API - Minimal Interface for Application Developers
// ----------------------------------------------------------------------------

// Essential exports for application developers
pub use channel::{AppEvent, ChannelTransportType, Command, ConnectionStatus, TransportStatus};
pub use config::{BitchatConfig, SharedBitchatConfig};
pub use errors::{BitchatError, BitchatResult, Result};
pub use transport_task::TransportTask;
pub use types::{Fingerprint, PeerId, Timestamp};

#[cfg(feature = "std")]
pub use geohash::{
    ChannelIdentity, GeoLocation, GeohashChannel, GeohashError, GeohashPrecision,
    LocationPrivacyManager,
};

// Core protocol exports (canonical implementation features)
pub use protocol::{
    BitchatMessage, BitchatPacket, BloomFilter, ConnectionEvent, ConnectionState, 
    DeduplicationManager, DeduplicationStats, DeliveryAttempt, DeliveryStatus, Fragment,
    FragmentHeader, MessageFragmenter, MessageReassembler, MessageType, NoiseHandshake,
    NoisePayload, NoisePayloadType, PacketFlags, PacketHeader, TrackedMessage,
};

// Experimental feature exports (only available with experimental feature flag)
#[cfg(feature = "experimental")]
pub use protocol::{
    // File transfer
    FileAccept, FileChunk, FileComplete, FileHash, FileMetadata, FileOffer,
    FileTransferId, FileTransferManager, FileTransferMessage, FileTransferSession, TransferStatus,
    // Group messaging
    GroupCreate, GroupId, GroupInvite, GroupJoin, GroupKick, GroupLeave, GroupManager, GroupMember,
    GroupMessage, GroupMessagingMessage, GroupMetadata, GroupRole, GroupSettings, GroupUpdate,
    // Multi-device sync
    DeviceAnnouncement, DeviceCapabilities, DeviceHeartbeat, DeviceId, DeviceInfo, DeviceStatus,
    DeviceType, MessageRef, MultiDeviceSessionManager, SessionSyncState, SessionStatus,
    SessionSyncMessage, SessionSyncRequest, SessionSyncResponse,
    // Capability negotiation
    Capability, CapabilityId, CapabilityManager, CapabilityMessage, CapabilityRejection,
    ImplementationInfo, NegotiationStatus, ProtocolVersion, RejectionReason, VersionAck, VersionHello,
};

// ----------------------------------------------------------------------------
// Internal API - For Transport and UI Crate Developers
// ----------------------------------------------------------------------------

// These are needed by transport crate developers (bitchat-ble, bitchat-nostr, etc.)
pub use channel::{Effect, EffectReceiver, Event, EventSender};

// ----------------------------------------------------------------------------
// Advanced/Internal API - Use at Your Own Risk
// ----------------------------------------------------------------------------
// These are internal implementation details that may change without notice.
// They are exported for advanced use cases and internal testing.

#[doc(hidden)]
pub mod internal {
    pub use crate::channel::{
        create_app_event_channel, create_command_channel, create_effect_channel,
        create_effect_receiver, create_event_channel, AppEventReceiver, AppEventSender,
        ChannelConfig, ChannelError, ChannelStats, CommandReceiver, CommandSender, EffectReceiver,
        EffectSender, EventReceiver, EventSender, NonBlockingSend, TaskSpawner,
    };
    pub use crate::config::{
        BitchatConfig, ConfigPresets, DeliveryConfig, MessageStoreConfig, MonitoringConfig,
        RateLimitConfig, SessionConfig, TestConfig,
    };
    pub use crate::errors::{
        CryptographicError, FragmentationError, PacketError, SessionError, TransportError,
    };
    #[cfg(feature = "monitoring")]
    pub use crate::monitoring::{
        ChannelUtilization, DeadlockWarning, Monitorable, MonitoringReport, MonitoringSystem,
        PerformanceMetrics, TaskHealth, TaskHealthMetrics,
    };
    pub use crate::protocol::{
        generate_fingerprint, AuditEntry, ConnectionEvent, ConnectionState,
        ContentAddressedMessage, ConversationId, DeliveryAttempt, DeliveryStatus, IdentityKeyPair,
        MessageId, MessageStore, MessageStoreStats, NoiseHandshake, NoiseKeyPair, NoiseSession,
        NoiseTransport, SessionParams, SessionState, StateTransition, StateTransitionError,
        TrackedMessage,
    };
    #[cfg(feature = "task-logging")]
    pub use crate::task_logging::{
        CommEvent, ConsoleLogger, Direction, LogLevel, MessageSummary,
        MessageType as LogMessageType, NoOpLogger, TaskId, TaskLogger,
    };
    #[cfg(feature = "std")]
    pub use crate::types::SystemTimeSource;
    pub use crate::types::{Fingerprint, TimeSource, Timestamp};
}

// Convenience type aliases for different feature combinations
cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub use types::SystemTimeSource;
    } else if #[cfg(feature = "wasm")] {
        pub use types::WasmTimeSource;
    }
}
